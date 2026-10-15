//! RISC-V reset vector — linked at `0x80000000` via `boot/linker.ld`.

#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
core::arch::global_asm!(
    r#"
    .section .text.entry, "ax", @progbits
    .global _start
    _start:
        la sp, __stack_top
        csrw mie, zero
        csrci mstatus, 8
        li t2, 0x2000
        csrs mstatus, t2
        la t0, trap_entry
        csrw mtvec, t0
        la t0, __bss_start
        la t1, __bss_end
    clear_bss:
        bgeu t0, t1, clear_bss_done
        sd zero, 0(t0)
        addi t0, t0, 8
        j clear_bss
    clear_bss_done:
        tail rust_main
    1:
        wfi
        j 1b

    .section .text.trap, "ax", @progbits
    .align 4
    .global trap_entry
    trap_entry:
        csrr a0, mcause
        csrr a1, mepc
        csrr a2, mtval
        call trap_report
    2:
        wfi
        j 2b
    "#
);
