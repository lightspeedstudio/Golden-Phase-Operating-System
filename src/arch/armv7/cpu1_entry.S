/* CPU1 BootROM target. CPU0 writes this address to 0xFFFFFFF0 then SEV.
   Do not MRC TTBR0 here: that reads *this* core's TTBR0, not CPU0's.
   TTBR0 physical address comes from CPU1_MAILBOX (written while CPU0
   D-cache was still off). */
.syntax unified
.cpu cortex-a9
.fpu softvfp

.global _cpu1_entry
.global cpu1_rust_main
.global CPU1_MAILBOX

.section .text.boot.cpu1, "ax", %progbits
.arm
.align 5

_cpu1_entry:
    cpsid   if

    ldr     r0, =_cpu1_stack_top
    mov     sp, r0

    mrs     r1, cpsr
    bic     r1, r1, #0x1F
    orr     r1, r1, #0x17
    msr     cpsr_c, r1
    ldr     sp, =_cpu1_irq_stack_top

    bic     r1, r1, #0x1F
    orr     r1, r1, #0x1B
    msr     cpsr_c, r1
    ldr     sp, =_cpu1_irq_stack_top

    bic     r1, r1, #0x1F
    orr     r1, r1, #0x12
    msr     cpsr_c, r1
    ldr     sp, =_cpu1_irq_stack_top

    bic     r1, r1, #0x1F
    orr     r1, r1, #0x13
    orr     r1, r1, #0xC0
    msr     cpsr_c, r1
    ldr     sp, =_cpu1_stack_top

    ldr     r0, =vectors
    mcr     p15, 0, r0, c12, c0, 0

    /* ACTLR SMP bit 6 (L1 coherency). Must precede D-cache enable. */
    mrc     p15, 0, r0, c1, c0, 1
    orr     r0, r0, #(1 << 6)
    orr     r0, r0, #1
    mcr     p15, 0, r0, c1, c0, 1
    isb

    /* mailbox.ttbr0 at offset 4 — already includes TTBR IRGN/S bits */
    ldr     r0, =CPU1_MAILBOX
    ldr     r0, [r0, #4]
    mcr     p15, 0, r0, c2, c0, 0

    mov     r0, #0
    mcr     p15, 0, r0, c2, c0, 2

    mvn     r0, #0
    mcr     p15, 0, r0, c3, c0, 0

    mov     r0, #0
    mcr     p15, 0, r0, c7, c5, 0
    mcr     p15, 0, r0, c8, c7, 0
    dsb
    isb

    mrc     p15, 0, r0, c1, c0, 0
    orr     r0, r0, #0x0005
    orr     r0, r0, #0x1800
    mcr     p15, 0, r0, c1, c0, 0
    isb
    dsb

    mrc     p15, 0, r0, c0, c0, 5
    and     r0, r0, #0x0F
    bl      cpu1_rust_main

.cpu1_dead:
    wfe
    b       .cpu1_dead
.size _cpu1_entry, . - _cpu1_entry
