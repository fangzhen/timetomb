//! Process context management
//!
//! This module handles saving and restoring CPU context during process switches.
//! The context includes all CPU registers that need to be preserved.

use core::fmt;

use timetomb::kernel::mm::PAGE_SIZE;

use crate::{
    arch::x86_64::{
        mm::CR3_ADDR,
        process::context_switch_asm::{process_entry_wrapper, user_process_entry_wrapper},
    },
    kernel::mm::slab::kmalloc,
};

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
    /// The key insight is that when restore_context is called, it should appear as if
    /// the process was previously running and is now being resumed.
    ///
    /// # Safety
    /// This function is unsafe because it manipulates raw memory addresses.
    pub unsafe fn init_process_context(
        &mut self,
        entry_point: usize,
        user_mode: bool,
        kernel_stack_base: usize,
        user_stack_base: usize,
    ) {
        self.rsp = kernel_stack_base as u64;
        self.r12 = user_stack_base as u64;
        self.r13 = self as *const ProcessContext as u64; // fake pt_regs

        // For new processes, we need to set up the context so that when restore_context
        // is called, it jumps to a wrapper function that will then call the actual entry point.
        // This simulates the process being "resumed" from a previous context switch.
        if user_mode {
            self.rip = user_process_entry_wrapper as usize as u64;
        } else {
            self.rip = process_entry_wrapper as usize as u64;
        }

        // Store the actual entry point in a register that the wrapper can use.
        // Save it in callee-saved register (rbx).
        self.rbx = entry_point as u64;

        // Set up flags (enable interrupts)
        self.rflags = 0x202; // IF (Interrupt Flag) set

        // Both user process and kernel process init to kernel segments,
        // since user process need to sysret from kernel space.
        self.cs = 0x08; // Kernel code segment (GDT entry 1)
        self.ss = 0x10; // Kernel data segment (GDT entry 2)
        self.ds = 0x10;
        self.es = 0x10;
        self.fs = 0x10;
        self.gs = 0x10;

        // CR3 will be set by the memory manager
        unsafe { self.cr3 = CR3_ADDR as u64 };
    }

    /// Allocate stack space for the process
    fn allocate_stack(size: usize) -> Result<usize, &'static str> {
        let addr = kmalloc(size);
        let stack_base = addr + size - 8; // Leave space for alignment
        // Set up stack - align to 16 bytes and leave space for initial setup
        let aligned_stack = (stack_base - 16) & !0xF;
        Ok(aligned_stack)
    }

    /// Allocate new stacks for a forked process
    /// This creates new kernel and user stacks while preserving the current context
    pub unsafe fn allocate_new_stacks(&mut self) -> Result<(), &'static str> {
        let kernel_stack_size = PAGE_SIZE * 2;
        let user_stack_size = PAGE_SIZE * 2;

        // Allocate new kernel stack
        let new_kernel_stack = Self::allocate_stack(kernel_stack_size)?;

        // Allocate new user stack
        let new_user_stack = Self::allocate_stack(user_stack_size)?;

        // Calculate the offset from old stack to new stack
        let old_kernel_stack_base = self.rsp & !(PAGE_SIZE as u64 - 1); // Align to page boundary
        let kernel_stack_offset = new_kernel_stack as i64 - old_kernel_stack_base as i64;

        let old_user_stack_base = self.r12 & !(PAGE_SIZE as u64 - 1); // Align to page boundary
        let user_stack_offset = new_user_stack as i64 - old_user_stack_base as i64;

        // Update stack pointers with the offset
        self.rsp = (self.rsp as i64 + kernel_stack_offset) as u64;
        self.r12 = (self.r12 as i64 + user_stack_offset) as u64;

        // Update frame pointer if it points to the stack
        if self.rbp >= old_kernel_stack_base
            && self.rbp < old_kernel_stack_base + kernel_stack_size as u64
        {
            self.rbp = (self.rbp as i64 + kernel_stack_offset) as u64;
        }

        Ok(())
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

impl fmt::Display for ProcessContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Context[RIP: 0x{:016x}, RSP: 0x{:016x}, CR3: 0x{:016x}]",
            self.rip, self.rsp, self.cr3
        )
    }
}
