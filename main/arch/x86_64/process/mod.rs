use crate::arch::x86_64::syscall;
use core::arch::asm;
use core::ptr::addr_of;
use timetomb::kernel::mm::memblock;
use timetomb::kernel::mm::PAGE_SIZE;

pub fn idle() -> ! {
    loop {
        unsafe { asm!("hlt") };
    }
}

pub fn to_userspace_ret() {
    //TODO Only test for now. Migrate after process management.
    let mut user_stack_addr = memblock::allocate_memory(0, PAGE_SIZE, PAGE_SIZE, 0);
    user_stack_addr = user_stack_addr + PAGE_SIZE; // stack grows down

    log::info!(
        "TO user space: func addr: {:#x}, stack_addr: {:#x}, local var addr: {:#x}, global vars addr: {:#x}, allocated addr: {:#x}",
        echo as *const () as usize,
        user_stack_addr,
        &user_stack_addr as *const usize as usize,
        addr_of!( memblock::ALL_MEMBLOCKS) as usize,
        //unsafe{addr_of!( memblock::ALL_MEMBLOCKS) as usize},
        0
        //&uefi_map[0] as *const _ as usize
    );
    syscall::syscall_init();
    syscall::sysret_to_userspace(echo as *const () as usize, user_stack_addr);
}

fn echo() {
    log::info!("user space ^^!");
    syscall::syscall_to_kernelspace();
}
