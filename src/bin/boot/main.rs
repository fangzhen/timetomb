#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

pub mod arch;

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
