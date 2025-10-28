use std::{
    array::{self, from_fn},
    borrow::{Borrow, BorrowMut},
    cmp::min,
    sync::Arc,
};

use ndarray::ArrayViewMut;
use openvm_circuit::{
    arch::*,
    system::memory::{
        offline_checker::{MemoryReadAuxRecord, MemoryWriteBytesAuxRecord},
        online::TracingMemory,
        MemoryAuxColsFactory,
    },
};
use openvm_circuit_primitives::AlignedBytesBorrow;
use openvm_instructions::{
    instruction::Instruction,
    program::DEFAULT_PC_STEP,
    riscv::{RV32_CELL_BITS, RV32_MEMORY_AS, RV32_REGISTER_AS, RV32_REGISTER_NUM_LIMBS},
    LocalOpcode,
};
use openvm_rv32im_circuit::adapters::{read_rv32_register, tracing_read, tracing_write};
use openvm_sha2_air::set_arrayview_from_u8_slice;
use openvm_sha2_transpiler::Rv32Sha2Opcode;
use openvm_stark_backend::{
    config::{StarkGenericConfig, Val},
    p3_air::{Air, AirBuilder, BaseAir},
    p3_field::{Field, FieldAlgebra, PrimeField32},
    p3_matrix::{dense::RowMajorMatrix, Matrix},
    p3_maybe_rayon::prelude::*,
    prover::{cpu::CpuBackend, types::AirProvingContext},
    rap::{BaseAirWithPublicValues, PartitionedBaseAir},
    Chip,
};

use crate::{
    Sha2ColsRef, Sha2ColsRefMut, Sha2Config, Sha2MainChip, Sha2Metadata, Sha2RecordHeader,
    Sha2RecordLayout, Sha2RecordMut, SHA2_WRITE_SIZE,
};

// We will allocate a new trace matrix instead of using the record arena directly,
// because we want to hand the record to Sha2BlockHasherChip to generate its trace.
impl<'a, RA, SC: StarkGenericConfig, C: Sha2Config> Chip<RA, CpuBackend<SC>>
    for Sha2MainChip<Val<SC>, RA, C>
where
    Self: TraceFiller<Val<SC>>,
    SC: StarkGenericConfig,
    RA: RowMajorMatrixArena<Val<SC>>,
{
    fn generate_proving_ctx(&self, arena: RA) -> AirProvingContext<CpuBackend<SC>> {
        let rows_used = arena.trace_offset() / arena.width();
        let trace = Val::<SC>::zero_vec(rows_used * arena.width());
        let mut trace_matrix = RowMajorMatrix::new(trace, arena.width());
        let mem_helper = self.mem_helper.as_borrowed();

        self.fill_trace(&mem_helper, &mut trace_matrix, rows_used);

        *self.arena.lock().unwrap() = Some(arena);

        AirProvingContext::simple(Arc::new(trace_matrix), self.generate_public_values())
    }
}

// The trace generation for each row is almost indepedent.
// The only problematic column is request_id, which should be 0 on the first row and
// incremented by 1 for each subsequent row.
impl<'a, F: PrimeField32, RA: Send + Sync, C: Sha2Config> TraceFiller<F>
    for Sha2MainChip<F, RA, C>
{
    // Similar to the default implementation of TraceFiller::fill_trace, but we need to pass the
    // row index to the fill_trace_row_with_row_idx function.
    fn fill_trace(
        &self,
        mem_helper: &MemoryAuxColsFactory<F>,
        trace: &mut RowMajorMatrix<F>,
        rows_used: usize,
    ) {
        let width = trace.width();
        trace.values[..rows_used * width]
            .par_chunks_exact_mut(width)
            .enumerate()
            .for_each(|(row_idx, row_slice)| {
                self.fill_trace_row_with_row_idx(mem_helper, row_slice, row_idx);
            });
        trace.values[rows_used * width..]
            .par_chunks_exact_mut(width)
            .for_each(|row_slice| {
                // fill with zeros
                self.fill_dummy_trace_row(row_slice);
            });
    }
}

impl<F: PrimeField32, RA, C: Sha2Config> Sha2MainChip<F, RA, C> {
    fn fill_trace_row_with_row_idx(
        &self,
        mem_helper: &MemoryAuxColsFactory<F>,
        mut row_slice: &mut [F],
        row_idx: usize,
    ) where
        F: Clone,
    {
        // SAFETY:
        // - caller ensures `trace` contains a valid record representation that was previously
        //   written by the executor
        // - slice contains a valid Sha2RecordMut with the exact layout specified
        // - get_record_from_slice will correctly split the buffer into header and other components
        //   based on this layout.
        let record: Sha2RecordMut = unsafe {
            get_record_from_slice(
                &mut row_slice,
                Sha2RecordLayout::new(Sha2Metadata {
                    variant: C::VARIANT,
                }),
            )
        };

        // save all the components of the record on the stack so that we don't overwrite them when
        // filling in the trace matrix.
        let vm_record = record.inner.clone();

        let mut message_bytes = Vec::with_capacity(C::BLOCK_BYTES);
        message_bytes.extend_from_slice(record.message_bytes);

        let mut prev_state = Vec::with_capacity(C::STATE_BYTES);
        prev_state.extend_from_slice(record.prev_state);

        let mut new_state = prev_state.clone();
        C::compress(&mut new_state, &message_bytes);

        let mut input_reads_aux =
            Vec::with_capacity(C::BLOCK_READS * size_of::<MemoryReadAuxRecord>());
        input_reads_aux.extend_from_slice(record.input_reads_aux);

        let mut state_reads_aux =
            Vec::with_capacity(C::STATE_READS * size_of::<MemoryReadAuxRecord>());
        state_reads_aux.extend_from_slice(record.state_reads_aux);

        let mut write_aux = Vec::with_capacity(
            C::DIGEST_WRITES * size_of::<MemoryWriteBytesAuxRecord<SHA2_WRITE_SIZE>>(),
        );
        write_aux.extend_from_slice(record.write_aux);

        let mut cols = Sha2ColsRefMut::from::<C>(&mut row_slice);

        *cols.block.request_id = F::from_canonical_usize(row_idx);
        set_arrayview_from_u8_slice(&mut cols.block.message_bytes, message_bytes);
        set_arrayview_from_u8_slice(&mut cols.block.prev_state, prev_state);
        set_arrayview_from_u8_slice(&mut cols.block.new_state, new_state);

        *cols.instruction.is_enabled = F::ONE;
        cols.instruction.from_state.timestamp = F::from_canonical_u32(vm_record.timestamp);
        cols.instruction.from_state.pc = F::from_canonical_u32(vm_record.from_pc);
        *cols.instruction.dst_reg_ptr = F::from_canonical_u32(vm_record.dst_reg_ptr);
        *cols.instruction.state_reg_ptr = F::from_canonical_u32(vm_record.state_reg_ptr);
        *cols.instruction.input_reg_ptr = F::from_canonical_u32(vm_record.input_reg_ptr);
        set_arrayview_from_u8_slice(
            &mut cols.instruction.dst_ptr_limbs,
            vm_record.dst_ptr.to_le_bytes(),
        );
        set_arrayview_from_u8_slice(
            &mut cols.instruction.state_ptr_limbs,
            vm_record.state_ptr.to_le_bytes(),
        );
        set_arrayview_from_u8_slice(
            &mut cols.instruction.input_ptr_limbs,
            vm_record.input_ptr.to_le_bytes(),
        );

        // fill in the register reads aux
        let mut timestamp = vm_record.timestamp;
        for (cols, vm_record) in cols
            .mem
            .register_aux
            .iter_mut()
            .zip(vm_record.register_reads_aux.iter())
        {
            mem_helper.fill(vm_record.prev_timestamp, timestamp, cols.as_mut());
            timestamp += 1;
        }

        for i in 0..C::BLOCK_READS {
            mem_helper.fill(
                input_reads_aux[i].prev_timestamp,
                timestamp,
                cols.mem.input_reads[i].as_mut(),
            );
            timestamp += 1;
        }

        for i in 0..C::STATE_READS {
            mem_helper.fill(
                state_reads_aux[i].prev_timestamp,
                timestamp,
                cols.mem.state_reads[i].as_mut(),
            );
            timestamp += 1;
        }

        for i in 0..C::DIGEST_WRITES {
            mem_helper.fill(
                write_aux[i].prev_timestamp,
                timestamp,
                cols.mem.write_aux[i].as_mut(),
            );
            timestamp += 1;
        }
    }
}
