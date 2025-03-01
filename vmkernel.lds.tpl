SECTIONS
{
  . = VMKERNEL_ENTRY_ADDRESS_REPLACE;
  .text.head : {
    KEEP(*(.text.head))
  }
} INSERT BEFORE .text;
