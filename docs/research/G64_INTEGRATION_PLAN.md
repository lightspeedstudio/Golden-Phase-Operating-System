# G64 Integration Plan

1. Keep G1P @ `0x40000000` / IRQ 61 unchanged.
2. Add `g64_axi_lite_sidecar` on `M_AXI_GP0` @ proposed `0x40001000` (Address Editor confirm).
3. First path: polled GP only; no HP/ACP; no new IRQ.
4. Software: feature `g64-research` only; every HW op compared to `gphase_core`.
5. Research BOOT only: `BOOT_G64_RESEARCH.BIN` on second SD.
6. After physical local sidecar: evaluate HP/ACP/IRQ expansions separately.
