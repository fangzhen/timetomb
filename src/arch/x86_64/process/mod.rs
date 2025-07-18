//! x86_64 specific process management
//!
//! This module provides x86_64 architecture specific implementations
//! for process management, including context switching.

pub mod context_switch_asm;

use crate::kernel::process::ProcessApi;

/// Initialize x86_64 process management
pub fn init() {}

pub fn timer_tick() {
    ProcessApi::schedule_next()
}
