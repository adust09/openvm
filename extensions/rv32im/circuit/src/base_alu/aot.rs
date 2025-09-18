use openvm_circuit::arch::{aot::AotExecutor, StaticProgramError};
use openvm_instructions::{
    instruction::Instruction,
    riscv::{RV32_IMM_AS, RV32_REGISTER_AS},
    LocalOpcode,
};
use openvm_rv32im_transpiler::BaseAluOpcode;
use openvm_stark_backend::p3_field::PrimeField32;

use super::BaseAluExecutor;

impl<F, A, const NUM_LIMBS: usize, const LIMB_BITS: usize> AotExecutor<F>
    for BaseAluExecutor<A, NUM_LIMBS, LIMB_BITS>
where
    F: PrimeField32,
{
    fn generate_aot_assembly(
        &self,
        pc: u32,
        inst: &Instruction<F>,
    ) -> Result<Option<String>, StaticProgramError> {
        let local_opcode = BaseAluOpcode::from_usize(inst.opcode.local_opcode_idx(self.offset));

        // Validate instruction format
        let d_val = inst.d.as_canonical_u32();
        let e_val = inst.e.as_canonical_u32();

        if d_val != RV32_REGISTER_AS {
            return Err(StaticProgramError::InvalidInstruction(pc));
        }

        let rd = inst.a.as_canonical_u32() as u8;
        let rs1 = inst.b.as_canonical_u32() as u8;
        let is_imm = e_val == RV32_IMM_AS;

        if !is_imm && e_val != RV32_REGISTER_AS {
            return Err(StaticProgramError::InvalidInstruction(pc));
        }

        let assembly = match (is_imm, local_opcode) {
            (true, BaseAluOpcode::ADD) => {
                let imm = inst.c.as_canonical_u32() as i32;
                generate_add_imm_assembly(rd, rs1, imm)
            }
            (false, BaseAluOpcode::ADD) => {
                let rs2 = inst.c.as_canonical_u32() as u8;
                generate_add_reg_assembly(rd, rs1, rs2)
            }
            (true, BaseAluOpcode::SUB) => {
                let imm = inst.c.as_canonical_u32() as i32;
                generate_sub_imm_assembly(rd, rs1, imm)
            }
            (false, BaseAluOpcode::SUB) => {
                let rs2 = inst.c.as_canonical_u32() as u8;
                generate_sub_reg_assembly(rd, rs1, rs2)
            }
            (true, BaseAluOpcode::XOR) => {
                let imm = inst.c.as_canonical_u32() as i32;
                generate_xor_imm_assembly(rd, rs1, imm)
            }
            (false, BaseAluOpcode::XOR) => {
                let rs2 = inst.c.as_canonical_u32() as u8;
                generate_xor_reg_assembly(rd, rs1, rs2)
            }
            (true, BaseAluOpcode::OR) => {
                let imm = inst.c.as_canonical_u32() as i32;
                generate_or_imm_assembly(rd, rs1, imm)
            }
            (false, BaseAluOpcode::OR) => {
                let rs2 = inst.c.as_canonical_u32() as u8;
                generate_or_reg_assembly(rd, rs1, rs2)
            }
            (true, BaseAluOpcode::AND) => {
                let imm = inst.c.as_canonical_u32() as i32;
                generate_and_imm_assembly(rd, rs1, imm)
            }
            (false, BaseAluOpcode::AND) => {
                let rs2 = inst.c.as_canonical_u32() as u8;
                generate_and_reg_assembly(rd, rs1, rs2)
            }
        };

        Ok(Some(assembly))
    }
}

// ADD operations
fn generate_add_imm_assembly(rd: u8, rs1: u8, imm: i32) -> String {
    if rs1 == 0 {
        format!(
            "    ; addi x{}, x{}, {} (rd=rs1+imm)\n    mov dword ptr [rbx + {}], {}  ; x{} = {}",
            rd,
            rs1,
            imm,
            rd * 4,
            imm,
            rd,
            imm
        )
    } else {
        format!(
            "    ; addi x{}, x{}, {} (rd=rs1+imm)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    add r15d, {}                  ; Add immediate\n    mov dword ptr [rbx + {}], r15d ; Store to x{}",
            rd, rs1, imm, rs1 * 4, rs1, imm, rd * 4, rd
        )
    }
}

fn generate_add_reg_assembly(rd: u8, rs1: u8, rs2: u8) -> String {
    if rs1 == 0 && rs2 == 0 {
        format!(
            "    ; add x{}, x{}, x{} (rd=rs1+rs2)\n    mov dword ptr [rbx + {}], 0  ; x{} = 0+0",
            rd,
            rs1,
            rs2,
            rd * 4,
            rd
        )
    } else if rs1 == 0 {
        format!(
            "    ; add x{}, x{}, x{} (rd=rs1+rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    mov dword ptr [rbx + {}], r15d ; Store to x{} (0+rs2 = rs2)",
            rd, rs1, rs2, rs2 * 4, rs2, rd * 4, rd
        )
    } else if rs2 == 0 {
        format!(
            "    ; add x{}, x{}, x{} (rd=rs1+rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    mov dword ptr [rbx + {}], r15d ; Store to x{} (rs1+0 = rs1)",
            rd, rs1, rs2, rs1 * 4, rs1, rd * 4, rd
        )
    } else {
        format!(
            "    ; add x{}, x{}, x{} (rd=rs1+rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    add r15d, dword ptr [rbx + {}] ; Add x{}\n    mov dword ptr [rbx + {}], r15d ; Store to x{}",
            rd, rs1, rs2, rs1 * 4, rs1, rs2 * 4, rs2, rd * 4, rd
        )
    }
}

// SUB operations
fn generate_sub_imm_assembly(rd: u8, rs1: u8, imm: i32) -> String {
    if rs1 == 0 {
        let neg_imm = (-imm) as u32;
        format!(
            "    ; subi x{}, x{}, {} (rd=rs1-imm)\n    mov dword ptr [rbx + {}], {}  ; x{} = 0-{}",
            rd,
            rs1,
            imm,
            rd * 4,
            neg_imm,
            rd,
            imm
        )
    } else {
        format!(
            "    ; subi x{}, x{}, {} (rd=rs1-imm)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    sub r15d, {}                  ; Subtract immediate\n    mov dword ptr [rbx + {}], r15d ; Store to x{}",
            rd, rs1, imm, rs1 * 4, rs1, imm, rd * 4, rd
        )
    }
}

fn generate_sub_reg_assembly(rd: u8, rs1: u8, rs2: u8) -> String {
    if rs1 == 0 && rs2 == 0 {
        format!(
            "    ; sub x{}, x{}, x{} (rd=rs1-rs2)\n    mov dword ptr [rbx + {}], 0  ; x{} = 0-0",
            rd,
            rs1,
            rs2,
            rd * 4,
            rd
        )
    } else if rs1 == 0 {
        format!(
            "    ; sub x{}, x{}, x{} (rd=rs1-rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    neg r15d                      ; Negate (0-rs2)\n    mov dword ptr [rbx + {}], r15d ; Store to x{}",
            rd, rs1, rs2, rs2 * 4, rs2, rd * 4, rd
        )
    } else if rs2 == 0 {
        format!(
            "    ; sub x{}, x{}, x{} (rd=rs1-rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    mov dword ptr [rbx + {}], r15d ; Store to x{} (rs1-0 = rs1)",
            rd, rs1, rs2, rs1 * 4, rs1, rd * 4, rd
        )
    } else {
        format!(
            "    ; sub x{}, x{}, x{} (rd=rs1-rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    sub r15d, dword ptr [rbx + {}] ; Subtract x{}\n    mov dword ptr [rbx + {}], r15d ; Store to x{}",
            rd, rs1, rs2, rs1 * 4, rs1, rs2 * 4, rs2, rd * 4, rd
        )
    }
}

// XOR operations
fn generate_xor_imm_assembly(rd: u8, rs1: u8, imm: i32) -> String {
    if rs1 == 0 {
        format!(
            "    ; xori x{}, x{}, {} (rd=rs1^imm)\n    mov dword ptr [rbx + {}], {}  ; x{} = 0^{}",
            rd,
            rs1,
            imm,
            rd * 4,
            imm as u32,
            rd,
            imm
        )
    } else {
        format!(
            "    ; xori x{}, x{}, {} (rd=rs1^imm)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    xor r15d, {}                  ; XOR immediate\n    mov dword ptr [rbx + {}], r15d ; Store to x{}",
            rd, rs1, imm, rs1 * 4, rs1, imm, rd * 4, rd
        )
    }
}

fn generate_xor_reg_assembly(rd: u8, rs1: u8, rs2: u8) -> String {
    if rs1 == 0 && rs2 == 0 {
        format!(
            "    ; xor x{}, x{}, x{} (rd=rs1^rs2)\n    mov dword ptr [rbx + {}], 0  ; x{} = 0^0",
            rd,
            rs1,
            rs2,
            rd * 4,
            rd
        )
    } else if rs1 == 0 {
        format!(
            "    ; xor x{}, x{}, x{} (rd=rs1^rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    mov dword ptr [rbx + {}], r15d ; Store to x{} (0^rs2 = rs2)",
            rd, rs1, rs2, rs2 * 4, rs2, rd * 4, rd
        )
    } else if rs2 == 0 {
        format!(
            "    ; xor x{}, x{}, x{} (rd=rs1^rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    mov dword ptr [rbx + {}], r15d ; Store to x{} (rs1^0 = rs1)",
            rd, rs1, rs2, rs1 * 4, rs1, rd * 4, rd
        )
    } else {
        format!(
            "    ; xor x{}, x{}, x{} (rd=rs1^rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    xor r15d, dword ptr [rbx + {}] ; XOR x{}\n    mov dword ptr [rbx + {}], r15d ; Store to x{}",
            rd, rs1, rs2, rs1 * 4, rs1, rs2 * 4, rs2, rd * 4, rd
        )
    }
}

// OR operations
fn generate_or_imm_assembly(rd: u8, rs1: u8, imm: i32) -> String {
    if rs1 == 0 {
        format!(
            "    ; ori x{}, x{}, {} (rd=rs1|imm)\n    mov dword ptr [rbx + {}], {}  ; x{} = 0|{}",
            rd,
            rs1,
            imm,
            rd * 4,
            imm as u32,
            rd,
            imm
        )
    } else {
        format!(
            "    ; ori x{}, x{}, {} (rd=rs1|imm)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    or r15d, {}                   ; OR immediate\n    mov dword ptr [rbx + {}], r15d ; Store to x{}",
            rd, rs1, imm, rs1 * 4, rs1, imm, rd * 4, rd
        )
    }
}

fn generate_or_reg_assembly(rd: u8, rs1: u8, rs2: u8) -> String {
    if rs1 == 0 && rs2 == 0 {
        format!(
            "    ; or x{}, x{}, x{} (rd=rs1|rs2)\n    mov dword ptr [rbx + {}], 0  ; x{} = 0|0",
            rd,
            rs1,
            rs2,
            rd * 4,
            rd
        )
    } else if rs1 == 0 {
        format!(
            "    ; or x{}, x{}, x{} (rd=rs1|rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    mov dword ptr [rbx + {}], r15d ; Store to x{} (0|rs2 = rs2)",
            rd, rs1, rs2, rs2 * 4, rs2, rd * 4, rd
        )
    } else if rs2 == 0 {
        format!(
            "    ; or x{}, x{}, x{} (rd=rs1|rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    mov dword ptr [rbx + {}], r15d ; Store to x{} (rs1|0 = rs1)",
            rd, rs1, rs2, rs1 * 4, rs1, rd * 4, rd
        )
    } else {
        format!(
            "    ; or x{}, x{}, x{} (rd=rs1|rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    or r15d, dword ptr [rbx + {}]  ; OR x{}\n    mov dword ptr [rbx + {}], r15d ; Store to x{}",
            rd, rs1, rs2, rs1 * 4, rs1, rs2 * 4, rs2, rd * 4, rd
        )
    }
}

// AND operations
fn generate_and_imm_assembly(rd: u8, rs1: u8, imm: i32) -> String {
    if rs1 == 0 {
        format!(
            "    ; andi x{}, x{}, {} (rd=rs1&imm)\n    mov dword ptr [rbx + {}], 0  ; x{} = 0&{} = 0",
            rd, rs1, imm, rd * 4, rd, imm
        )
    } else {
        format!(
            "    ; andi x{}, x{}, {} (rd=rs1&imm)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    and r15d, {}                  ; AND immediate\n    mov dword ptr [rbx + {}], r15d ; Store to x{}",
            rd, rs1, imm, rs1 * 4, rs1, imm, rd * 4, rd
        )
    }
}

fn generate_and_reg_assembly(rd: u8, rs1: u8, rs2: u8) -> String {
    if rs1 == 0 || rs2 == 0 {
        format!(
            "    ; and x{}, x{}, x{} (rd=rs1&rs2)\n    mov dword ptr [rbx + {}], 0  ; x{} = X&0 = 0",
            rd, rs1, rs2, rd * 4, rd
        )
    } else {
        format!(
            "    ; and x{}, x{}, x{} (rd=rs1&rs2)\n    mov r15d, dword ptr [rbx + {}] ; Load x{}\n    and r15d, dword ptr [rbx + {}] ; AND x{}\n    mov dword ptr [rbx + {}], r15d ; Store to x{}",
            rd, rs1, rs2, rs1 * 4, rs1, rs2 * 4, rs2, rd * 4, rd
        )
    }
}
