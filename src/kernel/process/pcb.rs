//! Process Control Block (PCB) implementation
//!
//! The PCB contains all the information needed to manage a process,
//! including its state, context, memory information, and scheduling data.

use crate::kernel::{mm::slab::kmalloc, process::context::ProcessContext};
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
    /// Process is blocked waiting for some event
    Blocked,
    /// Process has finished execution
    Terminated,
}

impl fmt::Display for ProcessState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessState::Ready => write!(f, "Ready"),
            ProcessState::Running => write!(f, "Running"),
            ProcessState::Blocked => write!(f, "Blocked"),
            ProcessState::Terminated => write!(f, "Terminated"),
        }
    }
}

/// Process priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessPriority {
    High = 0,
    Normal = 1,
    Low = 2,
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
    pid: ProcessId,
    /// Parent process identifier
    parent_pid: Option<ProcessId>,
    /// Current process state
    state: ProcessState,
    /// Process priority
    priority: ProcessPriority,
    /// CPU context (registers, stack pointer, etc.)
    context: ProcessContext,
    /// Process name (for debugging)
    name: String,
    /// Time slice remaining (for round-robin scheduling)
    time_slice: u32,
    /// Process flags
    flags: ProcessFlags,
    /// Total CPU time used
    cpu_time: u64,
    /// Process creation time
    _creation_time: u64,
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
    pub fn new(pid: ProcessId, entry_point: usize) -> Result<Self, &'static str> {
        Self::new_with_mode(pid, entry_point, true)
    }

    /// Create a new PCB for a kernel process
    pub fn new_kernel(pid: ProcessId, entry_point: usize) -> Result<Self, &'static str> {
        Self::new_with_mode(pid, entry_point, false)
    }

    pub fn new_stub(pid: ProcessId) -> Self {
        let context = ProcessContext::default();
        let flags = ProcessFlags::KERNEL_THREAD;
        let kernel_stack_size = PAGE_SIZE * 2;
        let kernel_stack_base = Self::allocate_stack(kernel_stack_size).unwrap();
        let mm_info = MemoryInfo {
            kernel_stack_base: kernel_stack_base,
            user_stack_base: 0,
        };
        Self {
            pid,
            parent_pid: None,
            state: ProcessState::Running,
            priority: ProcessPriority::Normal,
            context,
            name: alloc::format!("process_{}", pid.0),
            time_slice: 10, // Default time slice
            cpu_time: 0,
            _creation_time: Self::get_current_time(),
            flags: flags,
            memory_info: mm_info,
        }
    }
    /// Create a new PCB with specified mode (user or kernel)
    fn new_with_mode(
        pid: ProcessId,
        entry_point: usize,
        user_mode: bool,
    ) -> Result<Self, &'static str> {
        // Allocate stack space
        let kernel_stack_size = PAGE_SIZE * 2;
        let user_stack_size = PAGE_SIZE * 2;
        let kernel_stack_base = Self::allocate_stack(kernel_stack_size).unwrap();
        let user_stack_base;
        let mut flags = ProcessFlags::empty();
        if user_mode {
            user_stack_base = Self::allocate_stack(user_stack_size).unwrap();
        } else {
            user_stack_base = 0;
            flags = flags | ProcessFlags::KERNEL_THREAD;
        }
        let mm_info = MemoryInfo {
            kernel_stack_base,
            user_stack_base,
        };

        // Create a basic context first
        let mut context = ProcessContext::default();

        // Initialize the context properly for first execution
        // This sets up the context so that when restore() is called,
        // the process will start executing from its entry point
        unsafe {
            context.init_process_context(
                entry_point,
                user_mode,
                kernel_stack_base,
                user_stack_base,
            );
        }

        Ok(Self {
            pid,
            parent_pid: None,
            state: ProcessState::Ready,
            priority: ProcessPriority::Normal,
            context,
            name: alloc::format!(
                "{}_process_{}",
                if user_mode { "user" } else { "kernel" },
                pid.0
            ),
            time_slice: 10, // Default time slice
            cpu_time: 0,
            _creation_time: Self::get_current_time(),
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
            priority: parent.priority,
            context: child_context,
            name: alloc::format!("forked_from_{}", parent.pid.0),
            time_slice: parent.time_slice,
            cpu_time: 0, // Reset CPU time for child
            _creation_time: Self::get_current_time(),
            flags: parent.flags,
            memory_info: mm_info,
        })
    }

    /// Get current time (simplified)
    fn get_current_time() -> u64 {
        // In a real implementation, this would read from a timer
        0
    }

    // Getters
    pub fn pid(&self) -> ProcessId {
        self.pid
    }

    pub fn parent_pid(&self) -> Option<ProcessId> {
        self.parent_pid
    }

    pub fn state(&self) -> ProcessState {
        self.state
    }

    pub fn priority(&self) -> ProcessPriority {
        self.priority
    }

    pub fn context(&self) -> &ProcessContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut ProcessContext {
        &mut self.context
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn time_slice(&self) -> u32 {
        self.time_slice
    }

    pub fn cpu_time(&self) -> u64 {
        self.cpu_time
    }

    // Setters
    pub fn set_state(&mut self, state: ProcessState) {
        self.state = state;
    }

    pub fn set_parent_pid(&mut self, parent_pid: ProcessId) {
        self.parent_pid = Some(parent_pid);
    }

    pub fn set_priority(&mut self, priority: ProcessPriority) {
        self.priority = priority;
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn set_time_slice(&mut self, time_slice: u32) {
        self.time_slice = time_slice;
    }

    pub fn add_cpu_time(&mut self, time: u64) {
        self.cpu_time += time;
    }

    pub fn decrease_time_slice(&mut self) -> bool {
        if self.time_slice > 0 {
            self.time_slice -= 1;
            self.time_slice == 0
        } else {
            true
        }
    }

    pub fn reset_time_slice(&mut self) {
        self.time_slice = 10; // Reset to default
    }
}

impl fmt::Display for ProcessControlBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PCB[{}]: {} - {} (Priority: {:?}, CPU Time: {})",
            self.pid, self.name, self.state, self.priority, self.cpu_time
        )
    }
}
