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

use crate::compile_and_execute_aot;

#[test]
fn test_aot_base_alu_add_immediate() {
    type F = BabyBear;

    // Create a simple instruction: addi x10, x0, 42
    let instruction = Instruction {
        opcode: BaseAluOpcode::ADD.global_opcode(),
        a: F::from_canonical_u32(10),               // rd = x10
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

    // Test AOT compilation
    let assembly = compile_and_execute_aot(&exe).expect("AOT compilation should succeed");

    // Verify assembly contains expected patterns
    assert!(
        assembly.contains("openvm_aot_start"),
        "Should contain entry point"
    );
    assert!(assembly.contains(".pc_00000000"), "Should contain PC label");
    assert!(
        assembly.contains("addi x10, x0, 42"),
        "Should contain instruction comment"
    );
    assert!(
        assembly.contains("mov dword ptr [rbx + 40], 42"),
        "Should contain direct immediate store"
    );

    println!("Generated assembly:");
    println!("{}", assembly);
}

#[test]
fn test_aot_base_alu_add_register() {
    type F = BabyBear;

    // Create an instruction: add x10, x1, x2
    let instruction = Instruction {
        opcode: BaseAluOpcode::ADD.global_opcode(),
        a: F::from_canonical_u32(10),               // rd = x10
        b: F::from_canonical_u32(1),                // rs1 = x1
        c: F::from_canonical_u32(2),                // rs2 = x2
        d: F::from_canonical_u32(RV32_REGISTER_AS), // register format
        e: F::from_canonical_u32(RV32_REGISTER_AS), // register format
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

    // Test AOT compilation
    let assembly = compile_and_execute_aot(&exe).expect("AOT compilation should succeed");

    // Verify assembly contains expected patterns
    assert!(
        assembly.contains("add x10, x1, x2"),
        "Should contain instruction comment"
    );
    assert!(
        assembly.contains("mov r15d, dword ptr [rbx + 4]"),
        "Should load from x1"
    );
    assert!(
        assembly.contains("add r15d, dword ptr [rbx + 8]"),
        "Should add x2"
    );
    assert!(
        assembly.contains("mov dword ptr [rbx + 40], r15d"),
        "Should store to x10"
    );

    println!("Generated register add assembly:");
    println!("{}", assembly);
}

#[test]
fn test_aot_base_alu_sub_immediate() {
    type F = BabyBear;

    // Create an instruction: subi x5, x1, 10
    let instruction = Instruction {
        opcode: BaseAluOpcode::SUB.global_opcode(),
        a: F::from_canonical_u32(5),  // rd = x5
        b: F::from_canonical_u32(1),  // rs1 = x1
        c: F::from_canonical_u32(10), // imm = 10
        d: F::from_canonical_u32(RV32_REGISTER_AS),
        e: F::from_canonical_u32(RV32_IMM_AS),
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

    let assembly = compile_and_execute_aot(&exe).expect("AOT compilation should succeed");

    assert!(
        assembly.contains("subi x5, x1, 10"),
        "Should contain instruction comment"
    );
    assert!(
        assembly.contains("sub r15d, 10"),
        "Should subtract immediate"
    );

    println!("Generated SUB immediate assembly:");
    println!("{}", assembly);
}

#[test]
fn test_aot_base_alu_xor_register() {
    type F = BabyBear;

    // Create an instruction: xor x3, x1, x2
    let instruction = Instruction {
        opcode: BaseAluOpcode::XOR.global_opcode(),
        a: F::from_canonical_u32(3), // rd = x3
        b: F::from_canonical_u32(1), // rs1 = x1
        c: F::from_canonical_u32(2), // rs2 = x2
        d: F::from_canonical_u32(RV32_REGISTER_AS),
        e: F::from_canonical_u32(RV32_REGISTER_AS),
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

    let assembly = compile_and_execute_aot(&exe).expect("AOT compilation should succeed");

    assert!(
        assembly.contains("xor x3, x1, x2"),
        "Should contain instruction comment"
    );
    assert!(
        assembly.contains("xor r15d, dword ptr [rbx + 8]"),
        "Should XOR with x2"
    );

    println!("Generated XOR register assembly:");
    println!("{}", assembly);
}

#[test]
fn test_aot_base_alu_or_immediate() {
    type F = BabyBear;

    // Create an instruction: ori x7, x0, 255
    let instruction = Instruction {
        opcode: BaseAluOpcode::OR.global_opcode(),
        a: F::from_canonical_u32(7),   // rd = x7
        b: F::from_canonical_u32(0),   // rs1 = x0
        c: F::from_canonical_u32(255), // imm = 255
        d: F::from_canonical_u32(RV32_REGISTER_AS),
        e: F::from_canonical_u32(RV32_IMM_AS),
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

    let assembly = compile_and_execute_aot(&exe).expect("AOT compilation should succeed");

    assert!(
        assembly.contains("ori x7, x0, 255"),
        "Should contain instruction comment"
    );
    assert!(
        assembly.contains("mov dword ptr [rbx + 28], 255"),
        "Should store immediate result"
    );

    println!("Generated OR immediate assembly:");
    println!("{}", assembly);
}

#[test]
fn test_aot_base_alu_and_register() {
    type F = BabyBear;

    // Create an instruction: and x4, x1, x2
    let instruction = Instruction {
        opcode: BaseAluOpcode::AND.global_opcode(),
        a: F::from_canonical_u32(4), // rd = x4
        b: F::from_canonical_u32(1), // rs1 = x1
        c: F::from_canonical_u32(2), // rs2 = x2
        d: F::from_canonical_u32(RV32_REGISTER_AS),
        e: F::from_canonical_u32(RV32_REGISTER_AS),
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

    let assembly = compile_and_execute_aot(&exe).expect("AOT compilation should succeed");

    assert!(
        assembly.contains("and x4, x1, x2"),
        "Should contain instruction comment"
    );
    assert!(
        assembly.contains("and r15d, dword ptr [rbx + 8]"),
        "Should AND with x2"
    );

    println!("Generated AND register assembly:");
    println!("{}", assembly);
}

#[test]
fn test_aot_base_alu_zero_optimizations() {
    type F = BabyBear;

    // Test AND with x0 (should always result in 0)
    let instruction = Instruction {
        opcode: BaseAluOpcode::AND.global_opcode(),
        a: F::from_canonical_u32(5), // rd = x5
        b: F::from_canonical_u32(0), // rs1 = x0
        c: F::from_canonical_u32(7), // rs2 = x7
        d: F::from_canonical_u32(RV32_REGISTER_AS),
        e: F::from_canonical_u32(RV32_REGISTER_AS),
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

    let assembly = compile_and_execute_aot(&exe).expect("AOT compilation should succeed");

    assert!(
        assembly.contains("and x5, x0, x7"),
        "Should contain instruction comment"
    );
    assert!(
        assembly.contains("mov dword ptr [rbx + 20], 0"),
        "Should store 0 directly (X&0 = 0)"
    );

    println!("Generated zero optimization assembly:");
    println!("{}", assembly);
}
