use std::ptr;

use openvm_instructions::exe::VmExe;
use p3_baby_bear::BabyBear;

use super::{runtime::AotRuntimeBuilder, AotExecState, AotExecutor};
use crate::{
    arch::{
        execution_mode::ExecutionCtx, AotCompiler, ExecutionError, Streams, SystemConfig,
        VmExecState, VmState,
    },
    system::memory::online::GuestMemory,
};

/// Execute a VM program using AOT compilation
pub fn execute_aot<T>(
    exe: &VmExe<BabyBear>,
    aot_executors: &[T],
    system_config: SystemConfig,
    initial_memory: GuestMemory,
) -> Result<(VmState<BabyBear, GuestMemory>, Streams<BabyBear>), ExecutionError>
where
    T: AotExecutor<BabyBear>,
{
    // Compile to assembly
    let mut compiler = AotCompiler::new();
    let assembly = compiler
        .compile(exe, aot_executors)
        .map_err(|_e| ExecutionError::Fail {
            pc: exe.pc_start,
            msg: "AOT compilation failed",
        })?;

    // Build runtime with default handler
    let runtime = AotRuntimeBuilder::new(assembly)
        .build()
        .map_err(|_| ExecutionError::Fail {
            pc: exe.pc_start,
            msg: "AOT runtime build failed",
        })?;

    // Get entry point
    let entry_point = runtime
        .get_entry_point()
        .map_err(|_| ExecutionError::Fail {
            pc: exe.pc_start,
            msg: "Failed to get AOT entry point",
        })?;

    // Set up execution state
    let ctx = ExecutionCtx::new(None);
    let mut state = VmExecState::new(
        VmState::new_with_defaults(
            0, // instret
            exe.pc_start,
            initial_memory,
            Streams::default(),
            0, // seed
            system_config.num_public_values,
        ),
        ctx,
    );

    // Set up parameters
    let mut instret: u64 = 0;
    let mut pc: u32 = exe.pc_start;
    let arg: u64 = 0;

    // Execute AOT code
    unsafe {
        entry_point(
            ptr::null(), // pre_compute
            &mut instret as *mut u64,
            &mut pc as *mut u32,
            arg,
            &mut state as *mut AotExecState,
        );
    }

    // Return final state
    Ok((state.vm_state, Streams::default()))
}

/// Wrapper for AOT execution with custom handler
pub struct AotExecutionContext {
    runtime: Option<super::runtime::AotRuntime>,
}

impl AotExecutionContext {
    pub fn new() -> Self {
        Self { runtime: None }
    }

    /// Compile and prepare for execution
    pub fn compile<T>(
        &mut self,
        exe: &VmExe<BabyBear>,
        aot_executors: &[T],
        handler_source: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: AotExecutor<BabyBear>,
    {
        // Compile to assembly
        let mut compiler = AotCompiler::new();
        let assembly = compiler.compile(exe, aot_executors)?;

        // Build runtime
        let mut builder = AotRuntimeBuilder::new(assembly);
        if let Some(source) = handler_source {
            builder = builder.with_handler_source(source);
        }

        self.runtime = Some(builder.build()?);
        Ok(())
    }

    /// Execute the compiled code
    pub fn execute(
        &self,
        initial_memory: GuestMemory,
        pc_start: u32,
    ) -> Result<(VmState<BabyBear, GuestMemory>, u64), Box<dyn std::error::Error>> {
        let runtime = self.runtime.as_ref().ok_or("No compiled code available")?;

        let entry_point = runtime.get_entry_point()?;

        // Set up state
        let ctx = ExecutionCtx::new(None);
        let mut state = VmExecState::new(
            VmState::new_with_defaults(
                0, // instret
                pc_start,
                initial_memory,
                Streams::default(),
                0, // seed
                0, // num_public_values
            ),
            ctx,
        );

        let mut instret: u64 = 0;
        let mut pc: u32 = pc_start;

        // Execute AOT code
        unsafe {
            entry_point(
                ptr::null(),
                &mut instret as *mut u64,
                &mut pc as *mut u32,
                0,
                &mut state as *mut AotExecState,
            );
        }

        Ok((state.vm_state, instret))
    }
}

/// Create a C-compatible handler function from a Rust closure
#[macro_export]
macro_rules! aot_handler {
    ($name:ident, $body:expr) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(
            pre_compute: *const u8,
            instret: *mut u64,
            pc: *mut u32,
            arg: u64,
            state: *mut $crate::arch::aot::AotExecState,
        ) {
            let handler: fn(
                *const u8,
                &mut u64,
                &mut u32,
                u64,
                &mut $crate::arch::aot::AotExecState,
            ) = $body;
            handler(pre_compute, &mut *instret, &mut *pc, arg, &mut *state);
        }
    };
}
