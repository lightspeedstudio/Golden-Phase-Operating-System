.syntax unified
.cpu cortex-a9
.fpu softvfp

.global leave_user
.global svc_entry
.global kairos_abort_entry
.global kairos_saved_kernel_sp
.global kairos_saved_kernel_lr
.global kairos_kernel_ttbr
.global kairos_abort_depth
.global kairos_saved_kernel_sp
.global kairos_saved_kernel_lr
.global kairos_kernel_ttbr
.global kairos_abort_depth

.section .bss
.align 4
kairos_saved_kernel_sp:
    .word 0
kairos_saved_kernel_lr:
    .word 0
kairos_kernel_ttbr:
    .word 0
kairos_abort_depth:
    .word 0

.section .text
.arm

/* r0 = &UserEntry {{ pc, sp, r0, r1 }} */
leave_user:
    push {{r4-r11, lr}}
    ldr r1, =kairos_saved_kernel_sp
    str sp, [r1]
    ldr r4, [r0, #0]
    ldr r5, [r0, #4]
    ldr r6, [r0, #8]
    ldr r7, [r0, #12]
    mrs r1, cpsr
    bic r1, r1, #0x1F
    orr r1, r1, #0x1F
    msr cpsr_c, r1
    isb
    mov sp, r5
    mrs r1, cpsr
    bic r1, r1, #0x1F
    orr r1, r1, #0x13
    msr cpsr_c, r1
    isb
    mov r0, r6
    mov r1, r7
    mov r2, #0
    mov r3, #0
    mov lr, r4
    mov r12, #0x90
    msr spsr_cxsf, r12
    movs pc, lr

return_to_kernel:
    ldr r1, =kairos_kernel_ttbr
    ldr r1, [r1]
    cmp r1, #0
    beq 1f
    mcr p15, 0, r1, c2, c0, 0
    dsb
    mov r1, #0
    mcr p15, 0, r1, c8, c7, 0
    dsb
    isb
1:
    ldr r1, =kairos_saved_kernel_sp
    ldr sp, [r1]
    pop {{r4-r11, pc}}

svc_entry:
    push {{r0-r12, lr}}
    mrs r0, spsr
    push {{r0}}
    mov r0, sp
    bl kairos_svc_dispatch
    cmp r0, #1
    beq svc_exit_kernel
    pop {{r1}}
    msr spsr_cxsf, r1
    pop {{r0-r12, lr}}
    movs pc, lr
svc_exit_kernel:
    pop {{r1}}
    pop {{r0-r12, lr}}
    mov r0, #0
    b return_to_kernel

/* r0=FAR r1=FSR r2=PC r3=ifetch. depth already bumped in vector. */
kairos_abort_entry:
    ldr r4, =_abort_stack_top
    mov sp, r4
    push {{r0-r3}}
    ldr r1, =kairos_kernel_ttbr
    ldr r1, [r1]
    cmp r1, #0
    beq 1f
    mcr p15, 0, r1, c2, c0, 0
    dsb
    mov r1, #0
    mcr p15, 0, r1, c8, c7, 0
    dsb
    isb
1:
    pop {{r0-r3}}
    sub sp, sp, #32
    str r0, [sp, #0]
    str r1, [sp, #4]
    str r2, [sp, #8]
    str r3, [sp, #12]
    ldr r4, =kairos_saved_kernel_sp
    ldr r4, [r4]
    str r4, [sp, #16]
    mrs r4, spsr
    str r4, [sp, #20]
    ldr r4, =kairos_abort_depth
    ldr r4, [r4]
    str r4, [sp, #24]
    ldr r4, =_stack_top
    str r4, [sp, #28]
    mov r0, sp
    bl kairos_abort_decide
    cmp r0, #0
    beq abort_return
    cmp r0, #1
    beq c10_watchdog
    b kernel_fatal
abort_return:
    ldr r1, =kairos_abort_depth
    mov r2, #0
    str r2, [r1]
    ldr r1, =kairos_saved_kernel_sp
    ldr r1, [r1]
    cmp r1, #0
    beq c10_watchdog
    tst r1, #3
    bne c10_watchdog
    mrs r1, cpsr
    bic r1, r1, #0x1F
    orr r1, r1, #0x13
    msr cpsr_c, r1
    isb
    mov r0, #2
    b return_to_kernel
c10_watchdog:
    bl kairos_c10_watchdog
kernel_fatal:
    wfi
    b kernel_fatal
