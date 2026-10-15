ENTRY(_start)

MEMORY
{
  /* Match V2 FSBL handoff — application in DDR above 1 MiB. Board DDR ends
     at 0x1FFFFFFF; keep the image + stacks + heap well inside it. */
  ddr (rwx) : ORIGIN = 0x00100000, LENGTH = 0x0FF00000
}

SECTIONS
{
  .text : {
    KEEP(*(.text.boot))
    KEEP(*(.text.boot.cpu1))
    *(.text .text.*)
  } > ddr

  .rodata : {
    *(.rodata .rodata.*)
  } > ddr

  .data : {
    *(.data .data.*)
  } > ddr

  .bss : {
    . = ALIGN(8);
    __bss_start = .;
    *(.bss .bss.*)
    *(COMMON)
    . = ALIGN(8);
    __bss_end = .;
  } > ddr

  /* SVC stack (64 KiB) */
  . = ALIGN(16);
  . += 0x10000;
  _stack_top = .;

  /* Abort/undef/IRQ mode stack (8 KiB) */
  . = ALIGN(16);
  . += 0x2000;
  _irq_stack_top = .;

  /* Short-descriptor L1: 4096 x 1 MiB sections, 16 KiB aligned (ARM ARM).
     DDR, not OCM: this kernel's proven map is DDR @ 0x00100000. */
  . = ALIGN(0x4000);
  _mmu_l1 = .;
  . += 0x4000;

  /* CPU1 SVC stack (16 KiB). CPU0 stack stays at _stack_top. */
  . = ALIGN(16);
  . += 0x4000;
  _cpu1_stack_top = .;

  /* CPU1 abort/undef/IRQ (4 KiB) */
  . = ALIGN(16);
  . += 0x1000;
  _cpu1_irq_stack_top = .;

  /* Heap for boot::heap (16 MiB) */
  . = ALIGN(4096);
  __heap_start = .;
  . += 0x01000000;
  __heap_end = .;

  /DISCARD/ : {
    *(.ARM.exidx*)
    *(.comment)
  }
}
