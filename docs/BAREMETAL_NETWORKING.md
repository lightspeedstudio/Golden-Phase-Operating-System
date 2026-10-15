# Golden Phase OS — Bare-metal networking & services (PZ7020)

Status: **M1–M3 + S1–S4 all proven on real hardware.**
Board: PZ7020-StarLite (Zynq-7000, Cortex-A9), PS GEM0 / RTL8211F (U15), RGMII.
Branch `feature/native-ethernet`. No OS underneath — this is the bare-metal kernel.

This document ties together the full arc: from raw Layer-2 Ethernet up through a
real SSH login, all running directly on the silicon with no operating system.

---

## 1. What exists, and how it was proven

Every milestone was verified on the actual board (serial + on-wire), with the host
and board corroborating — never from a label. Evidence files are in `scripts/`.

| # | Capability | How it's proven | Evidence | Commit |
|---|------------|-----------------|----------|--------|
| — | RGMII TX delay + per-speed GEM clock | 32/32 wire frames | `pz7020_test_plan_gem.txt` | (native-eth) |
| — | Raw L2 (EtherType 0x88B5) | PhaseLink/PNW1 frames on the wire | `pz7020_test_plan_pn_wire1.txt` | 9bbdb48 |
| **M1** | ARP + IPv4 + ICMP echo | host `ping` → replies TTL=64; `arp -a` → board MAC | `pz7020_test_plan_netip_m1.txt` | d66313a |
| **M2** | UDP echo (:7) | datagram round-trips exact; `udp_echoes=1` | `pz7020_test_plan_netip_m23.txt` | daa6f05 |
| **M3** | TCP echo (:7) | real SYN/SYN-ACK/ACK + data; `tcp_conns=1` | `pz7020_test_plan_netip_m23.txt` | daa6f05 |
| **S1** | **SSH login** | `ssh -p 2222 kairos@board "system status"` → board output | `pz7020_test_plan_ssh.txt` | c8914e7 |
| **S2** | On-board crypto | `security crypto selftest → PASS` | `pz7020_test_plan_crypto.txt` | c8914e7 |
| **S3** | DHCP client + DNS | DORA handshake → `lease 192.168.5.50`; `net dns` → A record | `pz7020_test_plan_dhcp_dns.txt` | c8914e7 |
| **S4** | Interrupt-driven RX | `polls` ~60× lower, `gem_rx_irq>0`, `tick=200Hz` | `pz7020_test_plan_irq.txt` | 6cb576d |

Board MAC `02:00:00:47:01:50`. Static test IP `192.168.5.50/24`; host `192.168.5.2/24`.

---

## 2. Architecture

```
   ssh client ──TCP:2222──┐
                          ▼
   ┌───────────────────────────────────────────────┐
   │  net_ssh.rs   sunset 0.4 (no_std SSH, sans-io) │  S1
   │      exec → shell::execute_session (board CLI) │
   ├───────────────────────────────────────────────┤
   │  net_ip.rs    smoltcp 0.12 Interface + sockets │  M1–M3, S3
   │      ICMP · UDP · TCP · DHCPv4 · DNS            │
   ├───────────────────────────────────────────────┤
   │  gem_phy.rs   smoltcp phy::Device shim         │
   ├───────────────────────────────────────────────┤
   │  gem.rs       GEM0 DMA rings, wire_tx/rx,      │
   │               RX interrupt (one-shot)          │  S4
   └───────────────────────────────────────────────┘
        │ RGMII                    ▲ SPI IRQ 54
        ▼                          │
   RTL8211F PHY (U15)         gic.rs → kairos_board_irq_dispatch
```

- **TCP/IP engine:** `smoltcp` (`no_std` + `alloc`), driven by the vendor lwIP
  `NO_SYS` poll model. Device shim `board/zynq7000/gem_phy.rs` over the existing
  `gem::wire_tx_raw` / `wire_rx_nb` DMA primitives. Monotonic clock = A9 global timer.
- **SSH engine:** `sunset` 0.4 (`no_std`, sans-io, by Dropbear's author). Driven from
  the same poll loop — no async executor. Host key ed25519; userauth ed25519 pubkey;
  `exec` bridged into the board shell via `ShellAction::RunSshServer` from the boot loop
  (so it runs the *same* commands as the UART console).
- **Crypto:** RustCrypto — SHA-256/HMAC + AES-256-GCM (`aes-gcm` no_std) + ed25519
  (`ed25519-dalek` no_std). `crypto/sign.rs` provides on-board signing.
- **Interrupts:** the GIC path was already live (G1P). S4 adds GEM0 RX (SPI 54) and an
  A9 private-timer tick (PPI 29) to `kairos_board_irq_dispatch`; the serve loop `wfi`-sleeps.

### Feature flags (`Cargo.toml`)
- `bm-net` = smoltcp (ipv4/icmp/udp/tcp/dhcpv4/dns).
- `baremetal` now also pulls `aes-gcm` + `ed25519-dalek` (no_std).
- `ssh-bm` = `bm-net` + `baremetal` + `sunset` + `getrandom` (custom backend).
- Board image = `zynq-a9,bm-full,ssh-bm`.

---

## 3. Shell commands (board `gpos>` prompt, COM12 @ 115200)

```
net ip <a.b.c.d> [secs]      # serve ARP/ICMP + UDP echo :7 + TCP echo :7 (static IP)
net ip dhcp [secs]           # acquire a lease via DHCP (S3)
net dns <hostname> [secs]    # DHCP, then resolve an A record (S3)
net ssh serve [port] [secs]  # start the SSH server (default :2222) (S1)
security crypto selftest     # AES-GCM + ed25519 self-test (S2)
```

SSH is pubkey-only and **fail closed**: `AUTHORIZED_PUBKEY` in `src/kernel/net_ssh.rs`
is all zeros in the tree, so no client is accepted until you put *your* ed25519
public key there (do not commit a login key). Password auth is rejected. Host
keys are generated per `net ssh serve`, not baked in.
```bash
ssh -i <your-ed25519> -p 2222 \
    -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    kairos@192.168.5.50 "system status"
```

---

## 4. Build / flash / test recipe

```bash
# build (needs D: unlocked — BitLocker; ARM linker + bootgen live there)
cargo build --bin kairos_zynq --no-default-features \
      --features "zynq-a9,bm-full,ssh-bm" --target armv7a-none-eabi --release
cp target/armv7a-none-eabi/release/kairos_zynq \
   fpga/zynq/evidence/kairos_zynq_boot/BOOT_PZ7020_BASE_BMFULL/kairos_zynq.elf
```
```powershell
# bootgen (run the .bat via cmd; the unwrapped exe is missing DLLs)
cmd /c '"D:\Xilinx\2026.1\Vitis\bin\bootgen.bat" -image <BASE_BMFULL>\boot.bif -arch zynq -o BOOT.BIN -w on'
# SD = drive H: (FAT32); copy BOOT.BIN, then flush before pulling:
Write-VolumeCache -DriveLetter H
```
Reinsert the SD, power-cycle, and drive it over COM12. Host networking setup (the
PC-side gotcha that unblocks the wire) is documented in the memory note; in short:
disable the Hyper-V *WSL-Bridge* vEthernet + the VMware VMnet adapters so the
192.168.5.x route isn't hijacked.

Compile gates kept green: arm `bm-full,ssh-bm`; host default; riscv64 `bm-full`.

---

## 5. Honest caveats & remaining work

- **S3 DHCP/DNS** is proven on the direct cable, but only after three PC-side test-rig
  fixes (not board changes): unbind the Hyper-V vSwitch from the Ethernet (it drops the
  host's outbound broadcast OFFER), add inbound firewall rules for UDP/67 and UDP/53, and
  make the test DHCP server reply from source port 67 bound to the Ethernet IP. See
  `scripts/pz7020_test_plan_dhcp_dns.txt`. A real router avoids all of this.
- **SSH entropy is proof-grade only.** The Zynq-7000 PS has no hardware TRNG; the
  getrandom backend (`board/zynq7000/rng.rs`) is a SHA-256 DRBG seeded from A9 timer
  jitter. Production needs a real entropy source (e.g. a PL ring-oscillator TRNG) fed
  into `rng::reseed()`. **Do not treat these keys as high-assurance.**
- **SSH host key regenerates per boot** (`SignKey::generate`) → clients use
  `StrictHostKeyChecking=no`. A stable seed-derived host key is a small follow-up.
- **SSH is exec-only, one connection per `serve` window.** No interactive PTY yet
  (`ServEvent::SessionShell` handling + a streaming channel loop is the next step);
  the single sunset `Runner` is created once per serve.
- **`copyright_*`** deposit material in `kairos/` is intentionally **untracked** — do
  not commit it.

Toolchain: rustc **1.89** (sunset 0.4 MSRV 1.87 OK; sunset ≥0.5 needs 1.95).
