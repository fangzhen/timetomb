use core::arch::global_asm;
global_asm!(
    ".section .text.head",
    ".globl _start, _setup_header",
    "_start:",
    "jmp main",
    ".org 0x10",
    "_setup_header:",
    "    .zero 0x400",
    // initial kernel stack
    //TODO hardcoded size. It's better to use page size.
    // TODO too large stack size
    ".align 4096",
    ".globl _stack_bottom, _stack_top",
    "_stack_bottom:",
    "    .zero 40960",
    "_stack_top:",
    // initial page table. TODO(hardcode)
    ".align 4096",
    ".global _pgtable_start, _pgtable_end",
    "_pgtable_start: .zero 409600",
    "_pgtable_end:"
);
