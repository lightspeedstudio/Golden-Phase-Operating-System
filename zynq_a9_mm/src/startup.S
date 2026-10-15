.syntax unified
.cpu cortex-a9
.fpu softvfp

.global _start
.global _stack_top

.section .text.boot, "ax", %progbits
.arm
_start:
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
3:
    wfi
    b 3b
