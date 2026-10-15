# KairOS Board v1 — GDB init
# arm-none-eabi-gdb or riscv64-unknown-elf-gdb

target extended-remote localhost:3333
set remotetimeout 10
monitor reset halt
file release_snapshot/baremetal/tiers/full/kairos_boot.elf
break rust_main
continue
