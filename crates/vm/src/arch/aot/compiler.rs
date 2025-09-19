use std::fmt::Write as _;

use openvm_instructions::{exe::VmExe, program::Program};
use openvm_stark_backend::p3_field::PrimeField32;

use super::executor::AotExecutor;
use crate::arch::StaticProgramError;

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
        writeln!(&mut self.assembly, "global openvm_aot_entry").unwrap();
        writeln!(&mut self.assembly, "extern openvm_aot_handler").unwrap();
        writeln!(&mut self.assembly, "extern openvm_sync_registers_to_memory").unwrap();
        writeln!(
            &mut self.assembly,
            "extern openvm_sync_registers_from_memory"
        )
        .unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        // Entry point with AotHandler signature
        writeln!(&mut self.assembly, "openvm_aot_start:").unwrap();
        writeln!(&mut self.assembly, "    ; Function signature:").unwrap();
        writeln!(&mut self.assembly, "    ; rdi = pre_compute ptr").unwrap();
        writeln!(&mut self.assembly, "    ; rsi = instret ptr").unwrap();
        writeln!(&mut self.assembly, "    ; rdx = pc ptr").unwrap();
        writeln!(&mut self.assembly, "    ; rcx = arg").unwrap();
        writeln!(&mut self.assembly, "    ; r8  = state ptr (AotExecState)").unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        writeln!(&mut self.assembly, "    ; Save callee-saved registers").unwrap();
        writeln!(&mut self.assembly, "    push rbp").unwrap();
        writeln!(&mut self.assembly, "    mov rbp, rsp").unwrap();
        writeln!(&mut self.assembly, "    push rbx").unwrap();
        writeln!(&mut self.assembly, "    push r12").unwrap();
        writeln!(&mut self.assembly, "    push r13").unwrap();
        writeln!(&mut self.assembly, "    push r14").unwrap();
        writeln!(&mut self.assembly, "    push r15").unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        // Allocate space for local register array (32 * 4 = 128 bytes)
        writeln!(&mut self.assembly, "    ; Allocate local register array").unwrap();
        writeln!(
            &mut self.assembly,
            "    sub rsp, 128              ; 32 registers * 4 bytes"
        )
        .unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        // Register allocation:
        // rbx = local register array base pointer (rsp)
        // r12 = pre_compute pointer
        // r13 = state pointer
        // r14 = pc pointer
        // r15 = temporary for computations
        writeln!(&mut self.assembly, "    ; Set up register allocation").unwrap();
        writeln!(
            &mut self.assembly,
            "    mov rbx, rsp              ; rbx = local register array"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    mov r12, rdi              ; r12 = pre_compute"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    mov r13, r8               ; r13 = state ptr"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    mov r14, rdx              ; r14 = pc ptr"
        )
        .unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        // Initialize local registers from guest memory
        writeln!(&mut self.assembly, "    ; Load registers from guest memory").unwrap();
        writeln!(
            &mut self.assembly,
            "    mov rdi, r13              ; state ptr"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    mov rsi, rbx              ; register buffer"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    call openvm_sync_registers_from_memory"
        )
        .unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        // Load initial PC and jump to it
        writeln!(&mut self.assembly, "    ; Load initial PC and jump").unwrap();
        writeln!(
            &mut self.assembly,
            "    mov eax, [r14]            ; Load PC"
        )
        .unwrap();
        writeln!(&mut self.assembly, "    cmp eax, {:08x}h", exe.pc_start).unwrap();
        writeln!(&mut self.assembly, "    jne .fallback_handler").unwrap();
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

                // Update instret through the pointer
                writeln!(&mut self.assembly, "    ; Update instret").unwrap();
                writeln!(
                    &mut self.assembly,
                    "    mov rax, [rsi]           ; Load instret"
                )
                .unwrap();
                writeln!(
                    &mut self.assembly,
                    "    inc rax                  ; Increment"
                )
                .unwrap();
                writeln!(
                    &mut self.assembly,
                    "    mov [rsi], rax           ; Store back"
                )
                .unwrap();

                // Update PC and check if we should continue
                writeln!(&mut self.assembly, "    ; Update PC").unwrap();
                writeln!(
                    &mut self.assembly,
                    "    mov dword [r14], {:08x}h  ; Update PC",
                    pc + 4
                )
                .unwrap();
                writeln!(
                    &mut self.assembly,
                    "    ; Check if next PC is within program bounds"
                )
                .unwrap();
                writeln!(
                    &mut self.assembly,
                    "    cmp dword [r14], {:08x}h  ; Compare with program end",
                    program.len() as u32 * 4
                )
                .unwrap();
                writeln!(
                    &mut self.assembly,
                    "    jae .exit                ; Exit if PC >= program end"
                )
                .unwrap();
                writeln!(
                    &mut self.assembly,
                    "    jmp .dispatch            ; Otherwise dispatch to next instruction"
                )
                .unwrap();
            } else {
                // No AOT implementation - call external handler
                writeln!(
                    &mut self.assembly,
                    "    ; No AOT implementation - call external handler"
                )
                .unwrap();
                writeln!(
                    &mut self.assembly,
                    "    mov dword [r14], {:08x}h  ; Update PC",
                    pc
                )
                .unwrap();
                writeln!(&mut self.assembly, "    jmp .fallback_handler").unwrap();
            }

            writeln!(&mut self.assembly, "").unwrap();
        }

        Ok(())
    }

    /// Generate assembly footer with proper exit handling
    fn generate_footer(&mut self) {
        // Fallback handler for unsupported instructions
        writeln!(&mut self.assembly, ".fallback_handler:").unwrap();
        writeln!(
            &mut self.assembly,
            "    ; Sync registers to guest memory before external call"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    push rdi                  ; Save pre_compute ptr"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    push rsi                  ; Save instret ptr"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    push rcx                  ; Save arg"
        )
        .unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        writeln!(
            &mut self.assembly,
            "    mov rdi, r13              ; state ptr"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    mov rsi, rbx              ; register buffer"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    call openvm_sync_registers_to_memory"
        )
        .unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        writeln!(
            &mut self.assembly,
            "    ; Call external handler for unsupported instruction"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    pop rcx                   ; Restore arg"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    pop rsi                   ; Restore instret ptr"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    pop rdi                   ; Restore pre_compute ptr"
        )
        .unwrap();
        writeln!(&mut self.assembly, "    mov rdx, r14              ; pc ptr").unwrap();
        writeln!(&mut self.assembly, "    mov r8, r13               ; state").unwrap();
        writeln!(&mut self.assembly, "    call openvm_aot_handler").unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        writeln!(
            &mut self.assembly,
            "    ; Sync registers from guest memory after external call"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    mov rdi, r13              ; state ptr"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    mov rsi, rbx              ; register buffer"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    call openvm_sync_registers_from_memory"
        )
        .unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        // After handler returns, check new PC and jump to it if within program
        writeln!(
            &mut self.assembly,
            "    ; Check if we should continue execution"
        )
        .unwrap();
        writeln!(
            &mut self.assembly,
            "    mov eax, [r14]            ; Load new PC"
        )
        .unwrap();
        writeln!(&mut self.assembly, "    jmp .exit").unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        writeln!(&mut self.assembly, ".dispatch:").unwrap();
        writeln!(&mut self.assembly, "    jmp .exit").unwrap();
        writeln!(&mut self.assembly, "").unwrap();

        writeln!(&mut self.assembly, ".exit:").unwrap();
        writeln!(&mut self.assembly, "    ; Clean up stack").unwrap();
        writeln!(
            &mut self.assembly,
            "    add rsp, 128              ; Remove register array"
        )
        .unwrap();
        writeln!(&mut self.assembly, "    ; Restore callee-saved registers").unwrap();
        writeln!(&mut self.assembly, "    pop r15").unwrap();
        writeln!(&mut self.assembly, "    pop r14").unwrap();
        writeln!(&mut self.assembly, "    pop r13").unwrap();
        writeln!(&mut self.assembly, "    pop r12").unwrap();
        writeln!(&mut self.assembly, "    pop rbx").unwrap();
        writeln!(&mut self.assembly, "    pop rbp").unwrap();
        writeln!(&mut self.assembly, "    ret").unwrap();
    }
}
