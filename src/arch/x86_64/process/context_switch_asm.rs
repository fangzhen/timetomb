//! Low-level assembly context switching functions for x86_64
//!
//! This module provides the core assembly routines for saving and restoring
//! CPU context during process switches.

use crate::{
    arch::x86_64::{syscall::syscall_to_kernelspace, syscall::sysret_to_userspace},
    kernel::process::{ProcessContext, ProcessManager},
};
use core::arch::naked_asm;

/// Save current context, restore next context and switch to it.
///
/// This function saves the current CPU state to the provided context structure.
/// This function loads the CPU state from the provided context and jumps to it.
///
/// # Safety
/// This function is unsafe because it directly accesses memory through a raw pointer.
/// The caller must ensure that `context` points to valid memory.
/// This function is unsafe because it directly manipulates CPU registers and jumps
/// to arbitrary code. The caller must ensure that:
/// - The context contains valid register values
/// - The RIP points to valid executable code
/// - The RSP points to valid stack memory
#[unsafe(naked)]
pub unsafe extern "C" fn save_restore_context(
    current_context: *mut ProcessContext,
    next_context: *const ProcessContext,
) {
    naked_asm!(
        // Skip saving if current_context is null
        "test rdi, rdi",
        "jz 3f",
        // Save general purpose registers
        "mov [rdi + 0x00], rax",
        "mov [rdi + 0x08], rbx",
        "mov [rdi + 0x10], rcx",
        "mov [rdi + 0x18], rdx",
        "mov [rdi + 0x20], rsi",
        "mov [rdi + 0x28], rdi",
        "mov [rdi + 0x30], rbp",
        "mov [rdi + 0x38], rsp",
        "mov [rdi + 0x40], r8",
        "mov [rdi + 0x48], r9",
        "mov [rdi + 0x50], r10",
        "mov [rdi + 0x58], r11",
        "mov [rdi + 0x60], r12",
        "mov [rdi + 0x68], r13",
        "mov [rdi + 0x70], r14",
        "mov [rdi + 0x78], r15",
        // Save lable 2: as RIP
        "lea rax, [2f]",
        "mov [rdi + 0x80], rax", // Save RIP
        // Save RFLAGS
        "pushfq",
        "pop rax",
        "mov [rdi + 0x88], rax", // Save RFLAGS
        // Save segment registers
        "mov ax, cs",
        "mov [rdi + 0x90], rax",
        "mov ax, ss",
        "mov [rdi + 0x98], rax",
        "mov ax, ds",
        "mov [rdi + 0xa0], rax",
        "mov ax, es",
        "mov [rdi + 0xa8], rax",
        "mov ax, fs",
        "mov [rdi + 0xb0], rax",
        "mov ax, gs",
        "mov [rdi + 0xb8], rax",
        // Save CR3
        "mov rax, cr3",
        "mov [rdi + 0xc0], rax",
        //
        // RESTORE and jump to new process
        "test rsi, rsi", // If no next context, just return.
        "jz 2f",
        "3:",
        // Load CR3 first (page table)
        "mov rax, [rsi + 0xc0]", // Load CR3
        "mov cr3, rax",
        // Load segment registers
        "mov rax, [rsi + 0x90]",
        "mov cs, ax",
        "mov rax, [rsi + 0x98]",
        "mov ss, ax",
        "mov rax, [rsi + 0xa0]",
        "mov ds, ax",
        "mov rax, [rsi + 0xa8]",
        "mov es, ax",
        "mov rax, [rsi + 0xb0]",
        "mov fs, ax",
        "mov rax, [rsi + 0xb8]",
        "mov gs, ax",
        // Load RFLAGS
        "mov rax, [rsi + 0x88]",
        "push rax",
        "popfq",
        // Load general purpose registers
        "mov rax, [rsi + 0x00]",
        "mov rbx, [rsi + 0x08]",
        "mov rcx, [rsi + 0x10]",
        "mov rdx, [rsi + 0x18]",
        "mov rdi, [rsi + 0x28]",
        "mov rbp, [rsi + 0x30]",
        "mov rsp, [rsi + 0x38]",
        "mov r8,  [rsi + 0x40]",
        "mov r9,  [rsi + 0x48]",
        "mov r10, [rsi + 0x50]",
        "mov r11, [rsi + 0x58]",
        "mov r12, [rsi + 0x60]",
        "mov r13, [rsi + 0x68]",
        "mov r14, [rsi + 0x70]",
        "mov r15, [rsi + 0x78]",
        // Load RIP and jump
        "mov rax, [rsi + 0x80]", // Load RIP
        "mov rsi, [rsi + 0x20]", // Load RSI last
        "jmp rax",               // Jump to RIP
        "2:",
        "ret"
    );
}

/// Entry wrapper for new processes
///
/// This function is called when a new process is first scheduled.
/// It receives the actual entry point in RBX and jumps to it.
/// This simulates the process being resumed from a context switch.
#[unsafe(naked)]
pub unsafe extern "C" fn process_entry_wrapper() {
    naked_asm!(
        // Call simulate_schedule_end to ensure process manager is unlocked
        "call simulate_schedule_end",
        // RBX contains the actual entry point
        // Set up a clean stack frame and jump to it.
        "xor rbp, rbp", // Clear frame pointer
        "push rbp",     // Push null frame pointer (for stack unwinding)
        "and rsp, -16", // align rsp to 16 bytes
        "mov rbp, rsp", // Set up frame pointer
        "call rbx",
        "call process_end",
        "2: jmp 2b" // Infinite loop as fallback
    );
}
/// Entry wrapper for new processes
///
/// This function is called when a new process is first scheduled.
/// It receives the actual entry point in RBX and jumps to it.
/// This simulates the process being resumed from a context switch.
#[unsafe(naked)]
pub unsafe extern "C" fn user_process_entry_wrapper() {
    naked_asm!(
        // Call simulate_schedule_end to ensure process manager is unlocked
        "call simulate_schedule_end",
        // param to sysret_to_user
        "mov rdi, rbx",  // user entry point
        "xor rbp, rbp", // Clear frame pointer
        "push rbp",     // Push null frame pointer (for stack unwinding)
        "and r12, -16", // user stack rsp is stored in R12, align to 16 bytes
        "mov rbp, r12", // Set up frame pointer
        "lea rax, [3f]", // user process return to label 3: process_end() syscall
        "sub r12, 8",
        "mov [r12], rax",
        "mov rsi, r12",
        "mov rdx, r13",
        "call {sysret_to_user}",
        "3:",
        "mov rdi, 1",  // process exit
        "call {syscall_to_kernel}",
        "2: jmp 2b", // Infinite loop as fallback
        sysret_to_user = sym sysret_to_userspace,
        syscall_to_kernel = sym syscall_to_kernelspace,
    );
}

#[unsafe(naked)]
pub unsafe extern "C" fn fork_ret() {
    naked_asm!(
        // Call simulate_schedule_end to ensure process manager is unlocked
        "call simulate_schedule_end",
        // param to sysret_to_user
        "mov rdi, rbx",  // user entry point
        "mov rsi, r12",
        "mov rdx, r13",
        "call {sysret_to_user}",
        "2: jmp 2b", // Infinite loop as fallback
        sysret_to_user = sym sysret_to_userspace,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn simulate_schedule_end() {
    let pm = ProcessManager::get();
    if pm.is_locked() {
        unsafe { pm.force_unlock() };
    }
}

#[unsafe(no_mangle)]
pub fn process_end() {
    let mut pm = ProcessManager::get().lock();
    let pid = pm.current_process().unwrap();
    let _ = pm.terminate_process(pid);
    //TODO cleanup: free memory etc.
}
