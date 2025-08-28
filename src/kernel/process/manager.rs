use crate::arch::x86_64::mm as arch_mm;
use crate::arch::x86_64::process::context_switch_asm::{fork_ret, save_restore_context};
use crate::arch::x86_64::syscall::pt_regs::PtRegs;
use crate::kernel::process::ProcessContext;

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

    pub fn create_stub_kernel(&mut self) -> ProcessId {
        let pid = ProcessId(0);
        let pcb = ProcessControlBlock::new_stub(pid);

        self.processes.insert(pid, pcb);
        self.current_process = Some(pid);

        log::info!("Created new stub kernel process 0",);
        return pid;
    }
    /// Create a new user process
    pub fn create_process(&mut self, entry_point: usize) -> Result<ProcessId, &'static str> {
        let pid = self.next_pid;
        self.next_pid.0 += 1;

        let mut pcb = ProcessControlBlock::new(pid, entry_point)?;
        pcb.set_state(ProcessState::Ready);

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
    pub fn create_kernel_process(&mut self, entry_point: usize) -> Result<ProcessId, &'static str> {
        let pid = self.next_pid;
        self.next_pid.0 += 1;

        let mut pcb = ProcessControlBlock::new_kernel(pid, entry_point)?;
        pcb.set_state(ProcessState::Ready);

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
            if new_state == ProcessState::Ready {
                self.scheduler.add_process(current_pid);
            }
            let current_pcb = self.get_process_mut(current_pid).unwrap();
            if current_pcb.state() == ProcessState::Running {
                current_pcb.set_state(new_state);
            }
        }
        // Get next process from scheduler and switch to it
        let next_pid = self.schedule().unwrap();
        log::info!("Switching to process {:?}", next_pid);
        self.set_current_process(Some(next_pid));

        let next_pcb = self.get_process_mut(next_pid).unwrap();

        match next_pcb.state() {
            ProcessState::Ready => {
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

        // TODO check next pid is not current pid
        let next_context_ptr = next_pcb.context() as *const ProcessContext;
        let kernel_rsp = next_pcb.memory_info.kernel_stack_base;
        // This call will not return - it jumps directly to the next process
        let current_context_ptr;
        if current.is_none() {
            current_context_ptr = 0 as *mut ProcessContext;
        } else {
            current_context_ptr = (self
                .get_process_mut(current.unwrap())
                .unwrap()
                .context_mut()) as *mut ProcessContext;
        }
        unsafe {
            // update tss.rsp0 TODO(fangzhen) only needed for user process?
            arch_mm::init::TSS_WITH_IO_MAP.tss.rsps[0] = (kernel_rsp & 0xFFFFFFFF) as u32;
            arch_mm::init::TSS_WITH_IO_MAP.tss.rsps[1] = (kernel_rsp >> 32) as u32;
            save_restore_context(current_context_ptr, next_context_ptr);
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

    /// Fork the current process - creates a child process that is a copy of the parent
    /// Returns the child PID in the parent process and 0 in the child process
    pub fn fork(&mut self, regs: &PtRegs) -> Result<ProcessId, &'static str> {
        let current_pid = self.current_process().ok_or("No current process to fork")?;

        // Create new PID for child
        let child_pid = self.next_pid;
        self.next_pid.0 += 1;

        // Get the parent process
        let parent_pcb = self
            .get_process(current_pid)
            .ok_or("Current process not found")?;

        // Clone the parent's PCB
        let mut child_pcb = ProcessControlBlock::fork_from(child_pid, parent_pcb)?;
        child_pcb.set_parent_pid(current_pid);
        child_pcb.set_state(ProcessState::Ready);

        log::info!(
            "Forked process {:?} from parent {:?}",
            child_pid,
            current_pid
        );
        // Set return value for child process (0)
        let child_context = child_pcb.context_mut();
        child_context.rax = 0;
        let parent_pcb_ptr = parent_pcb as *const ProcessControlBlock;

        // Add child to processes and scheduler
        self.processes.insert(child_pid, child_pcb);
        self.scheduler.add_process(child_pid);

        let cp = self.get_process_mut(child_pid).unwrap();
        let child_pcb_ptr = cp as *mut ProcessControlBlock;
        unsafe { ProcessManager::fix_context(child_pcb_ptr, parent_pcb_ptr, regs) };

        Ok(child_pid)
    }

    unsafe extern "C" fn fix_context(
        child_pcb_ptr: *mut ProcessControlBlock,
        parent_pcb_ptr: *const ProcessControlBlock,
        regs: &PtRegs,
    ) {
        let child_pcb = unsafe { child_pcb_ptr.as_mut().unwrap() };
        let parent_pcb = unsafe { parent_pcb_ptr.as_ref().unwrap() };
        let child_kernel_base = child_pcb.memory_info.kernel_stack_base;
        let child_user_base = child_pcb.memory_info.user_stack_base;
        let child_context = child_pcb.context_mut();
        unsafe { save_restore_context(child_context, 0 as *const ProcessContext) };

        child_context.rsp = child_kernel_base as u64;
        child_context.rip = fork_ret as u64;
        child_context.rbx = regs.rcx;
        child_context.r13 = regs as *const PtRegs as u64;
        child_context.r12 =
            child_user_base as u64 - (parent_pcb.memory_info.user_stack_base as u64 - regs.rdx);
        // TODO simply regs.rdx (user rsp) when page table is setupped.
    }

    /// Terminate a process
    pub fn terminate_process(&mut self, pid: ProcessId) -> Result<(), &'static str> {
        if let Some(mut pcb) = self.processes.remove(&pid) {
            pcb.set_state(ProcessState::Terminated);
            self.scheduler.remove_process(pid);

            if self.current_process == Some(pid) {
                self.current_process = None;
            }
            self.schedule_next(ProcessState::Terminated);

            Ok(())
        } else {
            Err("Process not found")
        }
    }
}
