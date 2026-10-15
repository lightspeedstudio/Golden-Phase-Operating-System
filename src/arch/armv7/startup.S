/* Cortex-A9 reset stub — linked at DDR 0x00100000 after FSBL handoff.
   Vector table + crash letters mirror the board-proven zynq_a9_os startup.
   IRQ vector saves context, calls kairos_board_irq_dispatch, then returns. */
.syntax unified
.cpu cortex-a9
.fpu softvfp

.global _start
.global _stack_top
.global _irq_stack_top
.global vectors

.section .text.boot, "ax", %progbits
.arm

/* Exception vectors — VBAR points here */
.align 5
vectors:
    b   _start          /* Reset */
    b   abort_undef     /* Undefined */
    b   hang            /* SVC */
    b   abort_pabt      /* Prefetch abort */
    b   abort_dabt      /* Data abort */
    nop                 /* Reserved */
    b   irq_entry       /* IRQ */
    b   hang            /* FIQ */

_start:
    /* SVC stack */
    ldr r0, =_stack_top
    mov sp, r0

    /* Abort mode stack (data/prefetch abort) */
    mrs r1, cpsr
    bic r1, r1, #0x1F
    orr r1, r1, #0x17
    msr cpsr_c, r1
    ldr sp, =_irq_stack_top

    /* Undefined mode stack */
    mrs r1, cpsr
    bic r1, r1, #0x1F
    orr r1, r1, #0x1B
    msr cpsr_c, r1
    ldr sp, =_irq_stack_top

    /* IRQ mode stack */
    mrs r1, cpsr
    bic r1, r1, #0x1F
    orr r1, r1, #0x12
    msr cpsr_c, r1
    ldr sp, =_irq_stack_top

    /* Back to SVC, IRQs masked until GIC init completes in Rust */
    bic r1, r1, #0x1F
    orr r1, r1, #0x13
    orr r1, r1, #0x80
    msr cpsr_c, r1
    ldr sp, =_stack_top

    /* VBAR = vectors */
    ldr r0, =vectors
    mcr p15, 0, r0, c12, c0, 0

    /* Clear BSS */
    ldr r1, =__bss_start
    ldr r2, =__bss_end
    mov r3, #0
1:
    cmp r1, r2
    bhs 2f
    str r3, [r1], #4
    b 1b
2:
    bl rust_main
hang:
    wfi
    b hang

/* IRQ: adjust LR, save SPSR + regs, call Rust, restore, return */
irq_entry:
    sub lr, lr, #4
    srsdb sp!, #18          /* push LR_irq and SPSR_irq onto IRQ stack */
    push {{r0-r3, r12, lr}}
    bl  kairos_board_irq_dispatch
    pop  {{r0-r3, r12, lr}}
    rfeia sp!               /* pop PC and CPSR from stacked LR/SPSR */

/* UART0 putc — dump crash letter then hang (no stack needed beyond lr) */
uart_crash_putc:
    ldr r1, =0xE0000000
1:
    ldr r2, [r1, #0x2C]
    tst r2, #(1 << 3)
    beq 1b
    str r0, [r1, #0x30]
    bx  lr

abort_undef:
    mov r0, #'U'
    bl  uart_crash_putc
    b   hang
abort_pabt:
    mov r0, #'P'
    bl  uart_crash_putc
    b   hang
abort_dabt:
    ldr r1, =0xE0000000
dabt_tx_d:
    ldr r2, [r1, #0x2C]
    tst r2, #(1 << 3)
    beq dabt_tx_d
    mov r0, #'D'
    str r0, [r1, #0x30]
dabt_tx_bang:
    ldr r2, [r1, #0x2C]
    tst r2, #(1 << 3)
    beq dabt_tx_bang
    mov r0, #'!'
    str r0, [r1, #0x30]
    /* Print DFAR/DFSR as hex for the fault report */
    mrc p15, 0, r4, c6, c0, 0
    mrc p15, 0, r5, c5, c0, 0
    mov r3, #8
dabt_dfar:
    lsr r0, r4, #28
    and r0, r0, #0xF
    cmp r0, #10
    addlt r0, r0, #48
    addge r0, r0, #87
dabt_dfar_tx:
    ldr r2, [r1, #0x2C]
    tst r2, #(1 << 3)
    beq dabt_dfar_tx
    str r0, [r1, #0x30]
    lsl r4, r4, #4
    subs r3, r3, #1
    bne dabt_dfar
dabt_slash_tx:
    ldr r2, [r1, #0x2C]
    tst r2, #(1 << 3)
    beq dabt_slash_tx
    mov r0, #'/'
    str r0, [r1, #0x30]
    mov r3, #8
dabt_dfsr:
    lsr r0, r5, #28
    and r0, r0, #0xF
    cmp r0, #10
    addlt r0, r0, #48
    addge r0, r0, #87
dabt_dfsr_tx:
    ldr r2, [r1, #0x2C]
    tst r2, #(1 << 3)
    beq dabt_dfsr_tx
    str r0, [r1, #0x30]
    lsl r5, r5, #4
    subs r3, r3, #1
    bne dabt_dfsr
    b   hang
