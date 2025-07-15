//! Process Control Block (PCB) implementation
//!
//! The PCB contains all the information needed to manage a process,
//! including its state, context, memory information, and scheduling data.

use crate::kernel::{mm::slab::kmalloc, process::context::ProcessContext};
use alloc::string::String;
use core::fmt;

/// Process identifier type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(pub u32);

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
    /// Virtual memory space information
    memory_info: ProcessMemoryInfo,
    /// Process name (for debugging)
    name: String,
    /// Time slice remaining (for round-robin scheduling)
    time_slice: u32,
    /// Total CPU time used
    cpu_time: u64,
    /// Process creation time
    _creation_time: u64,
}

/// Memory information for a process
#[derive(Debug)]
pub struct ProcessMemoryInfo {
    /// Virtual address space start
    pub vaddr_start: usize,
    /// Virtual address space size
    pub vaddr_size: usize,
    /// Stack pointer
    pub stack_pointer: usize,
    /// Stack size
    pub stack_size: usize,
    /// Heap start address
    pub heap_start: usize,
    /// Heap size
    pub heap_size: usize,
    /// Code segment start
    pub code_start: usize,
    /// Code segment size
    pub code_size: usize,
}

impl ProcessControlBlock {
    /// Create a new PCB for a user process
    pub fn new(
        pid: ProcessId,
        entry_point: usize,
        stack_size: usize,
    ) -> Result<Self, &'static str> {
        Self::new_with_mode(pid, entry_point, stack_size, true)
    }

    /// Create a new PCB for a kernel process
    pub fn new_kernel(
        pid: ProcessId,
        entry_point: usize,
        stack_size: usize,
    ) -> Result<Self, &'static str> {
        Self::new_with_mode(pid, entry_point, stack_size, false)
    }

    /// Create a new PCB with specified mode (user or kernel)
    fn new_with_mode(
        pid: ProcessId,
        entry_point: usize,
        stack_size: usize,
        user_mode: bool,
    ) -> Result<Self, &'static str> {
        // Allocate stack space (simplified - in real implementation, this would use proper memory management)
        let stack_base = Self::allocate_stack(stack_size)?;
        let stack_pointer = stack_base + stack_size - 8; // Leave space for alignment

        // Create a basic context first
        let mut context = if user_mode {
            ProcessContext::new(entry_point, stack_pointer)
        } else {
            ProcessContext::new_kernel(entry_point, stack_pointer)
        };

        // Initialize the context properly for first execution
        // This sets up the context so that when restore() is called,
        // the process will start executing from its entry point
        unsafe {
            crate::arch::x86_64::process::context_switch_asm::init_process_context(
                &mut context as *mut ProcessContext,
                entry_point,
                stack_pointer,
                user_mode,
            );
        }

        let memory_info = ProcessMemoryInfo {
            vaddr_start: if user_mode { 0x400000 } else { 0x1000000 }, // Different address spaces
            vaddr_size: 0x100000,                                      // 1MB virtual space for now
            stack_pointer,
            stack_size,
            heap_start: if user_mode { 0x500000 } else { 0x1100000 },
            heap_size: 0,
            code_start: entry_point,
            code_size: 0x1000, // Assume 4KB for now
        };

        Ok(Self {
            pid,
            parent_pid: None,
            state: ProcessState::Ready,
            priority: ProcessPriority::Normal,
            context,
            memory_info,
            name: alloc::format!(
                "{}_process_{}",
                if user_mode { "user" } else { "kernel" },
                pid.0
            ),
            time_slice: 10, // Default time slice
            cpu_time: 0,
            _creation_time: Self::get_current_time(),
        })
    }

    /// Allocate stack space for the process
    fn allocate_stack(size: usize) -> Result<usize, &'static str> {
        let addr = kmalloc(size);
        Ok(addr)
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

    pub fn memory_info(&self) -> &ProcessMemoryInfo {
        &self.memory_info
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
