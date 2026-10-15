# G64 Physical Test Plan

Second microSD only. Never overwrite `com12_capture_20260730_132633.txt`.

1. Boot research image → splash → `gpos>`
2. `help`, `state`
3. `g1p info` (still valid)
4. `g64 info` (ID/version — not PASS alone)
5. `g64 compose` (16/16 vs software)
6. `g64 reflect` (4/4)
7. `g64 oracle` then `g64 atlas` (masks vs `gphase_core`)
8. `g64 benchmark`
9. Confirm shell still live; power-cycle; reboot

Evidence names: `com12_g64_sidecar_<timestamp>.{txt,json}` + `SHA256SUMS.txt` under research `evidence/`.

On boot failure: restore verified SD; confirm locked release still boots; document FAILED candidate.
