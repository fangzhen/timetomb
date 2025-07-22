//! Process management module

pub mod context;
pub mod manager;
pub mod pcb;
pub mod scheduler;

pub use context::ProcessContext;
pub use manager::ProcessManager;
pub use pcb::{ProcessControlBlock, ProcessId, ProcessState};
pub use scheduler::{RoundRobinScheduler, Scheduler};

use crate::arch::x86_64::syscall::syscall_to_kernelspace;
use alloc::string::String;
use alloc::string::ToString;

use crate::arch::x86_64::syscall::pt_regs::PtRegs;

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

pub fn setup_kernel_as_process(kernel_stack_base: usize) -> ProcessId {
    let pm = ProcessManager::get();
    let mut manager = pm.lock();
    return manager.create_stub_kernel(kernel_stack_base);
}
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

pub fn fork(regs: &PtRegs) -> ProcessId {
    let pm = ProcessManager::get();
    let mut manager = pm.lock();
    let pid = manager.fork(regs);
    return pid.unwrap();
}

/// Terminate a process
pub fn terminate_process(pid: ProcessId) -> Result<(), &'static str> {
    let pm = ProcessManager::get();
    let mut manager = pm.lock();
    manager.terminate_process(pid)
}

/// Yield CPU to next process
pub fn yield_current() {
    let pm = ProcessManager::get();
    pm.lock().yield_cpu();
}

/// Force schedule next process
pub fn schedule_next() {
    let pm = ProcessManager::get();
    pm.lock().schedule_next(ProcessState::Ready);
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

/// Initialize the process management subsystem.
pub fn init(kernel_stack_base: usize) {
    ProcessManager::init();
    let kernel_init_pid = setup_kernel_as_process(kernel_stack_base);
    log::info!(
        "Setup initial kernel process with PID: {:?} done.",
        kernel_init_pid
    );
    log::info!("Process management subsystem initialized");
}

pub fn idle() -> ! {
    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) };
    }
}

/// Start the first user process (init process)
pub fn start_user_init() -> Result<(), &'static str> {
    let init_pid = create_process(init_process as usize, Some(String::from("init")), true)?;
    log::info!("Init process created with PID: {:?}", init_pid);
    Ok(())
}

/// The init process - first user process
fn init_process() {
    log::info!("Init process started!");

    let pid = syscall_to_kernelspace(2); // fork
    if pid != 0 {
        log::info!("Created Child user process: {}", pid);
        for i in 0..10 {
            log::info!("Init process running, counter: {}", i);
            syscall_to_kernelspace(3); // yield process
        }
    } else {
        log::info!("Child user process running");
    }
}
