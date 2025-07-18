//! Process Control Block (PCB) implementation
//!
//! The PCB contains all the information needed to manage a process,
//! including its state, context, memory information, and scheduling data.

use crate::kernel::process::context::ProcessContext;
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
    /// Process name (for debugging)
    name: String,
    /// Time slice remaining (for round-robin scheduling)
    time_slice: u32,
    /// Total CPU time used
    cpu_time: u64,
    /// Process creation time
    _creation_time: u64,
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

    /// Create a new PCB with specified mode (user or kernel)
    fn new_with_mode(
        pid: ProcessId,
        entry_point: usize,
        user_mode: bool,
    ) -> Result<Self, &'static str> {
        // Allocate stack space (simplified - in real implementation, this would use proper memory management)

        // Create a basic context first
        let mut context = ProcessContext::default();

        // Initialize the context properly for first execution
        // This sets up the context so that when restore() is called,
        // the process will start executing from its entry point
        unsafe {
            context.init_process_context(entry_point, user_mode);
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
