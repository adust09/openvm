use openvm_instructions::riscv::RV32_REGISTER_AS;

use super::AotExecState;
use crate::{arch::VmExecState, system::memory::online::GuestMemory};

pub trait AotRegisterOps {
    fn read_register(&self, reg: u8) -> u32;

    fn write_register(&mut self, reg: u8, value: u32);

    fn read_all_registers(&self, buffer: &mut [u32; 32]);

    fn write_all_registers(&mut self, buffer: &[u32; 32]);
}

impl AotRegisterOps
    for VmExecState<p3_baby_bear::BabyBear, GuestMemory, crate::arch::execution_mode::ExecutionCtx>
{
    fn read_register(&self, reg: u8) -> u32 {
        if reg == 0 {
            return 0; // x0 is always 0
        }
        let bytes: [u8; 4] = unsafe { self.vm_state.memory.read(RV32_REGISTER_AS, reg as u32) };
        u32::from_le_bytes(bytes)
    }

    fn write_register(&mut self, reg: u8, value: u32) {
        if reg == 0 {
            return; // x0 is read-only
        }
        let bytes = value.to_le_bytes();
        unsafe {
            self.vm_state
                .memory
                .write(RV32_REGISTER_AS, reg as u32, bytes)
        };
    }

    fn read_all_registers(&self, buffer: &mut [u32; 32]) {
        buffer[0] = 0; // x0 is always 0
        for i in 1..32 {
            buffer[i] = self.read_register(i as u8);
        }
    }

    fn write_all_registers(&mut self, buffer: &[u32; 32]) {
        // Skip x0, it's read-only
        for i in 1..32 {
            self.write_register(i as u8, buffer[i]);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn openvm_sync_registers_to_memory(
    state: *mut AotExecState,
    register_buffer: *const u32,
) {
    let state = &mut *state;
    let registers = std::slice::from_raw_parts(register_buffer, 32);

    for i in 1..32 {
        state.write_register(i as u8, registers[i]);
    }
}

#[no_mangle]
pub unsafe extern "C" fn openvm_sync_registers_from_memory(
    state: *const AotExecState,
    register_buffer: *mut u32,
) {
    let state = &*state;
    let registers = std::slice::from_raw_parts_mut(register_buffer, 32);

    registers[0] = 0; // x0 is always 0
    for i in 1..32 {
        registers[i] = state.read_register(i as u8);
    }
}
