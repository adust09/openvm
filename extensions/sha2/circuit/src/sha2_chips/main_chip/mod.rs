mod air;
mod columns;
mod config;
mod trace;

use std::{
    marker::PhantomData,
    sync::{Arc, Mutex},
};

pub use air::*;
pub use columns::*;
pub use config::*;
use openvm_circuit::{arch::VmChipWrapper, system::memory::SharedMemoryHelper};
use openvm_circuit_primitives::bitwise_op_lookup::SharedBitwiseOperationLookupChip;
use openvm_stark_backend::p3_field::PrimeField32;
pub use trace::*;

use crate::{Sha2BlockHasherChip, Sha2Config};

pub struct Sha2MainChip<F, RA, C: Sha2Config> {
    // This Arc<Mutex<Option<RA>>> is shared with the block hasher chip (Sha2BlockHasherChip).
    // When the main chip's tracegen is done, it will set the value of the mutex to Some(arena)
    // and then the block hasher chip can see the arena and use it to generate its trace.
    // The arc mutex is not strictly necessary (we could just use a Cell) because tracegen is done
    // sequentially over the list of chips (although it is parallelized within each chip), but the
    // overhead of using a thread-safe type is negligible since we only access the 'arena' field
    // twice (once to set the value and once to get the value).
    // So, we will just use an arc mutex to avoid overcomplicating things.
    pub arena: Arc<Mutex<Option<RA>>>,
    pub bitwise_lookup_chip: SharedBitwiseOperationLookupChip<8>,
    pub pointer_max_bits: usize,
    pub mem_helper: SharedMemoryHelper<F>,
    _phantom: PhantomData<C>,
}

impl<F, RA, C: Sha2Config> Sha2MainChip<F, RA, C> {
    pub fn new(
        arena: Arc<Mutex<Option<RA>>>,
        bitwise_lookup_chip: SharedBitwiseOperationLookupChip<8>,
        pointer_max_bits: usize,
        mem_helper: SharedMemoryHelper<F>,
    ) -> Self {
        Self {
            arena,
            bitwise_lookup_chip,
            pointer_max_bits,
            mem_helper,
            _phantom: PhantomData,
        }
    }
}
