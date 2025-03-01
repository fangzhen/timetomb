use core::arch::asm;

use crate::driver::uart::PORT;

pub fn init_serial_port(io_permission_map: Option<&mut [u8]>) -> u8 {
    // Set io permission bit to allow uart port accssible in user space.
    match io_permission_map {
        Some(m) => {
            let idx = (PORT >> 3) as usize;
            m[idx] = 0;
        }
        _ => (),
    }
    return 0;
}

// Helper functions to perform I/O port operations
#[inline(always)]
pub fn inb(port: u16) -> u8 {
    let result: u8;
    unsafe {
        asm!(
        "in al, dx",
         out("al") result,
        in("dx") port);
    }
    result
}

#[inline(always)]
pub fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
        "out dx, al",
         in("al") value,
        in("dx") port);
    }
}
