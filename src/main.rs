#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(sync_unsafe_cell)]

extern crate alloc;
use crate::arch::x86_64::instruction_wrappers;
use crate::arch::x86_64::interrupt;
use crate::arch::x86_64::mm as arch_mm;
use crate::kernel::process;
use arch::x86_64::mm::direct_map_p2l;
use arch::x86_64::mm::init::TSS_WITH_IO_MAP;
use arch::x86_64::syscall;
use core::panic::PanicInfo;
use kernel::mm::memblock;
use kernel::mm::paging;
use kernel::mm::physical;
use kernel::mm::slab;
use kernel::process::idle;
use timetomb::arch::x86_64::mm as share_mm;
use timetomb::arch::x86_64::SetupHeader;
use timetomb::driver::uart;
use timetomb::kernel::logger::Logger;

pub mod arch;
pub mod driver;
pub mod head;
pub mod kernel;
pub mod library;

static mut LOGGER: Logger = Logger { writer: None };
unsafe extern "C" {
    static _setup_header: u8;
    static _stack_bottom: u8;
    static _stack_top: u8;
}
pub static mut SETUP_HEADER: *const SetupHeader =
    unsafe { &_setup_header as *const u8 as *const SetupHeader };

pub static mut INITIAL_KERNEL_STACK: *const u8 = unsafe { &_stack_top as *const u8 };

//TODO (redundant code with boot)
fn setup_logger() {
    unsafe {
        LOGGER.writer = Some(&mut (uart::UartOutput {}) as *mut uart::UartOutput);
        log::set_logger(&*(&raw const LOGGER))
            .map(|()| log::set_max_level(log::LevelFilter::Info))
            .ok();
    }
}

// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn main() -> ! {
    core::arch::naked_asm!("lea rsp, [_stack_top]", "call {_main}", _main = sym _main)
}

extern "C" fn _main() -> ! {
    setup_logger();
    arch_mm::init::setup_gdt();
    log::info!("After setup gdt.");
    //TODO(fangzhen) Don't use io_permission_map
    uart::init_serial_port(Some(unsafe {
        &mut *(&raw mut TSS_WITH_IO_MAP.io_permission_map)
    }));

    let setup_header: &SetupHeader = unsafe { SETUP_HEADER.as_ref().unwrap() };
    paging::init_paging(setup_header);
    let uefi_map = unsafe {
        core::slice::from_raw_parts(
            direct_map_p2l(setup_header.mem_desc_physical) as *const _,
            setup_header.mem_desc_count,
        )
    };

    share_mm::print_physical_map(uefi_map);

    memblock::generate_memblock_from_physical_map(uefi_map);
    memblock::print_memblocks();

    // memory management init
    physical::init_page_allocator(unsafe { &*(&raw const memblock::ALL_MEMBLOCKS) }, unsafe {
        &*(&raw const memblock::USED_MEMBLOCKS)
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

    // Start the process management system
    crate::kernel::process::init();
    syscall::syscall_init();
    log::info!("Syscall init done");
    _ = crate::kernel::process::create_user_init();
    create_test_kernel_thread();
    process::yield_current();
    idle();
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

fn create_test_kernel_thread() {
    log::info!("Testing process management system");

    match process::create_kernel_process(test_kernel_thread_entry as usize) {
        Ok(pid) => {
            log::info!("Created test kernel thread with PID: {:?}", pid);
        }
        Err(e) => {
            log::error!("Failed to create test kernel thread: {}", e);
        }
    }
}

fn test_kernel_thread_entry() {
    log::info!("[Test kernel thread]: running!");

    process::yield_current();
    idle();
}
