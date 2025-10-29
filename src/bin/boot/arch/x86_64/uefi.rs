use log::*;
use timetomb::arch::x86_64::ffi_shared::SETUP_HEADER_OFFSET;
use timetomb::arch::x86_64::mm::MemoryDescriptor;

use crate::arch::uefi;
use crate::arch::uefi::spec;
use crate::arch::x86_64::mm as boot_arch_mm;
use core::arch::asm;
use timetomb::arch::x86_64::mm as arch_mm;
use timetomb::driver::uart;
use timetomb::kernel::logger::Logger;
use timetomb::kernel::mm::PAGE_SIZE;

use timetomb::arch::x86_64::SetupHeader;

static mut LOGGER: Logger = Logger { writer: None };

pub static mut SETUP_HEADER: SetupHeader = SetupHeader {
    mem_desc_count: 0,
    mem_desc: 0,
    mem_desc_physical: 0,
    mem_desc_size: 0,
    cr3_addr: 0,
    pgtable_size: 0,
    identity_map_max_idx: 0,
    kernel_physical: 0,
    kernel_size: 0,
    kernel_stack_physical: 0,
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

    unsafe {
        SETUP_HEADER.rsdp_addr = uefi::get_rsdp_addr();
    }

    unsafe {
        info!(
            "vmkernel initial address: {:x} - {:x}",
            &__vmkernel_start as *const u8 as usize, &__vmkernel_end as *const u8 as usize
        );
    }
    let vmkernel_start = unsafe { &__vmkernel_start as *const u8 as usize };
    let vmkernel_end = unsafe { &__vmkernel_end as *const u8 as usize };
    let kernel_size = vmkernel_end as usize - vmkernel_start as usize;

    let mut kernel_physical = 0;
    uefi::allocate_pages(
        st,
        spec::MemoryType::EfiLoaderData,
        (kernel_size + PAGE_SIZE - 1) / PAGE_SIZE,
        &mut kernel_physical as *mut _ as *mut u8,
    );

    let uefi_map = uefi::exit_boot_services(hdr, st);
    // From now on, we don't use uefi boot services any more,
    // including memory management, text output etc.

    info!("Memory mapping after exit boot service:");
    arch_mm::print_memory_map(uefi_map);
    let um_len = uefi_map.len();
    let um_start = uefi_map.as_ptr() as usize;
    let um_size = um_len * size_of::<MemoryDescriptor>();

    for i in 0..kernel_size {
        unsafe { *((kernel_physical + i) as *mut u8) = *((vmkernel_start + i) as *const u8) };
    }
    boot_arch_mm::paging_kernel_text_map(kernel_physical, kernel_size, um_start, um_size);
    unsafe {
        SETUP_HEADER.kernel_physical = kernel_physical;
        SETUP_HEADER.kernel_size = kernel_size;
        SETUP_HEADER.mem_desc = um_start + arch_mm::VMKERNEL_ENTRY_ADDRESS - kernel_physical;
        SETUP_HEADER.mem_desc_count = um_len;
        SETUP_HEADER.mem_desc_physical = um_start;
        SETUP_HEADER.mem_desc_size = um_size;
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
