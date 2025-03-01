use core::arch::global_asm;

global_asm!(
    ".section .text.head",
    ".globl _start, _setup_header",
    "_start:",
    "jmp main",
    ".org 0x10",
    "_setup_header:",
    ".rept 0x400",
    ".byte 0",
    ".endr",
);
