# Golden Phase OS — Bare-metal USB HID host (PZ7020)

Status: **U0 (controller + PHY bring-up, port detect) proven on hardware. U1/U2 planned.**
Companion to [BAREMETAL_NETWORKING.md](BAREMETAL_NETWORKING.md). Branch `feature/native-ethernet`.
Goal: keyboard/mouse (HID) input, kept simple — one device, boot protocol, no hub/storage yet.

---

## 1. The board's USB is a HOST port

The PZ7020 wires the PS USB through a **USB3320 ULPI PHY** to a **USB Type-A** connector
(`J10`). You plug devices *into* it — it can't act as a USB device. So "USB" here means a
**USB host stack**, which is why it's a large effort: USB deliberately pushes all complexity
onto the host (the computer), and the board now *is* the computer. The Xilinx `xusbps` driver
is **device-mode only** (no host stack), so unlike Ethernet — where `gem.rs` + `smoltcp` were
both handed to us — for USB host we write both the low-level driver *and* the stack.

## 2. Grounded hardware facts (transcribed from the vendor packs, not guessed)

| Fact | Value | Source |
|---|---|---|
| Controller | USB0 @ `0xE0002000`, ChipIdea/EHCI | `xusbps_hw.h`, device tree |
| PHY / mode | ULPI (USB3320), **host** | `image.ub` DT: `usb@e0002000 phy_type=ulpi dr_mode=host` |
| MIO mux | MIO 28–39 = `0x1204`/`0x1205` (L1_SEL=1 = USB0; `0x1205` inputs MIO 29/31/37) | vendor `2_SDK/1_hello_world/ps7_init.tcl` |
| Clock | `APER_CLK_CTRL` (`0xF800012C`) bit 2 `USB0_CPU_1XCLKACT` | vendor `ps7_init` |
| Key regs | `CMD 0x140`, `PORTSC1 0x184`, `MODE 0x1A8` (CM_HOST=3), `OTGSC 0x1A4`, `ULPIVIEW 0x170`, host `LISTBASE 0x154`/`ASYNCLISTADDR 0x158` | `xusbps_hw.h` |

Extracted vendor references live in `scratchpad/usb_ref/` (`ps7_init.tcl/.c`, `xusbps` source).
The current `ps7_init_data.rs` (generated, do-not-edit) does **not** route USB — so `usb.rs`
does its own MIO/clock init, exactly as `gem.rs` self-configures its SLCR GEM clock.

## 3. Architecture — a border crossing

```
   FOREIGN (muscle, no phase)          BORDER               YOUR WORLD (phase/states/math)
   USB3320 PHY / ChipIdea host   →   usb.rs           →    DomainId::Usb phase state domain
   (fixed USB/EHCI standard)        (register/DMA)         (statechain-recorded lifecycle)
```

- **Muscle** — `src/board/zynq7000/usb.rs` (feature `usb`): `usb_mio_init` (SLCR unlock →
  MIO 28–39 → USB0 clock → lock), `host_init` (CMD.RST → MODE=host → VBUS/port power → run),
  `poll_port` (PORTSC connect/reset/speed), A9 global-timer `delay_ms` for real reset timing.
- **Brain** — the device lifecycle is a first-class `DomainId::Usb` state domain (the
  phase-state-domain VM was un-gated std→baremetal, like the S2 crypto un-gate). 10 states as
  phase words over Kairos0–3: `disconnected → powered → connected → reset → addressed →
  configured → hid_ready → polling` (+ fault/removed). `usb probe` drives transitions via
  `Kernel::change_domain_state`, recording proof-backed entries in the statechain.

## 4. Shell commands

```
usb probe        # bring up USB0 host, read the root port, walk the phase lifecycle
state domains    # shows `usb states=10 word_len=2` — the Usb domain live on bare metal
```
(U1 adds `usb list` → VID:PID class; U2 feeds HID keypresses to the `gpos>` shell.)

## 5. U0 proof (real hardware, 2026-08-09 — first attempt)

```
gpos> usb probe
usb0: host up @ 0xE0002000 (ULPI) -- device CONNECTED, full-speed, reset done
usb: phase lifecycle -> powered -> connected -> reset (recorded in statechain, domain=usb)
gpos> state domains
usb states=10 word_len=2 capacity=16
```
Muscle (controller + PHY up, device detected + reset) and brain (phase lifecycle recorded)
both proven. Evidence: `scripts/pz7020_test_plan_usb_u0.txt`. Commits `d5d1857` (muscle),
`746a4a3` (brain), `16ed781` (evidence).

## 6. Build / flash

```bash
cargo build --bin kairos_zynq --no-default-features \
      --features "zynq-a9,bm-full,ssh-bm,usb" --target armv7a-none-eabi --release
```
Then bootgen → SD (H:) → boot (needs **D:** unlocked — BitLocker; linker + bootgen live there).
Plug a USB keyboard/mouse into `J10`. Compile gates: arm `bm-full,usb`; host; riscv64.

## 7. U1 → U2 — PROVEN on hardware (2026-08-10)

- **U1 enumeration (PROVEN, commit `b4b7d76`):** hand-built the EHCI **async schedule** + control
  QH/qTD in Rust; `GET_DESCRIPTOR` / `SET_ADDRESS` / `SET_CONFIGURATION`; `DomainId::Usb`
  `addressed → configured`. `usb list` →
  `usb1: 1a2c:7fff class=HID addr=1 cfg=1 ep0=64 full-speed int-in=0x83 …`.
  **Root cause of the earlier block** (commit `54fd5c0`): the ULPI MIO TRI map had **CLK (MIO36)
  and DATA5 (MIO37) swapped** — `0x1205` belongs on MIO36/CLK, `0x1204` on MIO37/DATA5. Not an
  EHCI/PHY design issue; a two-entry data error. After the swap: PHY VID `0x0424`, enumeration works.
- **U2 HID input (PROVEN, commit `a239f31`):** `SET_PROTOCOL(boot)` + an interrupt-IN poll (single
  IN qTD on the async schedule — no periodic frame list needed for one HID device). `usb keys`
  captured real 8-byte boot reports (e.g. `00 00 08 1c …` = e, y). `parse_config` records all HID
  interrupt-IN endpoints and **prefers the boot-keyboard interface** (composite-device fix). Brain
  → `hid_ready`.
- **Next:** decode HID-usage→ASCII and feed the shell `IoBuffer` so a USB keyboard drives `gpos>`
  standalone; mouse (3-byte report) follows.

## Honest caveats
- Enumeration/input are proven; the decode→`IoBuffer` shell bridge (keyboard drives `gpos>`
  without UART) is the remaining U2 step.
- The statechain root-view command is still std-only (cosmetic) — the transition *recording*
  works on bare metal.
