use openvm_circuit::arch::{aot::compile_and_execute_aot as core_compile, StaticProgramError};
use openvm_instructions::{exe::VmExe, LocalOpcode};
use openvm_rv32im_transpiler::BaseAluOpcode;
use openvm_stark_backend::p3_field::PrimeField32;

use crate::{
    adapters::{Rv32BaseAluAdapterExecutor, RV32_CELL_BITS},
    Rv32BaseAluExecutor,
};

/// Compile and execute a program with RV32IM AOT where possible
/// Uses the modular AOT system with registered RV32IM executors
pub fn compile_and_execute_aot<F: PrimeField32>(
    exe: &VmExe<F>,
) -> Result<String, StaticProgramError> {
    // Create RV32IM AOT executors
    let base_alu_executor = Rv32BaseAluExecutor::new(
        Rv32BaseAluAdapterExecutor::<RV32_CELL_BITS>::new(),
        BaseAluOpcode::CLASS_OFFSET,
    );

    // Collect all AOT executors for RV32IM
    let aot_executors = vec![
        base_alu_executor,
        // Add other RV32IM executors here as they get AOT support:
        // shift_executor,
        // less_than_executor,
        // etc.
    ];

    // Use the core AOT compiler with our executors
    core_compile(exe, &aot_executors)
}
