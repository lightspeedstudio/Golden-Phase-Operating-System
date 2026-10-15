ENTRY(_start)

MEMORY
{
  /* On-chip RAM banks 0-2 at reset mapping — the 192 KiB the BootROM loads. */
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

  /* SVC stack (8 KiB) */
  . = ALIGN(16);
  . += 0x2000;
  _stack_top = .;

  /* Abort/undef/IRQ mode stack (2 KiB) */
  . = ALIGN(16);
  . += 0x800;
  _irq_stack_top = .;

  /* No heap in stage-0 — zero-size region satisfies boot::heap externs. */
  __heap_start = .;
  __heap_end = .;

  /DISCARD/ : {
    *(.ARM.exidx*)
    *(.comment)
  }
}
