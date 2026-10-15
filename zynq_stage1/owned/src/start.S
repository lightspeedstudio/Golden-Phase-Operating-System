/* Cortex-A9 OCM reset for System 3 owned entry.
   SCTLR M/C/I off (ARM Cortex-A9 TRM). Not vendor FSBL. */

.syntax unified
.cpu cortex-a9
.fpu softvfp

.global _start
.global rust_main

.section .text.boot, "ax", %progbits
.arm
.align 5
_start:
    mrc p15, 0, r0, c1, c0, 0
    bic r0, r0, #0x0005
    bic r0, r0, #0x1000
    mcr p15, 0, r0, c1, c0, 0
    isb

    ldr r0, =_stack_top
    mov sp, r0

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
