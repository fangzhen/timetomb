//! Process management module

pub mod manager;
pub mod pcb;
pub mod scheduler;

pub use manager::ProcessManager;
pub use pcb::{ProcessControlBlock, ProcessId, ProcessState};
pub use scheduler::{RoundRobinScheduler, Scheduler};

use crate::arch::x86_64::syscall::syscall_to_kernelspace;

use crate::arch::x86_64::syscall::pt_regs::PtRegs;

/// Create a new kernel process
pub fn create_kernel_process(entry_point: usize) -> Result<ProcessId, &'static str> {
    let pm = ProcessManager::get();
    let mut manager = pm.lock();
    let pid = manager.create_kernel_process(entry_point)?;

    Ok(pid)
}
/// Fork a new user process from current
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
pub fn yield_process() {
    let pm = ProcessManager::get();
    pm.lock().schedule_next(ProcessState::Ready);
}

/// Initialize the process management subsystem.
pub fn init() {
    ProcessManager::init();
    let pm = ProcessManager::get();
    let mut manager = pm.lock();
    let kernel_init_pid = manager.setup_kernel_as_process();
    log::info!(
        "Setup initial kernel process as PID: {:?} done.",
        kernel_init_pid
    );
    log::info!("Process management subsystem initialized");
}

/// Start the first user process (init process)
pub fn create_user_init() -> Result<(), &'static str> {
    let pm = ProcessManager::get();
    let mut manager = pm.lock();

    let init_pid = manager.create_user_process(user_init_entry as usize)?;
    log::info!("Init process created with PID: {:?}", init_pid);
    Ok(())
}

/// The init process - first user process
fn user_init_entry() {
    log::info!("Init process started!");
    let pid = syscall_to_kernelspace(2); // fork
    if pid != 0 {
        log::info!("Created child user process: {}", pid);
        for i in 0..10 {
            log::info!("Init process running, counter: {}", i);
            syscall_to_kernelspace(3); // yield process
        }
    } else {
        log::info!("Child user process running");
    }
}

pub fn idle_entry() -> ! {
    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) };
    }
}
