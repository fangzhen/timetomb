//! High-level process management API
//!
//! This module provides a convenient API for process management operations
//! that can be used by other kernel subsystems.

use crate::kernel::process::pcb::{ProcessId, ProcessState};
use alloc::string::{String, ToString};

use super::manager::ProcessManager;

/// Process creation parameters
#[derive(Debug, Clone)]
pub struct ProcessCreateParams {
    /// Entry point address
    pub entry_point: usize,
    /// Process name (optional)
    pub name: Option<String>,
    /// Parent process ID (optional)
    pub parent_pid: Option<ProcessId>,
}

impl Default for ProcessCreateParams {
    fn default() -> Self {
        Self {
            entry_point: 0,
            name: None,
            parent_pid: None,
        }
    }
}

/// High-level process management API
pub struct ProcessApi;

impl ProcessApi {
    /// Create a new process
    pub fn create_process(
        entry_point: usize,
        name: Option<String>,
        user_mode: bool,
    ) -> Result<ProcessId, &'static str> {
        let pm = ProcessManager::get();
        let mut manager = pm.lock();
        let params = ProcessCreateParams {
            entry_point: entry_point,
            name,
            parent_pid: None,
        };
        let pid;
        if user_mode {
            pid = manager.create_process(params.entry_point)?;
        } else {
            pid = manager.create_kernel_process(params.entry_point)?;
        }

        // Set optional parameters
        if let Some(pcb) = manager.get_process_mut(pid) {
            if let Some(name) = params.name {
                pcb.set_name(name);
            }

            if let Some(parent_pid) = params.parent_pid {
                pcb.set_parent_pid(parent_pid);
            }

            // Set process to ready state
            pcb.set_state(ProcessState::Ready);
        }

        Ok(pid)
    }

    /// Terminate a process
    pub fn terminate_process(pid: ProcessId) -> Result<(), &'static str> {
        let pm = ProcessManager::get();
        let mut manager = pm.lock();
        manager.terminate_process(pid)
    }

    /// Get process information
    pub fn get_process_info(pid: ProcessId) -> Option<ProcessInfo> {
        let pm = ProcessManager::get();
        let manager = pm.lock();

        manager.get_process(pid).map(|pcb| ProcessInfo {
            pid: pcb.pid(),
            parent_pid: pcb.parent_pid(),
            state: pcb.state(),
            name: pcb.name().to_string(),
            cpu_time: pcb.cpu_time(),
            time_slice: pcb.time_slice(),
        })
    }

    /// List all processes
    pub fn list_processes() -> alloc::vec::Vec<ProcessInfo> {
        let pm = ProcessManager::get();
        let manager = pm.lock();

        let mut processes = alloc::vec::Vec::new();

        // This is a simplified implementation
        // In a real kernel, we'd iterate through the process table
        if let Some(current_pid) = manager.current_process() {
            if let Some(info) = Self::get_process_info(current_pid) {
                processes.push(info);
            }
        }

        processes
    }

    /// Set current process ID. Only used by idle process.
    pub fn set_current_process(pid: Option<ProcessId>) {
        let pm = ProcessManager::get();
        let mut manager = pm.lock();
        manager.set_current_process(pid)
    }

    /// Get current process ID
    pub fn current_process() -> Option<ProcessId> {
        let pm = ProcessManager::get();
        let manager = pm.lock();
        manager.current_process()
    }

    /// Yield CPU to next process
    pub fn yield_current() {
        let pm = ProcessManager::get();
        pm.lock().yield_cpu();
    }

    /// Block current process
    pub fn block_current(reason: &str) {
        let pm = ProcessManager::get();
        pm.lock().block_current(reason);
    }

    /// Force schedule next process
    pub fn schedule_next() {
        let pm = ProcessManager::get();
        pm.lock().schedule_next(ProcessState::Ready);
    }

    /// Sleep current process for a duration (simplified)
    pub fn sleep(_duration_ms: u64) {
        // In a real implementation, this would set up a timer
        // and block the process until the timer expires
        Self::block_current("sleeping");
    }

    /// Wait for a process to terminate
    pub fn wait_for_process(pid: ProcessId) -> Result<(), &'static str> {
        loop {
            if let Some(info) = Self::get_process_info(pid) {
                if info.state == ProcessState::Terminated {
                    return Ok(());
                }
            } else {
                return Err("Process not found");
            }

            // Yield and try again
            Self::yield_current();
        }
    }
}

/// Process information structure for external use
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: ProcessId,
    pub parent_pid: Option<ProcessId>,
    pub state: ProcessState,
    pub name: String,
    pub cpu_time: u64,
    pub time_slice: u32,
}

impl core::fmt::Display for ProcessInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}[{}]: {} - {} (CPU: {}, Slice: {})",
            self.name,
            self.pid,
            self.state,
            self.parent_pid
                .map_or("no parent".to_string(), |p| p.to_string()),
            self.cpu_time,
            self.time_slice
        )
    }
}
