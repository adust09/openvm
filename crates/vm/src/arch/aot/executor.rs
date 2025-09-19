use openvm_instructions::instruction::Instruction;
use openvm_stark_backend::p3_field::PrimeField32;

use crate::arch::StaticProgramError;

pub trait AotExecutor<F: PrimeField32> {
    fn generate_aot_assembly(
        &self,
        _pc: u32,
        _inst: &Instruction<F>,
    ) -> Result<Option<String>, StaticProgramError> {
        Ok(None)
    }
}
