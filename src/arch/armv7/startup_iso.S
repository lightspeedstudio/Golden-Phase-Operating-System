/* Isolation vectors: SVC + abort return to kernel. include_str in global_asm:
   double braces become single braces. */
.syntax unified
.cpu cortex-a9
.fpu softvfp

.global _start
.global _stack_top
.global _irq_stack_top
.global _abort_stack_top
.global vectors
.global kairos_abort_depth

.section .text.boot, "ax", %progbits
.arm

.align 5
vectors:
    b   _start
    b   abort_undef
    b   svc_entry
    b   abort_pabt
    b   abort_dabt
    nop
    b   irq_entry
    b   hang

_start:
    ldr r0, =_stack_top
    mov sp, r0

    mrs r1, cpsr
    bic r1, r1, #0x1F
    orr r1, r1, #0x17
    msr cpsr_c, r1
    ldr sp, =_abort_stack_top

    mrs r1, cpsr
    bic r1, r1, #0x1F
    orr r1, r1, #0x1B
    msr cpsr_c, r1
    ldr sp, =_abort_stack_top

    mrs r1, cpsr
    bic r1, r1, #0x1F
    orr r1, r1, #0x12
    msr cpsr_c, r1
    ldr sp, =_irq_stack_top

    bic r1, r1, #0x1F
    orr r1, r1, #0x13
    orr r1, r1, #0x80
    msr cpsr_c, r1
    ldr sp, =_stack_top

    ldr r0, =vectors
    mcr p15, 0, r0, c12, c0, 0

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

irq_entry:
    sub lr, lr, #4
    srsdb sp!, #18
    push {{r0-r3, r12, lr}}
    bl  kairos_board_irq_dispatch
    pop  {{r0-r3, r12, lr}}
    rfeia sp!

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
    ldr r12, =kairos_abort_depth
    ldr r2, [r12]
    add r2, r2, #1
    str r2, [r12]
    cmp r2, #1
    bhi c10_slcr_reset
    /* Capture the banked LR_abt and the CP15 fault registers BEFORE the bl.
       `bl uart_crash_putc` overwrites LR_abt, which previously made the
       reported prefetch-abort PC a kernel address (0010010c) instead of the
       faulting user address. r4-r6 are not banked, but this path already
       clobbers them (see the data-abort hex dump) and `return_to_kernel`
       restores r4-r11 from the saved SVC frame. */
    sub r6, lr, #4
    mrc p15, 0, r4, c6, c0, 2   /* IFAR */
    mrc p15, 0, r5, c5, c0, 1   /* IFSR */
    mov r0, #'P'
    bl  uart_crash_putc
    mov r0, r4
    mov r1, r5
    mov r2, r6
    mov r3, #1
    b   kairos_abort_entry

abort_dabt:
    ldr r12, =kairos_abort_depth
    ldr r2, [r12]
    add r2, r2, #1
    str r2, [r12]
    cmp r2, #1
    bhi c10_slcr_reset
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
    mrc p15, 0, r5, c5, c0, 0
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
    mrc p15, 0, r0, c6, c0, 0
    mrc p15, 0, r1, c5, c0, 0
    sub r2, lr, #8
    mov r3, #0
    b   kairos_abort_entry

c10_slcr_reset:
    /* #region agent log */
    ldr r1, =0xE0000000
    mov r0, #'R'
    str r0, [r1, #0x30]
    /* #endregion */
    ldr r0, =0xF8000008
    ldr r1, =0xDF0D
    str r1, [r0]
    ldr r0, =0xF8000200
    mov r1, #1
    str r1, [r0]
1:
    wfi
    b 1b
