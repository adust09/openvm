use openvm_instructions::instruction::Instruction;
use openvm_stark_backend::p3_field::PrimeField32;

use super::AotExecState;
use crate::arch::StaticProgramError;

pub type AotHandler = unsafe extern "C" fn(state: *mut AotExecState) -> u32;

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
