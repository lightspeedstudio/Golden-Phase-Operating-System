ENTRY(_start)

MEMORY
{
  ddr (rwx) : ORIGIN = 0x00100000, LENGTH = 0x01000000
}

SECTIONS
{
  .text : {
    KEEP(*(.text.boot))
    *(.text .text.*)
  } > ddr

  .rodata : { *(.rodata .rodata.*) } > ddr
  .data : { *(.data .data.*) } > ddr

  .bss : {
    . = ALIGN(8);
    __bss_start = .;
    *(.bss .bss.*)
    *(COMMON)
    . = ALIGN(8);
    __bss_end = .;
  } > ddr

  . = ALIGN(16);
  . += 0x4000;
  _stack_top = .;
}
