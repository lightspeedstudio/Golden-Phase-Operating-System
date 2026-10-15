#!/usr/bin/env python3
"""
ai_trace_phase.py — Observable AI artifact → GPL phase trajectory report.

Encodes textual artifacts (conversation excerpts, public prompt snippets) into
phase states using golden_phase_runtime.protocol.encoder (byte↔phase codec).
Topology and checksum use existing GPL modules — compose is never reimplemented.

This is trace archaeology on observable bytes, NOT decoding LLM hidden states.
"""

from __future__ import annotations

import argparse
import cmath
import json
import math
import sys
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

# Bootstrap repo root so golden_phase_runtime imports work from kairos/scripts/.
_REPO_ROOT = Path(__file__).resolve().parents[2]
if str(_REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(_REPO_ROOT))

from golden_phase_runtime.core.gphase_core import compose  # noqa: E402
from golden_phase_runtime.protocol.checksum import gpl_checksum  # noqa: E402
from golden_phase_runtime.protocol.encoder import bytes_to_phase  # noqa: E402
from golden_phase_runtime.protocol.topology_checker import (  # noqa: E402
    SLIDING,
    check_topology,
)

PHASE_LABELS = ("Phi0", "Phi1", "Phi2", "Phi3")


def _load_text(path: Path) -> tuple[str, str]:
    raw = path.read_text(encoding="utf-8", errors="replace")
    if path.suffix.lower() == ".jsonl":
        return _extract_jsonl_text(raw), "jsonl"
    return raw, "text"


def _extract_jsonl_text(raw: str) -> str:
    parts: list[str] = []
    for line in raw.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        if obj.get("type") == "turn_ended":
            continue
        msg = obj.get("message") or {}
        for block in msg.get("content") or []:
            if block.get("type") == "text":
                parts.append(block.get("text", ""))
    return "\n".join(parts)


def _dft_real(signal: list[float]) -> list[float]:
    """Simple DFT magnitude spectrum (proxy signal, not neural FFT)."""
    n = len(signal)
    if n == 0:
        return []
    mags: list[float] = []
    for k in range(n // 2 + 1):
        acc = 0j
        for t, x in enumerate(signal):
            acc += x * cmath.exp(-2j * math.pi * k * t / n)
        mags.append(abs(acc) / n)
    return mags


def _pair_transitions(states: list[int]) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for i in range(len(states) - 1):
        a, b = states[i], states[i + 1]
        out.append(
            {
                "index": i,
                "from": a,
                "to": b,
                "compose": compose(a, b),
                "from_label": PHASE_LABELS[a],
                "to_label": PHASE_LABELS[b],
                "compose_label": PHASE_LABELS[compose(a, b)],
            }
        )
    return out


def _phase_histogram(states: list[int]) -> dict[str, int]:
    counts = Counter(states)
    return {PHASE_LABELS[i]: counts.get(i, 0) for i in range(4)}


def analyze_artifact(
    text: str,
    *,
    source: str,
    input_format: str,
    max_bytes: int | None,
) -> dict[str, Any]:
    data = text.encode("utf-8")
    if max_bytes is not None:
        data = data[:max_bytes]

    states = bytes_to_phase(data)
    acc, chi = gpl_checksum(states)
    topo = check_topology(states, mode=SLIDING)

    byte_values = list(data)
    phase_indices = [float(s) for s in states]

    fft_bytes = _dft_real([float(b) for b in byte_values])
    fft_phases = _dft_real(phase_indices)

    # Dominant frequency bins (skip DC at k=0 for rhythm proxy).
    def _top_bins(mags: list[float], label: str) -> list[dict[str, Any]]:
        if len(mags) <= 1:
            return []
        ranked = sorted(enumerate(mags[1:], start=1), key=lambda x: x[1], reverse=True)
        return [
            {"bin": k, "magnitude": round(m, 6), "signal": label}
            for k, m in ranked[:5]
        ]

    transitions_sample = _pair_transitions(states)[:24]
    topo_sample = topo["results"][:24]

    return {
        "meta": {
            "tool": "kairos/scripts/ai_trace_phase.py",
            "generated_at": datetime.now(timezone.utc).isoformat(),
            "source": source,
            "input_format": input_format,
            "disclaimer": (
                "Observable trace archaeology on UTF-8 bytes. "
                "Does NOT read LLM weights, activations, or hidden prompts."
            ),
        },
        "input": {
            "byte_length": len(data),
            "phase_length": len(states),
            "utf8_preview": data[:120].decode("utf-8", errors="replace"),
        },
        "encoding": {
            "codec": "golden_phase_runtime.protocol.encoder.bytes_to_phase",
            "states_per_byte": 4,
            "phase_histogram": _phase_histogram(states),
            "gpl_checksum": {
                "acc": acc,
                "acc_label": PHASE_LABELS[acc],
                "chi": round(chi, 12),
            },
        },
        "trajectory": {
            "pair_transitions_sample": transitions_sample,
            "pair_transition_count": max(0, len(states) - 1),
            "topology_sliding": {
                "triple_count": topo["count"],
                "summary": topo["summary"],
                "sample": topo_sample,
            },
        },
        "proxy_fft": {
            "note": (
                "DFT on byte values and phase-index stream — exploratory rhythm "
                "proxy only, not neural-network frequency content."
            ),
            "byte_spectrum_top": _top_bins(fft_bytes, "utf8_bytes"),
            "phase_index_spectrum_top": _top_bins(fft_phases, "phase_indices"),
            "byte_spectrum": [round(m, 6) for m in fft_bytes[:32]],
            "phase_index_spectrum": [round(m, 6) for m in fft_phases[:32]],
        },
    }


def write_html(report: dict[str, Any], path: Path) -> None:
    hist = report["encoding"]["phase_histogram"]
    byte_spec = report["proxy_fft"]["byte_spectrum"]
    phase_spec = report["proxy_fft"]["phase_index_spectrum"]
    topo = report["trajectory"]["topology_sliding"]["summary"]

    def _bars(values: list[float], color: str) -> str:
        if not values:
            return ""
        vmax = max(values) or 1.0
        bars = []
        w = 640 / max(len(values), 1)
        for i, v in enumerate(values):
            h = 180 * (v / vmax)
            x = 40 + i * w
            y = 220 - h
            bars.append(
                f'<rect x="{x:.1f}" y="{y:.1f}" width="{max(w - 1, 1):.1f}" '
                f'height="{h:.1f}" fill="{color}"/>'
            )
        return "\n".join(bars)

    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>AI Phase Trace — {report['meta']['source']}</title>
  <style>
    :root {{ --bg:#0b1020; --panel:#121933; --text:#e8ecff; --muted:#b5c0eb;
             --accent:#7bc4ff; --accent2:#8ef0d1; --line:#31406f; }}
    body {{ margin:0; font-family:Arial,sans-serif; background:var(--bg);
            color:var(--text); line-height:1.5; }}
    .page {{ max-width:1100px; margin:0 auto; padding:24px 20px 48px; }}
    h1 {{ color:var(--accent); margin:0 0 8px; }}
    .sub {{ color:var(--muted); margin:0 0 24px; }}
    section {{ background:var(--panel); border:1px solid var(--line);
               border-radius:14px; padding:20px; margin-bottom:20px; }}
    h2 {{ color:var(--accent2); margin:0 0 12px; font-size:1.15rem; }}
    svg {{ width:100%; height:auto; display:block; }}
    table {{ width:100%; border-collapse:collapse; font-size:0.9rem; }}
    th,td {{ border:1px solid var(--line); padding:8px; text-align:left; }}
    th {{ background:rgba(123,196,255,0.1); }}
    .note {{ color:var(--muted); font-size:0.88rem; margin-top:10px; }}
    code {{ color:var(--accent); }}
  </style>
</head>
<body>
  <div class="page">
    <h1>AI Phase Trace Report</h1>
    <p class="sub">Observable artifact → GPL phase topology · NOT neural decoding</p>

    <section>
      <h2>Input summary</h2>
      <table>
        <tr><th>Source</th><td>{report['meta']['source']}</td></tr>
        <tr><th>Bytes</th><td>{report['input']['byte_length']}</td></tr>
        <tr><th>Phase states</th><td>{report['input']['phase_length']}</td></tr>
        <tr><th>GPL checksum</th><td>acc={report['encoding']['gpl_checksum']['acc_label']}
          chi={report['encoding']['gpl_checksum']['chi']}</td></tr>
        <tr><th>Preview</th><td><code>{report['input']['utf8_preview'][:200]}</code></td></tr>
      </table>
      <p class="note">{report['meta']['disclaimer']}</p>
    </section>

    <section>
      <h2>Phase histogram</h2>
      <svg viewBox="0 0 640 260" xmlns="http://www.w3.org/2000/svg">
        <text x="80" y="220" fill="#b5c0eb" text-anchor="middle">Phi0 {hist['Phi0']}</text>
        <text x="220" y="220" fill="#b5c0eb" text-anchor="middle">Phi1 {hist['Phi1']}</text>
        <text x="380" y="220" fill="#b5c0eb" text-anchor="middle">Phi2 {hist['Phi2']}</text>
        <text x="540" y="220" fill="#b5c0eb" text-anchor="middle">Phi3 {hist['Phi3']}</text>
        <rect x="40" y="{220 - min(180, hist['Phi0'])}"
              width="80" height="{min(180, hist['Phi0'])}" fill="#7bc4ff"/>
        <rect x="180" y="{220 - min(180, hist['Phi1'])}"
              width="80" height="{min(180, hist['Phi1'])}" fill="#8ef0d1"/>
        <rect x="340" y="{220 - min(180, hist['Phi2'])}"
              width="80" height="{min(180, hist['Phi2'])}" fill="#ffd27d"/>
        <rect x="500" y="{220 - min(180, hist['Phi3'])}"
              width="80" height="{min(180, hist['Phi3'])}" fill="#ff9f9f"/>
        <line x1="20" y1="230" x2="620" y2="230" stroke="#31406f"/>
      </svg>
    </section>

    <section>
      <h2>Topology (sliding triples)</h2>
      <p>same={topo['same']} · reflection={topo['reflection']} · error={topo['error']}</p>
    </section>

    <section>
      <h2>Proxy FFT — byte values (first 32 bins)</h2>
      <svg viewBox="0 0 680 260" xmlns="http://www.w3.org/2000/svg">
        <line x1="40" y1="220" x2="640" y2="220" stroke="#31406f"/>
        {_bars(byte_spec, "#7bc4ff")}
      </svg>
      <p class="note">{report['proxy_fft']['note']}</p>
    </section>

    <section>
      <h2>Proxy FFT — phase index stream (first 32 bins)</h2>
      <svg viewBox="0 0 680 260" xmlns="http://www.w3.org/2000/svg">
        <line x1="40" y1="220" x2="640" y2="220" stroke="#31406f"/>
        {_bars(phase_spec, "#8ef0d1")}
      </svg>
    </section>
  </div>
</body>
</html>
"""
    path.write_text(html, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Encode observable AI text artifacts into GPL phase trajectories."
    )
    parser.add_argument(
        "input",
        type=Path,
        help="UTF-8 text file or .jsonl conversation transcript",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        default=None,
        help="Write JSON report (default: <input>.phase_trace.json)",
    )
    parser.add_argument(
        "--html-out",
        type=Path,
        default=None,
        help="Write HTML chart (default: <input>.phase_trace.html)",
    )
    parser.add_argument(
        "--max-bytes",
        type=int,
        default=None,
        help="Truncate input to first N UTF-8 bytes",
    )
    args = parser.parse_args()

    if not args.input.is_file():
        print(f"error: input not found: {args.input}", file=sys.stderr)
        return 1

    text, fmt = _load_text(args.input)
    if not text.strip():
        print("error: no text extracted from input", file=sys.stderr)
        return 1

    report = analyze_artifact(
        text,
        source=str(args.input),
        input_format=fmt,
        max_bytes=args.max_bytes,
    )

    json_out = args.json_out or args.input.with_suffix(".phase_trace.json")
    html_out = args.html_out or args.input.with_suffix(".phase_trace.html")

    json_out.write_text(json.dumps(report, indent=2), encoding="utf-8")
    write_html(report, html_out)

    print(f"source      : {args.input}")
    print(f"bytes       : {report['input']['byte_length']}")
    print(f"phase states: {report['input']['phase_length']}")
    print(
        f"gpl_checksum: acc={report['encoding']['gpl_checksum']['acc_label']} "
        f"chi={report['encoding']['gpl_checksum']['chi']}"
    )
    topo = report["trajectory"]["topology_sliding"]["summary"]
    print(f"topology    : same={topo['same']} reflection={topo['reflection']}")
    print(f"json        : {json_out}")
    print(f"html        : {html_out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
