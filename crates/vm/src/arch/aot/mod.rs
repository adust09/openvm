use p3_baby_bear::BabyBear;

use crate::{
    arch::{execution_mode::ExecutionCtx, VmExecState},
    system::memory::online::GuestMemory,
};

pub mod compiler;
pub mod executor;
pub mod ffi;
pub mod register_ops;
pub mod runtime;

pub use compiler::*;
pub use executor::*;
pub use ffi::*;
pub use register_ops::*;
pub use runtime::*;

pub type AotExecState = VmExecState<BabyBear, GuestMemory, ExecutionCtx>;
pub type AotHandler = unsafe extern "C" fn(
    pre_compute: *const u8,
    instret: *mut u64,
    pc: *mut u32,
    arg: u64,
    state: *mut AotExecState,
);
