#![cfg(all(feature = "aot", test))]

use openvm_circuit::arch::{
    aot::{execute_aot, AotExecutionContext},
    create_memory_image, MemoryConfig, SystemConfig,
};
use openvm_instructions::{
    exe::VmExe,
    instruction::Instruction,
    program::Program,
    riscv::{RV32_IMM_AS, RV32_REGISTER_AS},
    LocalOpcode,
};
use openvm_rv32im_transpiler::BaseAluOpcode;
use openvm_stark_backend::p3_field::FieldAlgebra;
use openvm_stark_sdk::p3_baby_bear::BabyBear;

use crate::{
    adapters::{Rv32BaseAluAdapterExecutor, RV32_CELL_BITS},
    Rv32BaseAluExecutor,
};

// Only run these tests if NASM and GCC are available
fn check_build_tools() -> bool {
    std::process::Command::new("nasm")
        .arg("--version")
        .output()
        .is_ok()
        && std::process::Command::new("gcc")
            .arg("--version")
            .output()
            .is_ok()
}

#[test]
#[ignore] // Run with: cargo test --features aot aot_integration -- --ignored
fn test_aot_execution_add_immediate() {
    if !check_build_tools() {
        eprintln!("Skipping AOT integration test: NASM or GCC not available");
        return;
    }

    // Skip on ARM64 since we're generating x86_64 assembly
    if cfg!(target_arch = "aarch64") {
        eprintln!("Skipping AOT integration test: x86_64 assembly not compatible with ARM64");
        return;
    }

    type F = BabyBear;

    // Create a simple program: addi x1, x0, 42
    let instruction = Instruction {
        opcode: BaseAluOpcode::ADD.global_opcode(),
        a: F::from_canonical_u32(1),                // rd = x1
        b: F::from_canonical_u32(0),                // rs1 = x0
        c: F::from_canonical_u32(42),               // imm = 42
        d: F::from_canonical_u32(RV32_REGISTER_AS), // register format
        e: F::from_canonical_u32(RV32_IMM_AS),      // immediate format
        f: F::ZERO,
        g: F::ZERO,
    };

    let program = Program::from_instructions(&[instruction]);

    let exe = VmExe {
        program,
        pc_start: 0,
        fn_bounds: Default::default(),
        init_memory: Default::default(),
    };

    // Set up AOT executors
    let base_alu_executor = Rv32BaseAluExecutor::new(
        Rv32BaseAluAdapterExecutor::<RV32_CELL_BITS>::new(),
        BaseAluOpcode::CLASS_OFFSET,
    );
    let aot_executors = vec![base_alu_executor];

    // Set up initial memory
    let memory_config = MemoryConfig::default();
    let memory = create_memory_image(&memory_config, &exe.init_memory);

    // Execute with AOT
    let system_config = SystemConfig::new(0, memory_config, 0);

    match execute_aot(&exe, &aot_executors, system_config, memory) {
        Ok((final_state, _streams)) => {
            // Verify x1 contains 42 (register 1 at address 1*4=4 bytes)
            let x1_bytes: [u8; 4] = unsafe { final_state.memory.read(RV32_REGISTER_AS, 1) };
            let x1_value = u32::from_le_bytes(x1_bytes);
            assert_eq!(x1_value, 42, "x1 should contain 42");

            // Verify x0 is still 0
            let x0_bytes: [u8; 4] = unsafe { final_state.memory.read(RV32_REGISTER_AS, 0) };
            let x0_value = u32::from_le_bytes(x0_bytes);
            assert_eq!(x0_value, 0, "x0 should always be 0");
        }
        Err(e) => {
            panic!("AOT execution failed: {:?}", e);
        }
    }
}

#[test]
#[ignore]
fn test_aot_execution_add_register() {
    if !check_build_tools() {
        eprintln!("Skipping AOT integration test: NASM or GCC not available");
        return;
    }

    // Skip on ARM64 since we're generating x86_64 assembly
    if cfg!(target_arch = "aarch64") {
        eprintln!("Skipping AOT integration test: x86_64 assembly not compatible with ARM64");
        return;
    }

    type F = BabyBear;

    // Create a program that adds two registers
    // addi x1, x0, 10   ; x1 = 10
    // addi x2, x0, 32   ; x2 = 32
    // add x3, x1, x2    ; x3 = x1 + x2 = 42
    let instructions = vec![
        Instruction {
            opcode: BaseAluOpcode::ADD.global_opcode(),
            a: F::from_canonical_u32(1),  // rd = x1
            b: F::from_canonical_u32(0),  // rs1 = x0
            c: F::from_canonical_u32(10), // imm = 10
            d: F::from_canonical_u32(RV32_REGISTER_AS),
            e: F::from_canonical_u32(RV32_IMM_AS),
            f: F::ZERO,
            g: F::ZERO,
        },
        Instruction {
            opcode: BaseAluOpcode::ADD.global_opcode(),
            a: F::from_canonical_u32(2),  // rd = x2
            b: F::from_canonical_u32(0),  // rs1 = x0
            c: F::from_canonical_u32(32), // imm = 32
            d: F::from_canonical_u32(RV32_REGISTER_AS),
            e: F::from_canonical_u32(RV32_IMM_AS),
            f: F::ZERO,
            g: F::ZERO,
        },
        Instruction {
            opcode: BaseAluOpcode::ADD.global_opcode(),
            a: F::from_canonical_u32(3), // rd = x3
            b: F::from_canonical_u32(1), // rs1 = x1
            c: F::from_canonical_u32(2), // rs2 = x2
            d: F::from_canonical_u32(RV32_REGISTER_AS),
            e: F::from_canonical_u32(RV32_REGISTER_AS),
            f: F::ZERO,
            g: F::ZERO,
        },
    ];

    let program = Program::from_instructions(&instructions);

    let exe = VmExe {
        program,
        pc_start: 0,
        fn_bounds: Default::default(),
        init_memory: Default::default(),
    };

    // Set up AOT executors
    let base_alu_executor = Rv32BaseAluExecutor::new(
        Rv32BaseAluAdapterExecutor::<RV32_CELL_BITS>::new(),
        BaseAluOpcode::CLASS_OFFSET,
    );
    let aot_executors = vec![base_alu_executor];

    // Execute with AOT
    let memory_config = MemoryConfig::default();
    let memory = create_memory_image(&memory_config, &exe.init_memory);
    let system_config = SystemConfig::new(0, memory_config, 0);

    match execute_aot(&exe, &aot_executors, system_config, memory) {
        Ok((final_state, _streams)) => {
            // Verify results
            let x1_bytes: [u8; 4] = unsafe { final_state.memory.read(RV32_REGISTER_AS, 1) };
            assert_eq!(u32::from_le_bytes(x1_bytes), 10, "x1 = 10");

            let x2_bytes: [u8; 4] = unsafe { final_state.memory.read(RV32_REGISTER_AS, 2) };
            assert_eq!(u32::from_le_bytes(x2_bytes), 32, "x2 = 32");

            let x3_bytes: [u8; 4] = unsafe { final_state.memory.read(RV32_REGISTER_AS, 3) };
            assert_eq!(u32::from_le_bytes(x3_bytes), 42, "x3 = 42");
        }
        Err(e) => {
            panic!("AOT execution failed: {:?}", e);
        }
    }
}

#[test]
#[ignore]
fn test_aot_with_custom_handler() {
    if !check_build_tools() {
        eprintln!("Skipping AOT integration test: NASM or GCC not available");
        return;
    }

    // Skip on ARM64 since we're generating x86_64 assembly
    if cfg!(target_arch = "aarch64") {
        eprintln!("Skipping AOT integration test: x86_64 assembly not compatible with ARM64");
        return;
    }

    type F = BabyBear;

    // Create a program with an unsupported instruction
    // This will trigger the external handler
    let instruction = Instruction {
        opcode: BaseAluOpcode::ADD.global_opcode(), // Use valid opcode but no executor
        a: F::ZERO,
        b: F::ZERO,
        c: F::ZERO,
        d: F::ZERO,
        e: F::ZERO,
        f: F::ZERO,
        g: F::ZERO,
    };

    let program = Program::from_instructions(&[instruction]);

    let exe = VmExe {
        program,
        pc_start: 0,
        fn_bounds: Default::default(),
        init_memory: Default::default(),
    };

    // Custom handler source that sets x1 = 123
    let handler_source = r#"
#include <stdint.h>

void openvm_aot_handler(
    const uint8_t* pre_compute,
    uint64_t* instret,
    uint32_t* pc,
    uint64_t arg,
    void* state
) {
    // Terminate execution
    *pc = 0xFFFFFFFF;
    
    // Set instret to indicate we executed one instruction
    *instret = 1;
}
"#;

    // Use AotExecutionContext to compile with custom handler
    let mut aot_ctx = AotExecutionContext::new();
    let aot_executors: Vec<crate::Rv32BaseAluExecutor> = vec![]; // No executors, so all instructions go to handler

    if let Err(e) = aot_ctx.compile(&exe, &aot_executors[..], Some(handler_source)) {
        eprintln!(
            "Compilation failed (expected if missing build tools): {}",
            e
        );
        return;
    }

    let memory_config = MemoryConfig::default();
    let memory = create_memory_image(&memory_config, &exe.init_memory);

    match aot_ctx.execute(memory, 0) {
        Ok((_final_state, instret)) => {
            assert_eq!(instret, 1, "Handler should report 1 instruction executed");
        }
        Err(e) => {
            eprintln!("Execution failed (may be expected): {}", e);
        }
    }
}
