use core::arch::asm;

pub fn cli() {
    unsafe { asm!("cli") };
}
pub fn sti() {
    unsafe { asm!("sti") };
}

#[derive(Default)]
pub struct CPUIDRes {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
}

pub fn cpuid(eax: u32) -> CPUIDRes {
    let mut res = CPUIDRes::default();
    unsafe {
        asm!("cpuid",
             "mov {0:e}, ebx",
             out(reg) res.ebx,
             in("eax") eax,
             lateout("eax") res.eax,
             out("ecx") res.ecx,
             out("edx") res.edx,
        );
    }
    return res;
}

pub fn rdmsr(ecx: u32) -> u64 {
    let res: u64;
    unsafe {
        asm!("rdmsr",
             "shl rdx, 32",
             "add rax, rdx",
             in("ecx") ecx,
             out("rdx") _,
             out("rax") res,
        );
    }
    return res;
}

//Writes value to msr specified in ecx
pub fn wrmsr(ecx: u32, value: u64) {
    unsafe {
        asm!("wrmsr",
             in("ecx") ecx,
             in("edx") value >>32,
             in("eax") value as u32,
        );
    }
}

// Helper functions to perform I/O port operations
#[inline(always)]
pub fn inb(port: u16) -> u8 {
    let result: u8;
    unsafe {
        asm!(
        "in al, dx",
         out("al") result,
        in("dx") port);
    }
    result
}

#[inline(always)]
pub fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
        "out dx, al",
         in("al") value,
        in("dx") port);
    }
}
