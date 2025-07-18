use crate::arch::x86_64::instruction_wrappers;
use crate::arch::x86_64::mm as arch_mm;
use core::arch::asm;
use core::arch::naked_asm;

use super::process::context_switch_asm::process_end;

const MSR_IA32_STAR: u32 = 0xc0000081;
const MSR_IA32_LSTAR: u32 = 0xc0000082;
const MSR_IA32_FMASK: u32 = 0xc0000084;
const MSR_IA32_EFER: u32 = 0xc0000080;

#[unsafe(naked)]
unsafe extern "C" fn syscall_entrypoint() {
    naked_asm!(
        // Save user registers.
        // RCX and R11 are already saved by the syscall instruction (RIP and RFLAGS).
        "push rax",
        "push rbx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "push rbp",

        // Save user stack pointer and align stack.
        "mov rbp, rsp",

        // Call dispatch with syscall number from RAX.
        // The original RAX was saved on the stack.
        "mov rdi, [rsp + 12 * 8]", // RAX is the first pushed register
        "call {dispatch}",

        // Restore user registers
        "pop rbp",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rbx",
        "pop rax",

        // Return to user.
        // RCX (user RIP) and R11 (user RFLAGS) are not touched by this function.
        // The `syscall` instruction will set them, and `sysretq` will use them.
        "sysretq",
        dispatch = sym syscall_dispatch,
    );
}

fn syscall_dispatch(num: usize) {
    if num == 1 {
        process_end()
    }
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

pub fn syscall_to_kernelspace(num: usize) {
    unsafe {
        let rsp0 = arch_mm::init::TSS_WITH_IO_MAP.tss.rsps[0] as u64
            + ((arch_mm::init::TSS_WITH_IO_MAP.tss.rsps[1] as u64) << 32);
        asm!(
            "mov rsp, {rsp0}",
            "syscall",
            rsp0 = in(reg) rsp0,
            in("rax") num,
            out("r11") _,
            lateout("rcx") _,   //syscall
        );
    }
}

pub fn sysret_to_userspace(uf_addr: usize, stack_addr: usize) {
    unsafe {
        let rsp0 = &mut arch_mm::init::TSS_WITH_IO_MAP.tss.rsps[0] as *mut u32;
        asm!("mov [r10], rsp",      // save kernel rsp
            "mov rcx, rdi",        // first argument, new instruction pointer
            "mov rsp, rsi",        // second argument, new stack pointer
            "mov r11, 0x0200",     // rflags: IF TODO(fangzhen): restore
            "sysretq",             // to user space!
            in("rdi") uf_addr,
            in("rsi") stack_addr,
            in("r10") rsp0,
            out("r11") _,
            out("rcx") _,
        );
    };
}
