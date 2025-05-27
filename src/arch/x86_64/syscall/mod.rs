use super::process::idle;
use crate::arch::x86_64::instruction_wrappers;
use crate::arch::x86_64::mm as arch_mm;
use core::arch::asm;

const MSR_IA32_STAR: u32 = 0xc0000081;
const MSR_IA32_LSTAR: u32 = 0xc0000082;
const MSR_IA32_FMASK: u32 = 0xc0000084;
const MSR_IA32_EFER: u32 = 0xc0000080;

fn syscall_entrypoint() {
    log::info!("syscall entrypoint");
    idle();
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

pub fn syscall_to_kernelspace() {
    unsafe {
        let rsp0 = arch_mm::init::TSS_WITH_IO_MAP.tss.rsps[0] as u64
            + ((arch_mm::init::TSS_WITH_IO_MAP.tss.rsps[1] as u64) << 32);
        asm!(
            "mov rsp, {rsp0}",
            "syscall",
            rsp0 = in(reg) rsp0,
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
