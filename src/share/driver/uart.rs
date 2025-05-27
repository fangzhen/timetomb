use core::fmt;

#[cfg(target_arch = "x86_64")]
use crate::arch::x86_64::uart as arch_uart;
#[cfg(target_arch = "x86_64")]
use crate::arch::x86_64::uart::{inb, outb};

//TODO(fangzhen) some code is x86 specific.

// Base address of the first serial port's hardware registers
pub const PORT: u16 = 0x3F8;

// Initialize the serial port
pub fn init_serial_port(io_permission_map: Option<&mut [u8]>) -> u8 {
    outb(PORT + 1, 0x01); // Ensable interrupts: data avaliable
    outb(PORT + 3, 0x80); // Enable DLAB (set baud rate divisor)
    outb(PORT + 0, 0x03); // Set divisor to 3 (lo byte) 38400 baud
    outb(PORT + 1, 0x00); //                  (hi byte)
    outb(PORT + 3, 0x03); // 8 bits, no parity, one stop bit
    outb(PORT + 2, 0xC7); // Enable FIFO, clear them, with 14-byte threshold
    outb(PORT + 4, 0x0B); // IRQs enabled, RTS/DSR set
    outb(PORT + 4, 0x1E); // Set in loopback mode, test the serial chip
    outb(PORT + 0, 0xAE); // Test serial chip (send byte 0xAE and check if serial returns same byte)

    // Check if serial is faulty (i.e: not same byte as sent)
    if inb(PORT + 0) != 0xAE {
        return 1;
    }

    // If serial is not faulty set it in normal operation mode
    // (not-loopback with IRQs enabled and OUT#1 and OUT#2 bits enabled)
    outb(PORT + 4, 0x0F);
    arch_uart::init_serial_port(io_permission_map);
    return 0;
}

// Read a byte from the serial port
pub fn read_serial_port() -> u8 {
    // Wait for data to be ready
    while (inb(PORT + 5) & 1) == 0 {}

    // Read the byte from the data register
    inb(PORT)
}

// Write a byte to the serial port
pub fn write_serial_port(byte: u8) {
    // Wait for the transmitter to be empty
    while (inb(PORT + 5) & 0x20) == 0 {}

    // Write the byte to the data register
    outb(PORT, byte);
}

pub struct UartOutput {}

impl fmt::Write for UartOutput {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &byte in s.as_bytes() {
            write_serial_port(byte);
        }
        Ok(())
    }
}

// Main function to demonstrate the usage of the serial port driver
/*
fn main() {
    // Initialize the serial port
    init_serial_port();

    // Write a string to the serial port
    let string = "Hello, Serial Port!";
    for &byte in string.as_bytes() {
        write_serial_port(byte);
    }
    // Read bytes from the serial port and print them
    for _ in 0..string.len() {
        let byte = read_serial_port();
        print!("{}", byte as char);
    }
}
*/
