#!/usr/bin/env python3
"""Extract Ethernet/GEM/PHY-relevant text from PZ-StarLite manufacturer packs."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

import fitz

BASE = Path(r"C:\Users\richa\OneDrive\Desktop\FullOS")
OUT = (
    BASE
    / "kairos"
    / "fpga"
    / "zynq"
    / "evidence"
    / "kairos_zynq_boot"
    / "manufacturer_pack_read_20260801"
)
TERMS = re.compile(
    r"RTL8211|RGMII|GEM0|\bGEM\b|GPHY|PS_GPHY|PL_GPHY|MDIO|\bMDC\b|Ethernet|"
    r"以太网|PHY|TXC|TX_CTL|enet|ENET0|MIO\s*1[6-9]|MIO\s*2[0-7]|MIO\s*5[23]",
    re.I,
)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    pdfs: list[Path] = []
    for root_name in (
        "01.芯片手册--datasheet",
        "04.硬件相关--Hardware",
        "05.教程和源码--Course_Demos",
    ):
        root = BASE / root_name
        if root.is_dir():
            pdfs.extend(root.rglob("*.pdf"))
            pdfs.extend(root.rglob("*.PDF"))

    # Prefer eth-relevant filenames first
    def score(p: Path) -> int:
        s = p.name.lower()
        n = 0
        for k, w in (
            ("rtl8211", 100),
            ("schematic", 90),
            ("硬件用户", 80),
            ("linux", 70),
            ("sdk", 60),
            ("fpga", 50),
            ("pynq", 40),
            ("hls", 30),
        ):
            if k in s or k in str(p):
                n += w
        # skip enormous pinout/transceiver ug unless needed
        if "ug475" in s or "ug476" in s:
            n -= 200
        return n

    pdfs = sorted(set(pdfs), key=score, reverse=True)

    summary = []
    for p in pdfs:
        rel = str(p.relative_to(BASE))
        print(f"READING {rel}", flush=True)
        try:
            doc = fitz.open(p)
        except Exception as e:  # noqa: BLE001
            summary.append({"file": rel, "error": str(e)})
            continue

        n = doc.page_count
        hits = []
        for i in range(n):
            try:
                t = doc.load_page(i).get_text("text")
            except Exception:
                continue
            if not TERMS.search(t):
                continue
            lines = [ln for ln in t.splitlines() if TERMS.search(ln)]
            snippet = "\n".join(lines[:100]) if lines else t[:4000]
            hits.append({"page": i + 1, "snippet": snippet[:5000]})

        safe = re.sub(r"[^\w.\-]+", "_", p.name)[:120]
        hit_path = OUT / f"{safe}.eth_hits.txt"
        with hit_path.open("w", encoding="utf-8", errors="replace") as f:
            f.write(f"FILE: {rel}\nPAGES: {n}\nHITS: {len(hits)}\n\n")
            for h in hits[:80]:
                f.write(f"\n===== PAGE {h['page']} =====\n")
                f.write(h["snippet"])
                f.write("\n")

        # Full extract for core manufacturer docs (not giant UG pinouts)
        want_full = any(
            x in p.name
            for x in (
                "RTL8211",
                "Schematic",
                "硬件用户",
                "开发指南",
                "开发板之",
            )
        ) or ("PZ-StarLite" in p.name and p.stat().st_size < 40_000_000)
        if want_full and p.stat().st_size < 40_000_000 and n <= 400:
            full_path = OUT / f"{safe}.full.txt"
            with full_path.open("w", encoding="utf-8", errors="replace") as f:
                for i in range(n):
                    f.write(f"\n\n===== PAGE {i + 1} =====\n")
                    f.write(doc.load_page(i).get_text("text"))

        summary.append(
            {
                "file": rel,
                "pages": n,
                "eth_hits": len(hits),
                "hit_file": hit_path.name,
            }
        )
        doc.close()

    boot_info = []
    boot_root = BASE / "03.开机测试--Boot Test"
    if boot_root.is_dir():
        for b in boot_root.rglob("*"):
            if not b.is_file():
                continue
            h = hashlib.sha256()
            with b.open("rb") as f:
                for chunk in iter(lambda: f.read(1 << 20), b""):
                    h.update(chunk)
            boot_info.append(
                {
                    "file": str(b.relative_to(BASE)),
                    "size": b.stat().st_size,
                    "sha256": h.hexdigest(),
                }
            )

    soft = []
    soft_root = BASE / "02.软件和驱动--Software Tools and Drivers"
    if soft_root.is_dir():
        for b in soft_root.rglob("*"):
            if b.is_file() and b.suffix.lower() in {".txt", ".ini", ".docx", ".md"}:
                soft.append(
                    {
                        "file": str(b.relative_to(BASE)),
                        "size": b.stat().st_size,
                    }
                )
            elif b.is_file() and b.name.lower() in {"readme.txt", "history.txt", "order.txt"}:
                soft.append(
                    {
                        "file": str(b.relative_to(BASE)),
                        "size": b.stat().st_size,
                    }
                )

    # Read small text files from 02
    for item in soft:
        p = BASE / item["file"]
        if p.suffix.lower() == ".txt" and p.stat().st_size < 200_000:
            dest = OUT / ("02_" + re.sub(r"[^\w.\-]+", "_", p.name))
            dest.write_text(p.read_text(encoding="utf-8", errors="replace"), encoding="utf-8")

    # xlsx connector note (openpyxl if available)
    xlsx_note = []
    try:
        import openpyxl  # type: ignore

        for x in (BASE / "04.硬件相关--Hardware").rglob("*.xlsx"):
            wb = openpyxl.load_workbook(x, read_only=True, data_only=True)
            sheets = {}
            for name in wb.sheetnames:
                rows = []
                ws = wb[name]
                for i, row in enumerate(ws.iter_rows(values_only=True)):
                    if i > 200:
                        break
                    vals = ["" if c is None else str(c) for c in row]
                    joined = "\t".join(vals)
                    if TERMS.search(joined) or i < 5:
                        rows.append(joined)
                sheets[name] = rows[:80]
            outp = OUT / (re.sub(r"[^\w.\-]+", "_", x.name) + ".xlsx.txt")
            with outp.open("w", encoding="utf-8", errors="replace") as f:
                f.write(f"FILE: {x.relative_to(BASE)}\n")
                for sn, rows in sheets.items():
                    f.write(f"\n## SHEET {sn}\n")
                    f.write("\n".join(rows))
                    f.write("\n")
            xlsx_note.append(str(x.relative_to(BASE)))
            wb.close()
    except Exception as e:  # noqa: BLE001
        xlsx_note.append(f"openpyxl_error:{e}")

    (OUT / "INDEX.json").write_text(
        json.dumps(
            {
                "pdfs": summary,
                "boot": boot_info,
                "soft_textish": soft,
                "xlsx": xlsx_note,
            },
            indent=2,
            ensure_ascii=False,
        ),
        encoding="utf-8",
    )
    print("DONE", OUT)
    print(json.dumps({"pdfs": summary, "boot": boot_info}, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
