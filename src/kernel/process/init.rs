//! Process subsystem initialization
//!
//! This module handles the initialization of the process management subsystem.

use crate::arch::x86_64::process as arch_process;
use crate::kernel::process::{ProcessApi, ProcessManager};
use alloc::string::String;

/// Initialize the process management subsystem
pub fn init() {
    ProcessManager::init();
    arch_process::init();
    create_idle_process().expect("Failed to create idle process");
    log::info!("Process management subsystem initialized");
}

/// Create the idle process that runs when no other processes are ready
pub fn create_idle_process() -> Result<(), &'static str> {
    let idle_pid =
        ProcessApi::create_process(idle_process as usize, Some(String::from("idle")), false)?;
    ProcessApi::set_current_process(Some(idle_pid));
    log::info!("Idle process created with PID: {:?}", idle_pid);

    Ok(())
}

/// The idle process function
/// This process runs when no other processes are ready to run
pub fn idle_process() -> ! {
    loop {
        // Halt the CPU until the next interrupt
        // This saves power when the system is idle
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack));
        }
    }
}

/// Start the first user process (init process)
pub fn start_init_process() -> Result<(), &'static str> {
    let init_pid =
        ProcessApi::create_process(init_process as usize, Some(String::from("init")), true)?;
    log::info!("Init process created with PID: {:?}", init_pid);

    // Start scheduling
    ProcessApi::schedule_next();

    Ok(())
}

/// The init process - first user process
fn init_process() {
    log::info!("Init process started!");

    // Create some test processes
    for i in 1..=3 {
        match ProcessApi::create_process(
            test_process as usize,
            Some(alloc::format!("test_process_{}", i)),
            true,
        ) {
            Ok(pid) => log::info!("Created test process {} with PID: {:?}", i, pid),
            Err(e) => log::error!("Failed to create test process {}: {}", i, e),
        }
    }

    // Main loop
    let mut counter = 0;
    loop {
        log::debug!("Init process running, counter: {}", counter);
        counter += 1;

        // Yield every 100 iterations
        if counter % 100 == 0 {
            ProcessApi::yield_current();
        }

        // Sleep for a bit
        if counter % 1000 == 0 {
            ProcessApi::sleep(100); // Sleep for 100ms
        }
    }
}

/// Test process function
fn test_process() {
    let current_pid = ProcessApi::current_process().unwrap_or(crate::kernel::process::ProcessId(0));
    log::info!("Test process {:?} started!", current_pid);

    let mut counter = 0;
    loop {
        counter += 1;

        if counter % 50 == 0 {
            log::debug!(
                "Test process {:?} running, counter: {}",
                current_pid,
                counter
            );
        }

        // Yield every 10 iterations
        if counter % 10 == 0 {
            ProcessApi::yield_current();
        }

        // Terminate after 1000 iterations
        if counter >= 1000 {
            log::info!("Test process {:?} terminating", current_pid);
            ProcessApi::terminate_process(current_pid).ok();
            break;
        }
    }
}

/// Enable preemptive scheduling by setting up timer interrupts
pub fn enable_preemption() {
    // This would typically set up a timer interrupt that calls
    // the scheduler at regular intervals
    log::info!("Preemptive scheduling enabled");
}

/// Get process subsystem statistics
pub fn get_stats() -> ProcessStats {
    let pm = ProcessManager::get();
    let manager = pm.lock();

    ProcessStats {
        total_processes: 0, // Would count processes in real implementation
        running_processes: if manager.current_process().is_some() {
            1
        } else {
            0
        },
        ready_processes: 0,   // Would count ready processes
        blocked_processes: 0, // Would count blocked processes
    }
}

/// Process subsystem statistics
#[derive(Debug, Default)]
pub struct ProcessStats {
    pub total_processes: usize,
    pub running_processes: usize,
    pub ready_processes: usize,
    pub blocked_processes: usize,
}

impl core::fmt::Display for ProcessStats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "ProcessStats[Total: {}, Running: {}, Ready: {}, Blocked: {}]",
            self.total_processes,
            self.running_processes,
            self.ready_processes,
            self.blocked_processes
        )
    }
}
