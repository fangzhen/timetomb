pub mod apic;

use core::arch::asm;
use timetomb::driver::uart;

use log::info;

use timetomb::arch::x86_64::DescriptorTablePointer;

pub const PRESENT_FLAG: u16 = 1 << 15;

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct IdtEntry {
    pub offset15_0: u16,
    pub seg_selector: u16,
    pub flags: u16,
    pub offset31_16: u16,
    pub offset62_32: u32,
    pub reserved: u32,
}
pub static mut IDT_ENTRIES: [IdtEntry; 256] = [IdtEntry {
    offset15_0: 0,
    seg_selector: 0,
    flags: 0b0000_1110_0000_0000, // interrupt gate
    offset31_16: 0,
    offset62_32: 0,
    reserved: 0,
}; 256];

impl IdtEntry {
    /// Set the handler address for the IDT entry and sets the present bit.
    fn set_handler_addr(&mut self, addr: u64, dpl: u16) {
        self.offset15_0 = addr as u16;
        self.offset31_16 = (addr >> 16) as u16;
        self.offset62_32 = (addr >> 32) as u32;

        self.seg_selector = 1 << 3; //TODO(fangzhen) kernel cs; hardcoded.
        self.flags |= dpl << 13;

        self.flags |= PRESENT_FLAG;
    }
}

use core::fmt;

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct StackFrame {
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

impl fmt::Display for StackFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StackFrame {{\n  ss: {:#x},\n  rsp: {:#x},\n  rflags: {:#x},\n  cs: {:#x},\n  rip: {:#x}\n}}",
            self.ss, self.rsp, self.rflags, self.cs, self.rip
        )
    }
}

pub fn setup_idt() {
    unsafe {
        let idt_pointer = DescriptorTablePointer {
            limit: 256 * 16,
            base: &IDT_ENTRIES[0] as *const _ as u64,
        };
        asm!(
            "lidt [{idt_pointer}]",
            idt_pointer = in(reg) &idt_pointer,
        );

        IDT_ENTRIES[0].set_handler_addr(handle_de as u64, 0);
        IDT_ENTRIES[3].set_handler_addr(handle_bp as u64, 3);
        IDT_ENTRIES[6].set_handler_addr(handle_ud as u64, 3);
        IDT_ENTRIES[8].set_handler_addr(handle_df as u64, 0);
        IDT_ENTRIES[13].set_handler_addr(handle_gp as u64, 0);
        IDT_ENTRIES[14].set_handler_addr(handle_pf as u64, 0);

        // TODO vector should be set dynamically.
        IDT_ENTRIES[0x30].set_handler_addr(handle_timer as u64, 0);
        IDT_ENTRIES[0x34].set_handler_addr(handle_uart as u64, 0);

        IDT_ENTRIES[0xff].set_handler_addr(handle_spurious as u64, 0);
    }
}

pub extern "x86-interrupt" fn handle_de(_stack: StackFrame) {
    info!("DE exception.");
    panic!(); // Stop here for now. DE is a trap, we can't continue.
}

pub extern "x86-interrupt" fn handle_bp(_stack: StackFrame) {
    info!("BP exception.");
}

pub extern "x86-interrupt" fn handle_ud(_stack: StackFrame) {
    info!("UD exception.");
}

pub extern "x86-interrupt" fn handle_df(_stack: StackFrame, error_code: u64) {
    info!("DF exception with error code. {:#x}", error_code);
    panic!();
}

pub extern "x86-interrupt" fn handle_gp(stack: StackFrame, error_code: u64) {
    info!(
        "GP exception with error code. {:#x}, stack_frame: {}",
        error_code, stack
    );
    panic!();
}

pub extern "x86-interrupt" fn handle_pf(stack: StackFrame, error_code: u64) {
    info!(
        "Page Fault exception with error code: {:#x}, stack_frame: {}",
        error_code, stack
    );
    panic!();
}

pub extern "x86-interrupt" fn handle_timer(_stack: StackFrame) {
    apic::send_eoi();
    crate::arch::x86_64::process::timer_tick();
}

pub extern "x86-interrupt" fn handle_uart(_stack: StackFrame) {
    //TODO just to demo interrupt
    let c = uart::read_serial_port();
    if c as char == '\n' || c as char == '\r' {
        uart::write_serial_port('\n' as u8);
        uart::write_serial_port('\r' as u8);
    } else {
        uart::write_serial_port(c);
    }
    apic::send_eoi();
}

pub extern "x86-interrupt" fn handle_spurious(_stack: StackFrame) {}
