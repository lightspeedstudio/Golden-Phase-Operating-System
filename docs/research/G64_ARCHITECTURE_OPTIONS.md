# G64 Architecture Options

See RTL under `fpga/zynq/research/g64_phase_fabric_sidecar/rtl/candidates/`.

| ID | Name | Notes |
|----|------|-------|
| C1 | Table-minimal | Lower-bound resource probe |
| C2 | Pipelined single engine | Latency/II measurement |
| C3 | 8 lanes | 8 batches for full atlas |
| C4 | 16 lanes | 4 batches |
| C5 | 64 lanes | Full concurrent masks |
| C6 | 8×8 grid | Logical indexing + row/col OR |
| C7 | Resident fabric | 64×2-bit state + compose |
| C8 | Fixed-point stub | Not authority; DSP experiment hook |
| C9 | Hybrid | To be selected from sweep measurements |

Integration sidecar: `rtl/g64_axi_lite_sidecar.v` (polled atlas over runtime slot RAM).
