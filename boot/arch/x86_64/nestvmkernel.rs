use core::arch::global_asm;

global_asm!(
    // Trigger rebuild of current file. vmkernel.bin.hash is generated in makefile.
    core::include_str!("vmkernel.bin.hash"),
    ".section \".rdata_vmkernel\",\"a\"",
    ".globl __vmkernel_start, __vmkernel_end",
    "__vmkernel_start:",
    ".incbin \"target/x86_64-elf/vmkernel.bin\"",
    "__vmkernel_end:",
);
