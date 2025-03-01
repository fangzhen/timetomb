#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(sync_unsafe_cell)]
use crate::arch::x86_64::instruction_wrappers;
use crate::arch::x86_64::interrupt;
use crate::arch::x86_64::mm as arch_mm;
use arch::x86_64::mm::init::TSS_WITH_IO_MAP;
use arch::x86_64::process;
use core::panic::PanicInfo;
use core::ptr::addr_of;
use kernel::mm::physical;
use kernel::mm::slab;
use timetomb::arch::x86_64::mm as share_mm;
use timetomb::arch::x86_64::SetupHeader;
use timetomb::driver::uart;
use timetomb::kernel::logger::Logger;
use timetomb::kernel::mm::memblock;
use timetomb::kernel::mm::KERNEL_STACK_SIZE;

pub mod arch;
pub mod driver;
pub mod head;
pub mod kernel;
pub mod library;

static mut LOGGER: Logger = Logger { writer: None };
extern "C" {
    static _setup_header: u8;
}
pub static mut SETUP_HEADER: *const SetupHeader =
    unsafe { &_setup_header as *const u8 as *const SetupHeader };

//TODO (redundant code with boot)
fn setup_logger() {
    unsafe {
        LOGGER.writer = Some(&mut (uart::UartOutput {}) as *mut uart::UartOutput);
        log::set_logger(&*addr_of!(LOGGER))
            .map(|()| log::set_max_level(log::LevelFilter::Info))
            .ok();
    }
}

// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
#[no_mangle]
pub extern "C" fn main() -> ! {
    //TODO(fangzhen) make this global?
    let setup_header: &SetupHeader = unsafe { SETUP_HEADER.as_ref().unwrap() };
    uart::init_serial_port(Some(unsafe { &mut TSS_WITH_IO_MAP.io_permission_map }));
    setup_logger();
    arch_mm::init_setup(setup_header);
    log::info!("Kernel taking over!");

    let uefi_map = unsafe {
        core::slice::from_raw_parts(
            share_mm::p2l(setup_header.mem_desc) as *const _,
            setup_header.mem_desc_count,
        )
    };
    share_mm::print_memory_map(uefi_map);
    // Reserve memory used by pagetable
    share_mm::generate_memblock_from_uefi_map(uefi_map);
    memblock::add_used_memory(setup_header.cr3_addr, setup_header.pgtable_size, 0);
    memblock::add_used_memory(setup_header.kernel_physical, setup_header.kernel_size, 0);
    memblock::add_used_memory(setup_header.kernel_stack_physical, KERNEL_STACK_SIZE, 0);
    memblock::setup(share_mm::p2l);
    memblock::print_memblocks();

    // memory management init
    physical::init_page_allocator(unsafe { &*addr_of!(memblock::ALL_MEMBLOCKS) }, unsafe {
        &*addr_of!(memblock::USED_MEMBLOCKS)
    });
    physical::MEM_ZONE.print_buddy_status();
    slab::init_slab();
    slab::init_kmalloc();
    test_mm();

    // setup interrupt
    instruction_wrappers::cli();
    interrupt::setup_idt();
    log::info!("Setup initial idt done.");
    instruction_wrappers::sti();

    interrupt::apic::init_apic(setup_header.rsdp_addr);

    process::to_userspace_ret();
    process::idle();
}

fn test_mm() {
    physical::MEM_ZONE.print_buddy_status();
    log::info!("Test allocating pages");
    physical::allocate_pages(3);
    physical::allocate_pages(3);
    physical::allocate_pages(3);
    physical::allocate_pages(3);
    let page = physical::allocate_pages(3);
    log::info!("Test free pages");
    physical::free_pages(page.unwrap());

    slab::test_slab();
}
