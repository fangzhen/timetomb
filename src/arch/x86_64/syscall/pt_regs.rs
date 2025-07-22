//! Process Trap Registers structure for x86_64
//!
//! This module defines the pt_regs structure that matches the order of registers
//! pushed in the syscall_entrypoint function.

/// Structure representing the saved user-space registers
///
/// The order of fields in this struct matches the order in which registers
/// are pushed in syscall_entrypoint.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct PtRegs {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64, // user rip
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64, // user rflags
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

impl PtRegs {
    /// Create a new PtRegs instance with all registers set to 0
    pub fn new() -> Self {
        Self::default()
    }
}
