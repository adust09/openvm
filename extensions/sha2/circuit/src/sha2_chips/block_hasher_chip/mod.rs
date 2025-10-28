mod air;
mod columns;

mod config;
mod trace;

use std::{
    cell::Cell,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

pub use air::*;
pub use columns::*;
pub use config::*;
use openvm_circuit::{
    arch::{RowMajorMatrixArena, VmChipWrapper},
    system::memory::SharedMemoryHelper,
};
use openvm_circuit_primitives::{
    bitwise_op_lookup::SharedBitwiseOperationLookupChip, encoder::Encoder,
};
use openvm_instructions::riscv::RV32_CELL_BITS;
use openvm_sha2_air::{Sha2BlockHasherFillerHelper, Sha2BlockHasherSubairConfig};
pub use trace::*;

pub use super::config::*;

pub struct Sha2BlockHasherChip<F, RA, C: Sha2BlockHasherSubairConfig> {
    pub inner: Sha2BlockHasherFillerHelper<C>,
    pub bitwise_lookup_chip: SharedBitwiseOperationLookupChip<RV32_CELL_BITS>,
    pub pointer_max_bits: usize,
    pub mem_helper: SharedMemoryHelper<F>,
    // This Arc<Mutex<Option<RA>>> is shared with the main chip (Sha2MainChip).
    // When the main chip's tracegen is done, it will set the value of the mutex to Some(arena)
    // and then the block hasher chip can see the arena and use it to generate its trace.
    // The arc mutex is not strictly necessary (we could just use a Cell) because tracegen is done
    // sequentially over the list of chips (although it is parallelized within each chip), but the
    // overhead of using a thread-safe type is negligible since we only access the 'arena' field
    // twice (once to set the value and once to get the value).
    // So, we will just use an arc mutex to avoid overcomplicating things.
    pub arena: Arc<Mutex<Option<RA>>>,
    _phantom: PhantomData<C>,
}

impl<F, RA, C: Sha2BlockHasherSubairConfig> Sha2BlockHasherChip<F, RA, C> {
    pub fn new(
        bitwise_lookup_chip: SharedBitwiseOperationLookupChip<RV32_CELL_BITS>,
        pointer_max_bits: usize,
        mem_helper: SharedMemoryHelper<F>,
        arena: Arc<Mutex<Option<RA>>>,
    ) -> Self {
        Self {
            inner: Sha2BlockHasherFillerHelper::new(),
            bitwise_lookup_chip,
            pointer_max_bits,
            mem_helper,
            arena,
            _phantom: PhantomData,
        }
    }
}
