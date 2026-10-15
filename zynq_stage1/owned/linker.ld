ENTRY(_start)

MEMORY
{
  ocm (rwx) : ORIGIN = 0x00000000, LENGTH = 0x00030000
}

SECTIONS
{
  .text : {
    KEEP(*(.text.boot))
    *(.text .text.*)
  } > ocm

  .rodata : {
    *(.rodata .rodata.*)
  } > ocm

  .data : {
    *(.data .data.*)
  } > ocm

  .bss : {
    . = ALIGN(8);
    __bss_start = .;
    *(.bss .bss.*)
    *(COMMON)
    . = ALIGN(8);
    __bss_end = .;
  } > ocm

  . = ALIGN(16);
  . += 0x2000;
  _stack_top = .;
}
