use crate::arch::x86_64::instruction_wrappers;
use crate::arch::x86_64::mm as arch_mm;
use crate::kernel::process;
use core::arch::asm;
use core::arch::naked_asm;

use super::process::context_switch_asm::process_end;

pub mod pt_regs;
use pt_regs::PtRegs;

const MSR_IA32_STAR: u32 = 0xc0000081;
const MSR_IA32_LSTAR: u32 = 0xc0000082;
const MSR_IA32_FMASK: u32 = 0xc0000084;
const MSR_IA32_EFER: u32 = 0xc0000080;

#[unsafe(naked)]
unsafe extern "C" fn syscall_entrypoint() {
    naked_asm!(
        // We save all user registers, since:
        // - It should not rely on a specific ABI.
        // - syscalls like fork() don't return in a normal execution flow.
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push r11",  // user rflags
        "push r10",
        "push r9",
        "push r8",
        "push rsp",
        "push rbp",
        "push rdi",
        "push rsi",
        "push rdx",  // user rsp
        "push rcx",  // user rip
        "push rbx",
        "push rax",

        // Save user stack pointer and align stack.
        "mov rbp, rsp",

        // First argument: syscall number (RAX)
        "mov rdi, rax",
        // Second argument: pointer to saved registers (pt_regs)
        "mov rsi, rsp",
        "call {dispatch}",  // return value is in RAX

        // Restore user registers
        "add rsp, 8",  // Don't restore rax
        "pop rbx",
        "pop rcx",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop rbp",
        "add rsp, 8",  // Don't restore rsp
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r11",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",

        "mov rsp, rdx",  // restore user rsp
        "sysretq",
        dispatch = sym syscall_dispatch,
    );
}

extern "C" fn syscall_dispatch(num: usize, regs: &PtRegs) -> i64 {
    if num == 1 {
        process_end();
        return 0;
    }
    if num == 2 {
        return process::fork(regs).0 as i64;
    }
    if num == 3 {
        process::yield_current();
        return 0;
    }
    if num == 128 {
        log::info!("syscall stub invoked.");
    }
    return -1;
}

pub fn syscall_init() {
    unsafe {
        asm!(
            //EFER
            "mov ecx, {efer:e}",
            "rdmsr",
            "or eax, 1",           // enable SCE bit
            "wrmsr",
            // IA32_STAR
            "mov ecx, {star:e}",
            "rdmsr",
            "mov edx, 0x00100008", // load up GDT segment bases 0x08 (kernel) and 0x10 (user, auctully 0x18 for user ss and 0x20 for user code); TODO(fangzhen) hardcode
            "wrmsr",
            // FMASK -- just clear fmask for now
            "mov ecx, {fmask:e}",
            "xor eax, eax",
            "xor edx, edx",
            "wrmsr",
            efer = in(reg) MSR_IA32_EFER,
            star = in(reg) MSR_IA32_STAR,
            fmask = in(reg) MSR_IA32_FMASK,
            out("edx") _, // rdmsr, wrmsr use these registers.
            out("eax") _,
            out("ecx") _,
        );
        instruction_wrappers::wrmsr(MSR_IA32_LSTAR, syscall_entrypoint as u64);
    }
}

pub fn syscall_to_kernelspace(num: usize) -> i64 {
    let ret_value: i64;
    unsafe {
        let rsp0 = arch_mm::init::TSS_WITH_IO_MAP.tss.rsps[0] as u64
            + ((arch_mm::init::TSS_WITH_IO_MAP.tss.rsps[1] as u64) << 32);
        asm!(
            "mov rdx, rsp", // save user rsp to rdx
            "mov rsp, {rsp0}",
            "syscall",
            rsp0 = in(reg) rsp0,
            inlateout("rax") num => ret_value,
            lateout("r11") _,
            lateout("rcx") _,
            out("rdx") _,
        );
    }
    return ret_value;
}

/// TODO: duplicate with syscall_entrypoint
pub fn sysret_to_userspace(uf_addr: usize, stack_addr: usize, regs: &PtRegs) {
    unsafe {
        //TODO(fangzhen) which registers should be restored?
        asm!(
            "mov rax, [rdi + 0x00]",
            "mov rbx, [rdi + 0x08]",
            "mov rcx, [rdi + 0x10]",
            "mov rdx, [rdi + 0x18]",
            "mov rsi, [rdi + 0x20]",
            "mov r8,  [rdi + 0x40]",
            "mov r9,  [rdi + 0x48]",
            "mov r10, [rdi + 0x50]",
            "mov r11, [rdi + 0x58]",
            "mov r12, [rdi + 0x60]",
            "mov r13, [rdi + 0x68]",
            "mov r14, [rdi + 0x70]",
            "mov r15, [rdi + 0x78]",
            in("rdi") regs,
        );
        let rsp0 = &mut arch_mm::init::TSS_WITH_IO_MAP.tss.rsps[0] as *mut u32;
        asm!("mov [r10], rsp",      // save kernel rsp TODO(fangzhen) seems unnecessary.
             "mov rcx, rdi",        // first argument, new instruction pointer
             "mov rsp, rsi",        // second argument, new stack pointer
             "mov r11, 0x0200",     // rflags: IF TODO(fangzhen): restore
             "sysretq",             // to user space!
             in("rdi") uf_addr,
             in("rsi") stack_addr,
             in("rax") 0, // TODO hardcoded return 0
             in("r10") rsp0,
             out("r11") _,
             out("rcx") _,
        );
    };
}
