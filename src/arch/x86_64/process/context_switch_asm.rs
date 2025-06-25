//! Low-level assembly context switching functions for x86_64
//!
//! This module provides the core assembly routines for saving and restoring
//! CPU context during process switches.

use crate::{
    arch::x86_64::mm::CR3_ADDR,
    kernel::process::{ProcessContext, ProcessManager},
};
use core::arch::naked_asm;

/// Save current context without switching
///
/// This function saves the current CPU state to the provided context structure.
/// Unlike switch_context, this function returns to the caller.
///
/// # Safety
/// This function is unsafe because it directly accesses memory through a raw pointer.
/// The caller must ensure that `context` points to valid memory.
#[unsafe(naked)]
pub unsafe extern "C" fn save_context(context: *mut ProcessContext) {
    naked_asm!(
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
        // Save return address as RIP
        "mov rax, [rsp]",
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
        // Return to caller
        "ret"
    );
}

/// Restore context and jump to it
///
/// This function loads the CPU state from the provided context and jumps to it.
/// This function does not return.
///
/// # Safety
/// This function is unsafe because it directly manipulates CPU registers and jumps
/// to arbitrary code. The caller must ensure that:
/// - The context contains valid register values
/// - The RIP points to valid executable code
/// - The RSP points to valid stack memory
#[unsafe(naked)]
pub unsafe extern "C" fn restore_context(context: *const ProcessContext) {
    naked_asm!(
        // Load CR3 first (page table)
        "mov rax, [rdi + 0xc0]", // Load CR3
        "mov cr3, rax",
        // Load segment registers
        "mov rax, [rdi + 0xa0]",
        "mov ds, ax",
        "mov rax, [rdi + 0xa8]",
        "mov es, ax",
        "mov rax, [rdi + 0xb0]",
        "mov fs, ax",
        "mov rax, [rdi + 0xb8]",
        "mov gs, ax",
        // Load RFLAGS
        "mov rax, [rdi + 0x88]",
        "push rax",
        "popfq",
        // Load general purpose registers
        "mov rax, [rdi + 0x00]",
        "mov rbx, [rdi + 0x08]",
        "mov rcx, [rdi + 0x10]",
        "mov rdx, [rdi + 0x18]",
        "mov rsi, [rdi + 0x20]",
        "mov rbp, [rdi + 0x30]",
        "mov rsp, [rdi + 0x38]",
        "mov r8,  [rdi + 0x40]",
        "mov r9,  [rdi + 0x48]",
        "mov r10, [rdi + 0x50]",
        "mov r11, [rdi + 0x58]",
        "mov r12, [rdi + 0x60]",
        "mov r13, [rdi + 0x68]",
        "mov r14, [rdi + 0x70]",
        "mov r15, [rdi + 0x78]",
        // Load RIP and jump
        "mov rax, [rdi + 0x80]", // Load RIP
        "mov rdi, [rdi + 0x28]", // Load RDI last
        "jmp rax"                // Jump to RIP
    );
}

/// Initialize a new process context for first execution
///
/// This function sets up a context that can be used to start a new process.
/// It prepares the stack and registers for the initial jump to the process entry point.
/// The key insight is that when restore_context is called, it should appear as if
/// the process was previously running and is now being resumed.
///
/// # Safety
/// This function is unsafe because it manipulates raw memory addresses.
pub unsafe fn init_process_context(
    context: *mut ProcessContext,
    entry_point: usize,
    stack_top: usize,
    user_mode: bool,
) {
    let ctx = unsafe { &mut *context };

    // Clear all registers
    ctx.rax = 0;
    ctx.rbx = 0;
    ctx.rcx = 0;
    ctx.rdx = 0;
    ctx.rsi = 0;
    ctx.rdi = 0;
    ctx.rbp = 0;
    ctx.r8 = 0;
    ctx.r9 = 0;
    ctx.r10 = 0;
    ctx.r11 = 0;
    ctx.r12 = 0;
    ctx.r13 = 0;
    ctx.r14 = 0;
    ctx.r15 = 0;

    // Set up stack - align to 16 bytes and leave space for initial setup
    let aligned_stack = (stack_top - 16) & !0xF;
    ctx.rsp = aligned_stack as u64;

    // For new processes, we need to set up the context so that when restore_context
    // is called, it jumps to a wrapper function that will then call the actual entry point.
    // This simulates the process being "resumed" from a previous context switch.
    ctx.rip = process_entry_wrapper as usize as u64;

    // Store the actual entry point in a register that the wrapper can use.
    // Save it in callee-saved register (rbx).
    ctx.rbx = entry_point as u64;

    // Set up flags (enable interrupts)
    ctx.rflags = 0x202; // IF (Interrupt Flag) set

    if user_mode {
        // User mode segments
        ctx.cs = 0x20; // User code segment (GDT entry 4, RPL 3)
        ctx.ss = 0x18; // User data segment (GDT entry 3, RPL 3)
        ctx.ds = 0x18;
        ctx.es = 0x18;
        ctx.fs = 0x18;
        ctx.gs = 0x18;
    } else {
        // Kernel mode segments
        ctx.cs = 0x08; // Kernel code segment (GDT entry 1)
        ctx.ss = 0x10; // Kernel data segment (GDT entry 2)
        ctx.ds = 0x10;
        ctx.es = 0x10;
        ctx.fs = 0x10;
        ctx.gs = 0x10;
    }

    // CR3 will be set by the memory manager
    unsafe { ctx.cr3 = CR3_ADDR as u64 };
}

/// Entry wrapper for new processes
///
/// This function is called when a new process is first scheduled.
/// It receives the actual entry point in RDI and jumps to it.
/// This simulates the process being resumed from a context switch.
#[unsafe(naked)]
unsafe extern "C" fn process_entry_wrapper() {
    naked_asm!(
        // Call schedule_end to ensure process manager is unlocked
        "call schedule_end",
        // RDI contains the actual entry point
        // Set up a clean stack frame
        "xor rbp, rbp", // Clear frame pointer
        "push rbp",     // Push null frame pointer (for stack unwinding)
        "mov rbp, rsp", // Set up frame pointer
        // Jump to the actual entry point
        // RDI already contains the entry point address
        "and rsp, -16", //align rsp to 16 bytes
        "call rbx",
        // If the process returns, we should terminate it
        // For now, just halt
        "hlt",
        "2: jmp 2b" // Infinite loop as fallback
    );
}

#[unsafe(no_mangle)]
pub fn schedule_end() {
    let pm = ProcessManager::get();
    if pm.is_locked() {
        unsafe { pm.force_unlock() };
    }
}
