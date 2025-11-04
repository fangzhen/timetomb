use crate::arch::x86_64::instruction_wrappers;
use crate::arch::x86_64::mm as arch_mm;
use crate::kernel::process;
use core::arch::asm;
use core::arch::naked_asm;

use super::process::context_switch::process_end;

pub mod pt_regs;
use pt_regs::PtRegs;

pub const DEFAULT_USER_FLAGS: u64 = 0x0200;

const MSR_IA32_STAR: u32 = 0xc0000081;
const MSR_IA32_LSTAR: u32 = 0xc0000082;
const MSR_IA32_FMASK: u32 = 0xc0000084;
const MSR_IA32_EFER: u32 = 0xc0000080;

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

        "mov rdi, rax",        // First argument: syscall number (RAX)
        "mov rsi, rsp",        // Second argument: pointer to saved registers (pt_regs)
        "call {dispatch}",

        "mov rsi, rax",        // return value is in RAX
        "mov rdi, rsp",
        "call {sysret_to_user}",
        dispatch = sym syscall_dispatch,
        sysret_to_user = sym sysret_to_userspace,
    );
}

/// Return to userspace by sysret.
/// Notice that kernel rsp is not saved. When trap into kernel next time, it uses
/// kernel stack from scratch.
#[unsafe(naked)]
pub unsafe extern "C" fn sysret_to_userspace(regs: &PtRegs, ret_value: u64) {
    naked_asm!(
        "mov rax, rsi", // syscall return value
        "mov rbx, [rdi + 0x08]",
        "mov rcx, [rdi + 0x10]",
        "mov rdx, [rdi + 0x18]",
        "mov rsi, [rdi + 0x20]",
        "mov rbp, [rdi + 0x30]",
        "mov rsp, [rdi + 0x38]",
        "mov r8,  [rdi + 0x40]",
        "mov r9,  [rdi + 0x48]",
        "mov r10, [rdi + 0x50]",
        "mov r11, [rdi + 0x58]",
        "mov r12, [rdi + 0x60]",
        "mov r13, [rdi + 0x68]",
        "mov r14, [rdi + 0x70]",
        "mov r15, [rdi + 0x78]",
        "mov rdi, [rdi + 0x28]", // restore rdi at last.
        "mov rsp, rdx",          // restore user rsp
        "sysretq",               // to user space!
    );
}
