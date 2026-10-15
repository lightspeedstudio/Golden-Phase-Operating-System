# PZ7020 Recovery

Recovery is intentionally simple: the board boots from a **FAT32 file**
(`BOOT.BIN`), so restoring a known-good state is a plain file copy. No
raw-sector operations are ever required, and the on-card user-data region is
never touched by flashing.

## Known-good reference

| Item | Value |
|------|-------|
| Branch | `zynq-pz7020-port` |
| Preservation commit | `7b7d75490c06d38d9b45c1540f53aaed0db1e846` |
| Tag | `pz7020-console-proof-20260730` |
| BOOT.BIN sha256 | `11272de87b28886e87a8e8023a2619e92f2c543536a09b4fc05102798a3ccd9e` |
| system.bit sha256 | `bb758133f639986fce1d4035ef310571a15a04ced8e8da10f656493f5eb56f91` |
| kairos_zynq ELF (as proven) sha256 | `b5d0ded50dd95f1e4a3f9ada6df368b7f93a86d57fd5cdf3813617897cf354f0` |
| FSBL sha256 | `8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874` |

## A. Restore a bootable card

1. Format the microSD as **FAT32**.
2. Copy a known-good `BOOT.BIN` to the **root**.
3. Verify the hash matches the reference above.
4. Safely eject, insert into the board, RESET.

If you don't have a saved `BOOT.BIN`, rebuild it from source — see
`docs/PZ7020_BUILD_AND_FLASH.md` (build kernel → bootgen with the sealed FSBL +
`system.bit`).

## B. Rebuild the exact as-proven kernel

```bash
git checkout pz7020-console-proof-20260730
cargo build --release --bin kairos_zynq \
  --no-default-features --features zynq-a9 --target armv7a-none-eabi
sha256sum target/armv7a-none-eabi/release/kairos_zynq
# expect b5d0ded5…  (as-proven; the label-fix commit changes this hash)
```

Then re-package BOOT.BIN and verify `11272de8…`.

## C. Board won't boot / no UART

1. Confirm boot-mode jumpers = **SD**.
2. Confirm `BOOT.BIN` is at the card **root** and hashes correctly.
3. Confirm COM12 @ **115200 8-N-1** and press **RESET** during the capture
   window (the interactive shell is silent until it boots or receives input).
4. If a USB-UART adapter ties **DTR/RTS to reset**, the board can be held in
   reset while the port is open — re-run the capture with DTR/RTS low
   (set both `$false` in the script) or use a terminal that doesn't assert them.
5. If you see a crash letter:
   - `U` undefined instruction, `P` prefetch abort, `I` unexpected IRQ.
   - `D!<DFAR>/<DFSR>` data abort with fault-address/status hex — decode to find
     the bad access.

## D. G1P reports `UNEXPECTED …`

The PL bitstream did not load or the wrong bitstream is present. Rebuild
`BOOT.BIN` ensuring partition order is `FSBL → system.bit → kairos_zynq.elf`
(system.bit **before** the app). Re-verify `system.bit` sha `bb758133…`.

## E. Git safety

- The preservation commit and tag are never rewritten or force-pushed.
- To return the working tree to the proven state without losing other work:
  ```bash
  git stash            # if you have local changes to keep
  git checkout pz7020-console-proof-20260730
  ```
- The original repository/history is preserved; recovery never deletes or
  resets shared history.

## F. What recovery never does

- No raw-sector writes, no `diskpart`, no admin disk hacking.
- Never touches the on-card PhaseFS/user data region.
- Never force-pushes or rewrites git history.
