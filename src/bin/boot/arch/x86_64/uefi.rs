use log::*;
use timetomb::arch::x86_64::ffi_shared::SETUP_HEADER_OFFSET;
use timetomb::arch::x86_64::mm::{MemoryDescriptor, p2l};

use crate::arch::uefi;
use crate::arch::uefi::spec;
use crate::arch::x86_64::mm as boot_arch_mm;
use core::arch::asm;
use timetomb::arch::x86_64::mm as arch_mm;
use timetomb::driver::uart;
use timetomb::kernel::logger::Logger;
use timetomb::kernel::mm::memblock;
use timetomb::kernel::mm::PAGE_SIZE;

use timetomb::arch::x86_64::SetupHeader;

static mut LOGGER: Logger = Logger { writer: None };

pub static mut SETUP_HEADER: SetupHeader = SetupHeader {
    mem_desc_count: 0,
    mem_desc: 0,
    cr3_addr: 0,
    pgtable_size: 0,
    identity_map_max_idx: 0,
    kernel_physical: 0,
    kernel_size: 0,
    kernel_stack_physical: 0,
    kernel_area_size: 0,
    rsdp_addr: 0,
};

unsafe extern "C" {
    static __vmkernel_start: u8;
    static __vmkernel_end: u8;
}

fn setup_logger() {
    unsafe {
        LOGGER.writer = Some(&mut (uart::UartOutput {}) as *mut uart::UartOutput);
        log::set_logger(&*(&raw const LOGGER))
            .map(|()| log::set_max_level(LevelFilter::Info))
            .ok();
    }
}

#[unsafe(no_mangle)]
pub extern "efiapi" fn efi_main(hdr: spec::Handle, system: *const spec::SystemTable) {
    unsafe {
        uefi::UEFI_SYSTEM_TAB = system;
    }
    let st = unsafe { system.as_ref().unwrap() };
    st.clear_screen();

    uart::init_serial_port(None);
    setup_logger();

    let uefi_map = uefi::exit_boot_services(hdr, st);
    // From now on, we don't use uefi boot services any more,
    // including memory management, text output etc.
    info!("Memory mapping after exit boot service:");
    arch_mm::print_memory_map(uefi_map);

    // We already runs on paging since paging is mandatory on x86_64
    // long mode. UEFI firmware had setup an identity mappend page table.
    // However, We need to manage page table by our kernel itself.
    arch_mm::generate_memblock_from_uefi_map(uefi_map);
    memblock::setup(p2l);
    memblock::print_memblocks();

    collect_boot_info();
    go_to_vmkernel(uefi_map);
}

fn collect_boot_info() {
    unsafe {
        SETUP_HEADER.rsdp_addr = uefi::get_rsdp_addr();
    }
}
pub fn go_to_vmkernel(uefi_map: &[MemoryDescriptor]) {
    unsafe {
        info!(
            "vmkernel initial address: {:x} - {:x}",
            &__vmkernel_start as *const u8 as usize, &__vmkernel_end as *const u8 as usize
        );
    }
    let vmkernel_start = unsafe { &__vmkernel_start as *const u8 as usize };
    let vmkernel_end = unsafe { &__vmkernel_end as *const u8 as usize };
    let kernel_size = vmkernel_end as usize - vmkernel_start as usize;
    let kernel_size_align = align_ceil(kernel_size, size_of::<usize>());
    let um_len = uefi_map.len();
    let um_start = uefi_map.as_ptr() as usize;
    let um_size = um_len * size_of::<MemoryDescriptor>();
    let um_size_align = align_ceil(um_size, size_of::<usize>());
    let size = kernel_size_align + um_size_align;

    let kernel_physical = memblock::allocate_physical_memory(0, size, PAGE_SIZE, 0);
    for i in 0..kernel_size {
        unsafe { *((kernel_physical + i) as *mut u8) = *((vmkernel_start + i) as *const u8) };
    }
    for i in 0..um_size {
        unsafe {
            *((kernel_physical + kernel_size_align + i) as *mut u8) = *((um_start + i) as *const u8)
        };
    }
    boot_arch_mm::paging_kernel_text_map(kernel_physical, size);
    unsafe {
        SETUP_HEADER.kernel_physical = kernel_physical;
        SETUP_HEADER.kernel_size = kernel_size;
        SETUP_HEADER.mem_desc = arch_mm::VMKERNEL_ENTRY_ADDRESS + kernel_size_align;
        SETUP_HEADER.mem_desc_count = um_len;
        SETUP_HEADER.kernel_area_size = size;
    }

    let setup_physical = kernel_physical + SETUP_HEADER_OFFSET;
    unsafe { *(setup_physical as *mut SetupHeader) = SETUP_HEADER };

    info!("Go to kernel");
    unsafe {
        // jump to kernel entry, which is virtual address of vmkernel_start.
        asm!("jmp {entry}",
             entry = in(reg) arch_mm::VMKERNEL_ENTRY_ADDRESS,
        )
    }
}

fn align_ceil(n: usize, a: usize) -> usize {
    return (n + a - 1) / a * a;
}
