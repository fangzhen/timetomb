use core::arch::global_asm;

global_asm!(
    ".section .text.head",
    ".globl _start, _setup_header",
    "_start:",
    "jmp main",
    ".org SETUP_HEADER_OFFSET",
    "_setup_header:",
    ".rept SETUP_HEADER_SIZE",
    ".byte 0",
    ".endr",
);
