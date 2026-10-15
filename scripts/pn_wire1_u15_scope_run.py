#!/usr/bin/env python3
"""Arm PZ7020 for U15 RGMII scope (A/B/C). Does NOT invent pin results.

Runs on COM12:
  net wire init
  net wire txburst 32

Writes evidence dir with SCOPE_LETTER.txt. Operator fills A/B/C from LA/scope
on U15 (PS_GPHY / RTL8211F-CG) — not U16.

  python scripts/pn_wire1_u15_scope_run.py --port COM12
  python scripts/pn_wire1_u15_scope_run.py --port COM12 --letter A
"""

from __future__ import annotations

import argparse
import sys
import time
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def evidence_dir() -> Path:
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    d = ROOT / "fpga" / "zynq" / "evidence" / "kairos_zynq_boot" / f"pn_wire1_u15_scope_{stamp}"
    d.mkdir(parents=True, exist_ok=True)
    return d


def run_uart(port: str, baud: int, count: int, boot_timeout: float) -> str:
    try:
        import serial  # type: ignore
    except ImportError:
        print("pip install pyserial", file=sys.stderr)
        raise SystemExit(1)

    ser = serial.Serial(port, baud, timeout=0.25, write_timeout=2)
    ser.dtr = True
    ser.rts = True
    time.sleep(0.25)
    ser.reset_input_buffer()
    buf = ""
    deadline = time.time() + boot_timeout
    last = 0.0
    print(f"[u15] wait gpos> on {port} ({boot_timeout:.0f}s)", flush=True)
    while time.time() < deadline:
        chunk = ser.read(4096)
        if chunk:
            t = chunk.decode("utf-8", "replace")
            buf += t
            sys.stdout.write(t.encode("ascii", "replace").decode("ascii"))
            sys.stdout.flush()
            if "gpos> " in buf:
                break
        if time.time() - last > 3.0 and "gpos>" not in buf:
            ser.write(b"\r")
            last = time.time()
    else:
        ser.close()
        raise RuntimeError("no gpos> prompt")

    def cmd(c: str, timeout: float = 60.0) -> None:
        nonlocal buf
        before = buf.count("gpos> ")
        print(f"\n>>> {c}", flush=True)
        ser.write((c + "\r").encode("ascii"))
        d = time.time() + timeout
        while time.time() < d:
            chunk = ser.read(4096)
            if chunk:
                t = chunk.decode("utf-8", "replace")
                buf += t
                sys.stdout.write(t.encode("ascii", "replace").decode("ascii"))
                sys.stdout.flush()
                if buf.count("gpos> ") > before:
                    return
        raise TimeoutError(f"timeout after: {c}")

    cmd("net wire init")
    print(
        "\n[u15] SCOPE NOW — probe U15 TXC / TXCTL / TXD[3:0] during next burst\n",
        flush=True,
    )
    time.sleep(1.0)
    cmd(f"net wire txburst {count}")
    # Second burst for LA re-trigger
    time.sleep(0.5)
    cmd(f"net wire txburst {count}")
    ser.close()
    return buf


def write_forms(ev: Path, letter: str, uart: str, uart_err: str | None) -> None:
    (ev / "com12.txt").write_text(
        uart + (f"\n[uart_error] {uart_err}\n" if uart_err else ""),
        encoding="utf-8",
    )

    letter_norm = letter.strip().upper()
    if letter_norm not in {"A", "B", "C", "UNRECORDED"}:
        letter_norm = "UNRECORDED"

    meanings = {
        "A": "TXC + TX_CTL + TXD[3:0] all show burst activity → fault after PHY/media/PC; soft-reset/delay only after media checks",
        "B": "TXC toggles; TX_CTL/TXD idle → TX BD / STARTTX / data-path enable",
        "C": "No TXC during burst → GEM output enable / MIO / NWCTRL TX on silicon",
        "UNRECORDED": "Physical pin observation not entered — do not invent from UART txburst OK",
    }

    branch = {
        "A": "AFTER_PHY_MEDIA_PC_THEN_BMCR_SOFT_RESET_OR_DELAY",
        "B": "TX_BD_STARTTX_DATA_PATH",
        "C": "GEM_MIO_NWCTRL_TX_OUTPUT",
        "UNRECORDED": "NONE_UNTIL_LETTER",
    }[letter_norm]

    (ev / "SCOPE_LETTER.txt").write_text(
        "\n".join(
            [
                "PN-WIRE-1 U15 RGMII scope (not PhaseNet, not wire PASS)",
                f"evidence_dir={ev}",
                "chip=U15 RTL8211F-CG sheet=PZ-StarLite--GPHY(PS)",
                "probe=TXC TXCTL/TX_EN TXD[3:0]  (NOT U16 PL_GPHY)",
                "stimulus=net wire init ; net wire txburst 32 (x2)",
                f"LETTER={letter_norm}",
                f"MEANING={meanings[letter_norm]}",
                f"SOFTWARE_BRANCH={branch}",
                "NOTE=UART txburst OK is not A/B/C",
                "",
            ]
        ),
        encoding="utf-8",
    )

    (ev / "PROBE_MAP.txt").write_text(
        "\n".join(
            [
                "From manufacturer schematic / hardware user manual:",
                "  U15 RTL8211F-CG  PS_GPHY  PHY_AD[2:0]=001",
                "  PS_GPHY_GTX_CLK  -> MIO16  (TXC from MAC side)",
                "  PS_GPHY_TXD0..3  -> MIO17..20",
                "  PS_GPHY_TX_EN    -> MIO21  (TXCTL)",
                "  PS_GPHY_RX_CLK   -> MIO22",
                "  PS_GPHY_RXD0..3  -> MIO23..26",
                "  PS_GPHY_RX_DV    -> MIO27",
                "  PS_GPHY_MDC/MDIO -> MIO52/53",
                "Scope classification uses U15 RGMII TX pins during txburst.",
                "",
            ]
        ),
        encoding="utf-8",
    )


def main() -> int:
    ap = argparse.ArgumentParser(description="U15 RGMII scope arming (A/B/C)")
    ap.add_argument("--port", default="COM12")
    ap.add_argument("--baud", type=int, default=115200)
    ap.add_argument("--count", type=int, default=32)
    ap.add_argument("--boot-timeout", type=float, default=70.0)
    ap.add_argument(
        "--letter",
        default="UNRECORDED",
        help="Physical result A|B|C after you scoped; default UNRECORDED",
    )
    ap.add_argument(
        "--skip-uart",
        action="store_true",
        help="Only write evidence forms (no COM12)",
    )
    args = ap.parse_args()

    ev = evidence_dir()
    print(f"[u15] evidence={ev}", flush=True)

    uart = ""
    uart_err = None
    if not args.skip_uart:
        try:
            uart = run_uart(args.port, args.baud, args.count, args.boot_timeout)
        except Exception as e:
            uart_err = str(e)
            print(f"[u15] UART error: {e}", flush=True)

    write_forms(ev, args.letter, uart, uart_err)
    print((ev / "SCOPE_LETTER.txt").read_text(encoding="utf-8"))
    print(f"[u15] wrote {ev / 'SCOPE_LETTER.txt'}")
    if args.letter.strip().upper() not in {"A", "B", "C"}:
        print("[u15] LETTER=UNRECORDED — re-run with --letter A|B|C after you scope")
        return 2
    return 0 if uart_err is None else 1


if __name__ == "__main__":
    sys.exit(main())
