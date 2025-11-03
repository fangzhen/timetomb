//! Process context management
//!
//! This module handles saving and restoring CPU context during process switches.
//! The context includes all CPU registers that need to be preserved.

use timetomb::kernel::mm::PhysicalAddr;

/// CPU context for x86_64 architecture
/// This structure represents the complete CPU state that needs to be saved/restored
/// during context switches.
#[derive(Debug, Clone, Default)]
#[repr(C)]
pub struct ProcessContext {
    // General purpose registers
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,

    // Instruction pointer
    pub rip: u64,

    // Flags register
    pub rflags: u64,

    // Segment registers (for user space)
    pub cs: u64,
    pub ss: u64,
    pub ds: u64,
    pub es: u64,
    pub fs: u64,
    pub gs: u64,

    // Control registers
    pub cr3: u64, // Page table base
}

impl ProcessContext {
    /// Initialize a new process context for first execution
    ///
    /// This function sets up a context that can be used to start a new process.
    /// It prepares the stack and registers for the initial jump to the process entry point.
    pub fn new(
        entry_point: usize,
        rip: usize,
        kernel_stack_laddr: usize,
        user_stack_laddr: usize,
        pt_base: PhysicalAddr,
    ) -> Self {
        let mut context = Self::default();
        context.rsp = kernel_stack_laddr as u64;
        context.r12 = user_stack_laddr as u64;
        context.r13 = &context as *const Self as u64; // fake pt_regs

        // This simulates the process being "resumed" from a previous context switch.
        context.rip = rip as u64;

        // Store the actual entry point in a register that the wrapper can use.
        // Save it in callee-saved register (rbx).
        context.rbx = entry_point as u64;

        // Set up flags (enable interrupts)
        context.rflags = 0x202; // IF (Interrupt Flag) set

        // Both user process and kernel process init to kernel segments,
        // since user process need to sysret from kernel space.
        context.cs = 0x08; // Kernel code segment (GDT entry 1)
        context.ss = 0x10; // Kernel data segment (GDT entry 2)
        context.ds = 0x10;
        context.es = 0x10;
        context.fs = 0x10;
        context.gs = 0x10;

        context.cr3 = pt_base as u64;
        return context;
    }
}
