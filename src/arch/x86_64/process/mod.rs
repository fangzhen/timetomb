//! x86_64 specific process management
//!
//! This module provides x86_64 architecture specific implementations
//! for process management, including context switching.

pub mod context;
pub mod context_switch;

use crate::kernel::process;

pub fn timer_tick() {
    process::schedule_next()
}
