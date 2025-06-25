//! Process scheduler implementation
//!
//! This module provides different scheduling algorithms.
//! Currently implements a simple round-robin scheduler.

use crate::kernel::process::ProcessId;
use alloc::collections::VecDeque;
use core::fmt;

/// Trait for different scheduling algorithms
pub trait Scheduler {
    /// Add a process to the scheduler
    fn add_process(&mut self, pid: ProcessId);

    /// Remove a process from the scheduler
    fn remove_process(&mut self, pid: ProcessId);

    /// Select the next process to run
    fn schedule(&mut self) -> Option<ProcessId>;

    /// Notify scheduler that current process's time slice expired
    fn time_slice_expired(&mut self, current_pid: ProcessId);

    /// Get the number of processes in the scheduler
    fn process_count(&self) -> usize;

    /// Check if scheduler is empty
    fn is_empty(&self) -> bool {
        self.process_count() == 0
    }
}

/// Round-robin scheduler implementation
///
/// This scheduler maintains a circular queue of ready processes and
/// gives each process an equal time slice in a round-robin fashion.
#[derive(Debug)]
pub struct RoundRobinScheduler {
    /// Queue of ready processes
    ready_queue: VecDeque<ProcessId>,
    /// Currently running process (if any)
    current_process: Option<ProcessId>,
    /// Default time slice for each process (in timer ticks)
    default_time_slice: u32,
}

impl RoundRobinScheduler {
    /// Create a new round-robin scheduler
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
            current_process: None,
            default_time_slice: 10, // Default 10 timer ticks
        }
    }

    /// Create a new round-robin scheduler with custom time slice
    pub fn with_time_slice(time_slice: u32) -> Self {
        Self {
            ready_queue: VecDeque::new(),
            current_process: None,
            default_time_slice: time_slice,
        }
    }

    /// Set the default time slice
    pub fn set_time_slice(&mut self, time_slice: u32) {
        self.default_time_slice = time_slice;
    }

    /// Get the default time slice
    pub fn time_slice(&self) -> u32 {
        self.default_time_slice
    }

    /// Get the current running process
    pub fn current_process(&self) -> Option<ProcessId> {
        self.current_process
    }

    /// Get a reference to the ready queue (for debugging)
    pub fn ready_queue(&self) -> &VecDeque<ProcessId> {
        &self.ready_queue
    }
}

impl Scheduler for RoundRobinScheduler {
    fn add_process(&mut self, pid: ProcessId) {
        // Don't add if already in queue
        if !self.ready_queue.contains(&pid) {
            self.ready_queue.push_back(pid);
        }
    }

    fn remove_process(&mut self, pid: ProcessId) {
        // Remove from ready queue
        self.ready_queue.retain(|&p| p != pid);

        // Clear current process if it's the one being removed
        if self.current_process == Some(pid) {
            self.current_process = None;
        }
    }

    fn schedule(&mut self) -> Option<ProcessId> {
        // If there's a current process, it means we're doing a context switch
        // The current process should already be back in the ready queue if it's still runnable

        // Get the next process from the front of the queue
        if let Some(next_pid) = self.ready_queue.pop_front() {
            self.current_process = Some(next_pid);
            Some(next_pid)
        } else {
            self.current_process = None;
            None
        }
    }

    fn time_slice_expired(&mut self, current_pid: ProcessId) {
        // Move current process to back of queue (round-robin)
        if self.current_process == Some(current_pid) {
            self.ready_queue.push_back(current_pid);
            self.current_process = None;
        }
    }

    fn process_count(&self) -> usize {
        let queue_count = self.ready_queue.len();
        let current_count = if self.current_process.is_some() { 1 } else { 0 };
        queue_count + current_count
    }
}

impl fmt::Display for RoundRobinScheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RoundRobinScheduler[Ready: {}, Current: {:?}, TimeSlice: {}]",
            self.ready_queue.len(),
            self.current_process,
            self.default_time_slice
        )
    }
}
