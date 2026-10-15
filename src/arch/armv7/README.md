# arch/armv7 — Cortex-A9 Golden Phase OS

Bare-metal AArch32 GP-OS for Zynq-7000 lives in workspace crate
[`zynq_a9_os`](../../../zynq_a9_os/) (feature path: build that crate for
`armv7a-none-eabi`).

Board package: [`board/zynq7000`](../../board/zynq7000/).

This directory is the merge anchor (V14): RISC-V QEMU boot paths stay under
their own arch; Zynq A9 does not share the RISC-V linker or startup.
