ENTRY(_start)

MEMORY
{
  ddr (rwx) : ORIGIN = 0x00100000, LENGTH = 0x0FF00000
}

SECTIONS
{
  .text : {
    KEEP(*(.text.boot))
    KEEP(*(.text.boot.cpu1))
    *(.text .text.*)
    _text_end = .;
  } > ddr

  /* Isolation maps the kernel image with 4 KiB pages, so every section
     boundary must be page-aligned. Without this a single page holds the tail
     of one section and the head of the next, and the page can only carry one
     set of permissions -- which mapped live `.data` read-only and aborted on
     the first kernel write. See `mmu::page_kind`.

     The alignment goes on the output-section header, not a bare `. = ALIGN()`
     between sections: with `> ddr` region allocation a loose location-counter
     assignment is ignored, which left `_rodata_start` sharing a page with
     `_text_end`. */
  .rodata ALIGN(4096) : {
    _rodata_start = .;
    *(.rodata .rodata.*)
    . = ALIGN(4096);
    _rodata_end = .;
  } > ddr

  .data ALIGN(4096) : {
    _data_start = .;
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

  . = ALIGN(16);
  . += 0x10000;
  _stack_top = .;

  . = ALIGN(16);
  . += 0x2000;
  _irq_stack_top = .;

  . = ALIGN(16);
  . += 0x2000;
  _abort_stack_top = .;

  . = ALIGN(0x4000);
  _mmu_l1 = .;
  . += 0x4000;

  . = ALIGN(0x4000);
  _mmu_l1_cpu1 = .;
  . += 0x4000;

  . = ALIGN(16);
  . += 0x4000;
  _cpu1_stack_top = .;

  . = ALIGN(16);
  . += 0x1000;
  _cpu1_irq_stack_top = .;

  . = ALIGN(0x400);
  _mmu_l2_kernel = .;
  . += 0x8000;
  _mmu_l2_cpu1 = .;
  . += 0x8000;

  . = ALIGN(4096);
  __heap_start = .;
  . += 0x01000000;
  __heap_end = .;

  /DISCARD/ : {
    *(.ARM.exidx*)
    *(.comment)
  }
}
