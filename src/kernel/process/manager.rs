use super::{ProcessControlBlock, ProcessId, ProcessState};
use super::{RoundRobinScheduler, Scheduler};
use alloc::collections::BTreeMap;
use spin::Once;

/// Global process manager instance
static PROCESS_MANAGER: Once<spin::Mutex<ProcessManager>> = Once::new();

#[derive(Debug, PartialEq)]
pub enum ProcessSwitchResult {
    /// Context switch completed successfully
    Success,
    /// No process to switch to
    NoProcess,
    /// Current process not found
    CurrentProcessNotFound,
    /// Next process not found
    NextProcessNotFound,
    /// Context switch failed
    Failed,
}

/// Process manager that handles all process-related operations
pub struct ProcessManager {
    current_process: Option<ProcessId>,
    next_pid: ProcessId,
    processes: BTreeMap<ProcessId, ProcessControlBlock>,
    scheduler: RoundRobinScheduler,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            current_process: None,
            next_pid: ProcessId(1), // PID 0 reserved for kernel
            processes: BTreeMap::new(),
            scheduler: RoundRobinScheduler::new(),
        }
    }

    /// Initialize the global process manager
    pub fn init() {
        PROCESS_MANAGER.call_once(|| spin::Mutex::new(ProcessManager::new()));
    }

    /// Get reference to the global process manager
    pub fn get() -> &'static spin::Mutex<ProcessManager> {
        PROCESS_MANAGER
            .get()
            .expect("Process manager not initialized")
    }

    /// Create a new user process
    pub fn create_process(
        &mut self,
        entry_point: usize,
        stack_size: usize,
    ) -> Result<ProcessId, &'static str> {
        let pid = self.next_pid;
        self.next_pid.0 += 1;

        let mut pcb = ProcessControlBlock::new(pid, entry_point, stack_size)?;
        // New processes start in New state and will be moved to Ready when first scheduled
        pcb.set_state(ProcessState::New);

        self.processes.insert(pid, pcb);
        self.scheduler.add_process(pid);

        log::info!(
            "Created new user process {:?} with entry point 0x{:x}",
            pid,
            entry_point
        );
        Ok(pid)
    }

    /// Create a new kernel process
    pub fn create_kernel_process(
        &mut self,
        entry_point: usize,
        stack_size: usize,
    ) -> Result<ProcessId, &'static str> {
        let pid = self.next_pid;
        self.next_pid.0 += 1;

        let mut pcb = ProcessControlBlock::new_kernel(pid, entry_point, stack_size)?;
        // New processes start in New state and will be moved to Ready when first scheduled
        pcb.set_state(ProcessState::New);

        self.processes.insert(pid, pcb);
        self.scheduler.add_process(pid);

        log::info!(
            "Created new kernel process {:?} with entry point 0x{:x}",
            pid,
            entry_point
        );
        Ok(pid)
    }

    /// Get the currently running process ID
    pub fn current_process(&self) -> Option<ProcessId> {
        self.current_process
    }

    /// Get a reference to a process by ID
    pub fn get_process(&self, pid: ProcessId) -> Option<&ProcessControlBlock> {
        self.processes.get(&pid)
    }

    /// Get a mutable reference to a process by ID
    pub fn get_process_mut(&mut self, pid: ProcessId) -> Option<&mut ProcessControlBlock> {
        self.processes.get_mut(&pid)
    }

    /// Schedule the next process to run
    pub fn schedule(&mut self) -> Option<ProcessId> {
        self.scheduler.schedule()
    }

    /// Switch to the next scheduled process
    pub fn schedule_next(&mut self, new_state: ProcessState) -> ProcessSwitchResult {
        let current = self.current_process();

        if let Some(current_pid) = current {
            if let Some(current_pcb) = self.get_process_mut(current_pid) {
                if current_pcb.state() == ProcessState::Running {
                    current_pcb.set_state(new_state);
                }
                let saved_context = crate::kernel::process::ProcessContext::save_current();
                *current_pcb.context_mut() = saved_context;
            }
        }

        // Get next process from scheduler and switch to it
        if let Some(next_pid) = self.schedule() {
            log::info!("Switching to process {:?}", next_pid);
            self.set_current_process(Some(next_pid));

            let next_pcb = self.get_process_mut(next_pid).unwrap();

            // Handle first-time execution vs. resuming
            match next_pcb.state() {
                ProcessState::New => {
                    log::info!("Starting new process {:?} for the first time", next_pid);
                    next_pcb.set_state(ProcessState::Running);
                    // For new processes, the context is already set up by init_process_context
                    // to simulate being resumed from a context switch
                }
                ProcessState::Ready => {
                    log::info!("Resuming process {:?}", next_pid);
                    next_pcb.set_state(ProcessState::Running);
                    // For ready processes, we're resuming from where they left off
                }
                _ => {
                    log::warn!(
                        "Attempting to schedule process {:?} in state {:?}",
                        next_pid,
                        next_pcb.state()
                    );
                    next_pcb.set_state(ProcessState::Running);
                }
            }

            let next_context = next_pcb.context().clone();

            // This call will not return - it jumps directly to the next process
            unsafe {
                next_context.restore();
            }

            // This line should never be reached
            ProcessSwitchResult::Success
        } else {
            ProcessSwitchResult::NoProcess
        }
    }

    /// Handle timer interrupt for preemptive scheduling
    pub fn timer_interrupt(&mut self) -> ProcessSwitchResult {
        if let Some(current_pid) = self.current_process() {
            if let Some(current_pcb) = self.get_process_mut(current_pid) {
                // Decrease time slice
                let time_expired = current_pcb.decrease_time_slice();

                if time_expired {
                    // Time slice expired, schedule next process
                    current_pcb.reset_time_slice();
                    return self.schedule_next(ProcessState::Ready);
                }
            }
        }

        ProcessSwitchResult::Success
    }

    /// Yield CPU voluntarily
    pub fn yield_cpu(&mut self) -> ProcessSwitchResult {
        self.schedule_next(ProcessState::Ready)
    }

    /// Block current process
    pub fn block_current(&mut self, _reason: &str) -> ProcessSwitchResult {
        self.schedule_next(ProcessState::Blocked)
    }

    /// Set the current running process
    pub fn set_current_process(&mut self, pid: Option<ProcessId>) {
        self.current_process = pid;
    }

    /// Terminate a process
    pub fn terminate_process(&mut self, pid: ProcessId) -> Result<(), &'static str> {
        if let Some(mut pcb) = self.processes.remove(&pid) {
            pcb.set_state(ProcessState::Terminated);
            self.scheduler.remove_process(pid);

            if self.current_process == Some(pid) {
                self.current_process = None;
            }

            Ok(())
        } else {
            Err("Process not found")
        }
    }
}
