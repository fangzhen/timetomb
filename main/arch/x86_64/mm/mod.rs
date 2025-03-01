pub mod init;

use crate::arch::x86_64::mm as arch_mm;
use timetomb::arch::x86_64::mm as share_mm;
use timetomb::arch::x86_64::SetupHeader;

pub static mut CR3_ADDR: usize = 0;

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct GdtEntry {
    limit15_0: u16,
    base15_0: u16,

    base23_16: u8,
    access_byte: u8,
    limit19_16_and_flags: u8,
    base31_24: u8,
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Tss {
    pub reserved0: u32,
    pub rsps: [u32; 6],
    pub reserved1: u32,
    pub reserved2: u32,
    pub ists: [u32; 14],
    pub reserved3: u32,
    pub reserved4: u32,
    pub reserved5: u16,
    pub iopb: u16,
}

#[repr(C)]
pub struct TssWithIoMap {
    pub tss: Tss,
    pub io_permission_map: [u8; 8192],
}

pub fn init_setup(setup_header: &SetupHeader) {
    unsafe {
        CR3_ADDR = setup_header.cr3_addr;
    }
    arch_mm::init::setup_gdt();
    log::info!("After setup gdt.");

    // Remove identity map.
    // We asume the identity map is continous and has a hole after it.
    // TODO(fangzhen) release used memory
    let entries = share_mm::init::addr_to_page_entries(share_mm::p2l(setup_header.cr3_addr));
    for idx in 0..setup_header.identity_map_max_idx + 1 {
        entries[idx] = 0;
    }
}
