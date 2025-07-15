//! Process context management
//!
//! This module handles saving and restoring CPU context during process switches.
//! The context includes all CPU registers that need to be preserved.

use core::fmt;

/// CPU context for x86_64 architecture
/// This structure represents the complete CPU state that needs to be saved/restored
/// during context switches.
#[derive(Debug, Clone)]
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
    /// Create a new process context with initial values
    pub fn new(entry_point: usize, stack_pointer: usize) -> Self {
        Self {
            // Initialize general purpose registers to 0
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            rsp: stack_pointer as u64,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,

            // Set instruction pointer to entry point
            rip: entry_point as u64,

            // Set default flags (interrupts enabled)
            rflags: 0x202, // IF (Interrupt Flag) set

            // Set up segment registers for user mode
            cs: 0x20, // User code segment (GDT entry 4, RPL 3)
            ss: 0x18, // User data segment (GDT entry 3, RPL 3)
            ds: 0x18, // User data segment
            es: 0x18, // User data segment
            fs: 0x18, // User data segment
            gs: 0x18, // User data segment

            // Initialize CR3 to kernel page table for now
            cr3: 0, // Will be set by memory manager
        }
    }

    /// Create a kernel context (for kernel threads)
    pub fn new_kernel(entry_point: usize, stack_pointer: usize) -> Self {
        let mut ctx = Self::new(entry_point, stack_pointer);

        // Set kernel segment selectors
        ctx.cs = 0x08; // Kernel code segment (GDT entry 1)
        ctx.ss = 0x10; // Kernel data segment (GDT entry 2)
        ctx.ds = 0x10;
        ctx.es = 0x10;
        ctx.fs = 0x10;
        ctx.gs = 0x10;

        ctx
    }

    /// Set the instruction pointer
    pub fn set_instruction_pointer(&mut self, rip: u64) {
        self.rip = rip;
    }

    /// Set the stack pointer
    pub fn set_stack_pointer(&mut self, rsp: u64) {
        self.rsp = rsp;
    }

    /// Set the page table base (CR3)
    pub fn set_page_table(&mut self, cr3: u64) {
        self.cr3 = cr3;
    }

    /// Get the instruction pointer
    pub fn instruction_pointer(&self) -> u64 {
        self.rip
    }

    /// Get the stack pointer
    pub fn stack_pointer(&self) -> u64 {
        self.rsp
    }

    /// Get the page table base
    pub fn page_table(&self) -> u64 {
        self.cr3
    }
}
/// Save the current CPU context
/// Restore next context to the CPU
pub unsafe fn save_restore_context(current: Option<&mut ProcessContext>, next: &ProcessContext) {
    unsafe {
        if current.is_none() {
            crate::arch::x86_64::process::context_switch_asm::save_restore_context(
                0 as *mut ProcessContext,
                next as *const ProcessContext,
            );
        } else {
            crate::arch::x86_64::process::context_switch_asm::save_restore_context(
                current.unwrap() as *mut ProcessContext,
                next as *const ProcessContext,
            );
        }
    }
}

impl Default for ProcessContext {
    fn default() -> Self {
        Self {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            rsp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rip: 0,
            rflags: 0x202,
            cs: 0,
            ss: 0,
            ds: 0,
            es: 0,
            fs: 0,
            gs: 0,
            cr3: 0,
        }
    }
}

impl fmt::Display for ProcessContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Context[RIP: 0x{:016x}, RSP: 0x{:016x}, CR3: 0x{:016x}]",
            self.rip, self.rsp, self.cr3
        )
    }
}
