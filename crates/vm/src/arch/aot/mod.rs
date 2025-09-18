use std::ffi::c_void;

use openvm_instructions::instruction::Instruction;
use openvm_stark_backend::p3_field::PrimeField32;

use crate::{
    arch::{execution_mode::ExecutionCtxTrait, StaticProgramError, VmExecState},
    system::memory::online::GuestMemory,
};

pub mod compiler;
pub mod traits;

pub use compiler::*;
pub use traits::*;

pub type AotExecState = VmExecState<BabyBear, GuestMemory, E1ExecutionCtx>;
pub type AotHandler = unsafe extern "C" fn(
    pre_compute: *const u8,
    instret: *mut u64,
    pc: *mut u32,
    arg: u64,
    state: *mut AotExecState,
);

pub trait AotExecutor<F: PrimeField32> {
    fn generate_aot_assembly(
        &self,
        _pc: u32,
        _inst: &Instruction<F>,
    ) -> Result<Option<String>, StaticProgramError> {
        Ok(None)
    }

    fn aot_compile(
        &self,
        _pc: u32,
        _inst: &Instruction<F>,
    ) -> Result<Option<AotHandler>, StaticProgramError> {
        Ok(None)
    }
}
