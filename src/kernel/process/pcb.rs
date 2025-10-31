//! Process Control Block (PCB) implementation
//!
//! The PCB contains all the information needed to manage a process,
//! including its state, context, memory information, and scheduling data.

use crate::{arch::x86_64::process::context::ProcessContext, kernel::mm::slab::kmalloc};
use alloc::string::String;
use bitflags::bitflags;
use core::fmt;
use timetomb::kernel::mm::PAGE_SIZE;

/// Process identifier type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(pub u64);

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PID({})", self.0)
    }
}

/// Process states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    /// Process is ready to run and waiting for CPU
    Ready,
    /// Process is currently running
    Running,
    /// Process has finished execution
    Terminated,
}

impl fmt::Display for ProcessState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessState::Ready => write!(f, "Ready"),
            ProcessState::Running => write!(f, "Running"),
            ProcessState::Terminated => write!(f, "Terminated"),
        }
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct ProcessFlags: u32 {
        const KERNEL_THREAD = 0b00000001;
    }
}

/// Process Control Block - contains all process information
#[derive(Debug)]
pub struct ProcessControlBlock {
    /// Process identifier
    pub pid: ProcessId,
    /// Parent process identifier
    pub parent_pid: Option<ProcessId>,
    /// Current process state
    pub state: ProcessState,
    /// CPU context (registers, stack pointer, etc.)
    pub context: ProcessContext,
    /// Process name (for debugging)
    pub name: String,
    /// Process flags
    pub flags: ProcessFlags,
    /// memory info
    pub memory_info: MemoryInfo,
}

#[derive(Debug)]
pub struct MemoryInfo {
    pub kernel_stack_base: usize,
    pub user_stack_base: usize,
}

impl ProcessControlBlock {
    /// Create a new PCB for a user process
    pub fn new_user(pid: ProcessId, entry_point: usize) -> Result<Self, &'static str> {
        // Allocate stack space
        let kernel_stack_size = PAGE_SIZE * 2;
        let user_stack_size = PAGE_SIZE * 2;
        let kernel_stack_base = Self::allocate_stack(kernel_stack_size).unwrap();
        let user_stack_base = Self::allocate_stack(user_stack_size).unwrap();
        let flags = ProcessFlags::empty();
        let mm_info = MemoryInfo {
            kernel_stack_base,
            user_stack_base,
        };

        let context = ProcessContext::new(entry_point, true, kernel_stack_base, user_stack_base);

        Ok(Self {
            pid,
            parent_pid: None,
            state: ProcessState::Ready,
            context,
            name: alloc::format!("user_process_{}", pid.0),
            flags: flags,
            memory_info: mm_info,
        })
    }

    /// Create a new PCB for a kernel process
    pub fn new_kernel(pid: ProcessId, entry_point: usize) -> Result<Self, &'static str> {
        let kernel_stack_size = PAGE_SIZE * 2;
        let kernel_stack_base = Self::allocate_stack(kernel_stack_size).unwrap();
        let user_stack_base = 0;
        let flags = ProcessFlags::KERNEL_THREAD;

        let mm_info = MemoryInfo {
            kernel_stack_base,
            user_stack_base,
        };

        let context = ProcessContext::new(entry_point, false, kernel_stack_base, user_stack_base);
        Ok(Self {
            pid,
            parent_pid: None,
            state: ProcessState::Ready,
            context,
            name: alloc::format!("kernel_process_{}", pid.0),
            flags: flags,
            memory_info: mm_info,
        })
    }

    /// Allocate stack space for the process
    fn allocate_stack(size: usize) -> Result<usize, &'static str> {
        let addr = kmalloc(size);
        let stack_base = addr + size - 8; // Leave space for alignment

        // Set up stack - align to 16 bytes and leave space for initial setup
        let aligned_stack = (stack_base - 16) & !0xF;
        Ok(aligned_stack)
    }

    /// Create a new PCB by forking from an existing process
    pub fn fork_from(
        child_pid: ProcessId,
        parent: &ProcessControlBlock,
    ) -> Result<Self, &'static str> {
        //let parent_kernel_base = parent.memory_info.kernel_stack_base;
        //let parent_user_base = parent.memory_info.user_stack_base;
        // Allocate stack space
        let kernel_stack_size = PAGE_SIZE * 2;
        let user_stack_size = PAGE_SIZE * 2;

        let kernel_stack_base =
            Self::allocate_stack(kernel_stack_size).unwrap() + kernel_stack_size;
        let user_stack_base;
        if parent.flags.contains(ProcessFlags::KERNEL_THREAD) {
            user_stack_base = 0;
        } else {
            user_stack_base = Self::allocate_stack(user_stack_size).unwrap() + user_stack_size;
            // TODO(fangzhen) We should map parent and child stack to same virtual address.
            unsafe {
                core::ptr::copy(
                    (parent.memory_info.user_stack_base - user_stack_size) as *const u8,
                    (user_stack_base - user_stack_size) as *mut u8,
                    user_stack_size,
                )
            };
        }
        let mm_info = MemoryInfo {
            kernel_stack_base,
            user_stack_base,
        };

        // Clone the parent's context
        let child_context = parent.context.clone();

        Ok(Self {
            pid: child_pid,
            parent_pid: Some(parent.pid),
            state: ProcessState::Ready,
            context: child_context,
            name: alloc::format!("forked_from_{}", parent.pid.0),
            flags: parent.flags,
            memory_info: mm_info,
        })
    }
}
