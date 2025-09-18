use std::fmt::Write as _;

use openvm_instructions::{exe::VmExe, program::Program};
use openvm_stark_backend::p3_field::PrimeField32;

use super::AotExecutor;
use crate::arch::StaticProgramError;

/// AOT Compiler that generates x86-64 assembly implementing the complete RISC-V program
pub struct AotCompiler<F: PrimeField32> {
    assembly: String,
    _phantom: std::marker::PhantomData<F>,
}

impl<F: PrimeField32> AotCompiler<F> {
    pub fn new() -> Self {
        Self {
            assembly: String::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn compile<T>(
        &mut self,
        exe: &VmExe<F>,
        aot_executors: &[T],
    ) -> Result<String, StaticProgramError>
    where
        T: AotExecutor<F>,
    {
        self.assembly.clear();

        // Generate assembly header with register setup
        self.generate_header(exe);

        // Generate inline assembly for each instruction with PC labels
        self.generate_program_assembly(&exe.program, exe.pc_start, aot_executors)?;

        // Generate assembly footer
        self.generate_footer();

        Ok(self.assembly.clone())
    }

    fn generate_header(&mut self, exe: &VmExe<F>) {
        writeln!(&mut self.assembly, "; OpenVM AOT Generated Assembly").unwrap();
        writeln!(
            &mut self.assembly,
            "; Program: {} instructions",
            exe.program.num_defined_instructions()
        )
        .unwrap();
        writeln!(&mut self.assembly, "; Entry PC: 0x{:08x}", exe.pc_start).unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        writeln!(&mut self.assembly, "section .text").unwrap();
        writeln!(&mut self.assembly, "global openvm_aot_start").unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        // Entry point - set up registers
        writeln!(&mut self.assembly, "openvm_aot_start:").unwrap();
        writeln!(&mut self.assembly, "    ; Save host state").unwrap();
        writeln!(&mut self.assembly, "    push rbp").unwrap();
        writeln!(&mut self.assembly, "    push rbx").unwrap();
        writeln!(&mut self.assembly, "    push r12").unwrap();
        writeln!(&mut self.assembly, "    push r13").unwrap();
        writeln!(&mut self.assembly, "    push r14").unwrap();
        writeln!(&mut self.assembly, "    push r15").unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        // Register allocation
        // rbx = register base pointer (RISC-V registers stored as 32 consecutive 4-byte values)
        // r14 = instret (instruction counter)
        // r15 = temporary for computations
        writeln!(&mut self.assembly, "    ; Initialize VM state").unwrap();
        writeln!(
            &mut self.assembly,
            "    mov rbx, rdi              ; rbx = register base pointer"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    xor r14, r14              ; r14 = instret = 0"
        )
        .unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        // Initialize RISC-V registers to 0 (x0 must always be 0)
        writeln!(&mut self.assembly, "    ; Initialize RISC-V registers").unwrap();
        writeln!(
            &mut self.assembly,
            "    mov rcx, 32               ; 32 registers"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    xor rax, rax              ; zero value"
        )
        .unwrap();
        writeln!(&mut self.assembly, ".init_loop:").unwrap();
        writeln!(
            &mut self.assembly,
            "    mov dword ptr [rbx + rcx*4 - 4], eax"
        )
        .unwrap();
        writeln!(&mut self.assembly, "    loop .init_loop").unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        // Jump to entry point
        writeln!(&mut self.assembly, "    ; Jump to program entry point").unwrap();
        writeln!(&mut self.assembly, "    jmp .pc_{:08x}", exe.pc_start).unwrap();
        writeln!(&mut self.assembly, "").unwrap();
    }

    /// Generate inline assembly for each instruction
    fn generate_program_assembly<T>(
        &mut self,
        program: &Program<F>,
        _start_pc: u32,
        aot_executors: &[T],
    ) -> Result<(), StaticProgramError>
    where
        T: AotExecutor<F>,
    {
        for (pc, instruction, _debug_info) in program.enumerate_by_pc() {
            writeln!(&mut self.assembly, ".pc_{:08x}:", pc).unwrap();

            // Try to find an AOT executor for this instruction
            let mut aot_assembly = None;
            for executor in aot_executors {
                if let Some(assembly) = executor.generate_aot_assembly(pc, &instruction)? {
                    aot_assembly = Some(assembly);
                    break;
                }
            }

            if let Some(assembly) = aot_assembly {
                // Write the AOT assembly directly
                writeln!(&mut self.assembly, "{}", assembly).unwrap();
                writeln!(
                    &mut self.assembly,
                    "    inc r14                   ; instret++"
                )
                .unwrap();
                writeln!(
                    &mut self.assembly,
                    "    jmp .pc_{:08x}           ; Jump to next instruction",
                    pc + 4
                )
                .unwrap();
            } else {
                writeln!(
                    &mut self.assembly,
                    "    ; No AOT implementation - fallback to interpreter"
                )
                .unwrap();
                writeln!(
                    &mut self.assembly,
                    "    mov rax, {}              ; Return current PC",
                    pc
                )
                .unwrap();
                writeln!(&mut self.assembly, "    jmp .execute_end").unwrap();
            }

            writeln!(&mut self.assembly, "").unwrap();
        }

        Ok(())
    }

    /// Generate assembly footer with proper exit handling
    fn generate_footer(&mut self) {
        writeln!(&mut self.assembly, ".execute_end:").unwrap();
        writeln!(&mut self.assembly, "    ; Restore host state and return").unwrap();
        writeln!(
            &mut self.assembly,
            "    ; rax contains the final PC or exit code"
        )
        .unwrap();
        writeln!(&mut self.assembly, "    pop r15").unwrap();
        writeln!(&mut self.assembly, "    pop r14").unwrap();
        writeln!(&mut self.assembly, "    pop r13").unwrap();
        writeln!(&mut self.assembly, "    pop r12").unwrap();
        writeln!(&mut self.assembly, "    pop rbx").unwrap();
        writeln!(&mut self.assembly, "    pop rbp").unwrap();
        writeln!(&mut self.assembly, "    ret").unwrap();
    }
}

pub fn compile_and_execute_aot<F: PrimeField32, T>(
    exe: &VmExe<F>,
    aot_executors: &[T],
) -> Result<String, StaticProgramError>
where
    T: AotExecutor<F>,
{
    let mut compiler = AotCompiler::new();
    compiler.compile(exe, aot_executors)
}
