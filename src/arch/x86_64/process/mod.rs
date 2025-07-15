//! x86_64 specific process management
//!
//! This module provides x86_64 architecture specific implementations
//! for process management, including context switching.

pub mod context_switch_asm;

pub fn timer_tick() {
    // TODO: Implement timer tick handling
    // This should handle process scheduling on timer interrupts
}

use crate::arch::x86_64::syscall;
use timetomb::kernel::mm::PAGE_SIZE;
use timetomb::kernel::mm::memblock;

/// Initialize x86_64 process management
pub fn init() {}

pub fn to_userspace_ret() {
    //TODO Only test for now. Migrate after process management.
    let mut user_stack_addr = memblock::allocate_memory(0, PAGE_SIZE, PAGE_SIZE, 0);
    user_stack_addr = user_stack_addr + PAGE_SIZE; // stack grows down

    log::info!(
        "TO user space: func addr: {:#x}, stack_addr: {:#x}, local var addr: {:#x}, global vars addr: {:#x}, allocated addr: {:#x}",
        echo as *const () as usize,
        user_stack_addr,
        &user_stack_addr as *const usize as usize,
        (&raw const memblock::ALL_MEMBLOCKS) as usize,
        //unsafe{&raw const ( memblock::ALL_MEMBLOCKS) as usize},
        0 //&uefi_map[0] as *const _ as usize
    );
    syscall::syscall_init();
    syscall::sysret_to_userspace(echo as *const () as usize, user_stack_addr);
}

fn echo() {
    log::info!("user space ^^!");
    syscall::syscall_to_kernelspace();
}
