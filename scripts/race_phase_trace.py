#!/usr/bin/env python3
"""
race_phase_trace.py — JSONL race event log → GPL phase trajectory + race analysis.

Ingests structured race-counter events, encodes log bytes with the existing GPL
codec (bytes_to_phase, gpl_checksum, check_topology), then compares honest
heuristic signals (reflection bursts, checksum drift, transition entropy) against
ground-truth lost-update windows from the event log.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

_REPO_ROOT = Path(__file__).resolve().parents[2]
if str(_REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(_REPO_ROOT))

from golden_phase_runtime.core.gphase_core import compose  # noqa: E402
from golden_phase_runtime.protocol.checksum import (  # noqa: E402
    GROUND_STATE,
    STATE_SHEET,
    gpl_checksum,
)
from golden_phase_runtime.protocol.encoder import bytes_to_phase  # noqa: E402
from golden_phase_runtime.protocol.topology_checker import (  # noqa: E402
    SLIDING,
    check_topology,
)

PHASE_LABELS = ("Phi0", "Phi1", "Phi2", "Phi3")


def _load_events(path: Path) -> tuple[list[dict[str, Any]], bytes, list[dict[str, Any]]]:
    raw = path.read_bytes()
    events: list[dict[str, Any]] = []
    line_spans: list[dict[str, Any]] = []
    offset = 0
    for line in raw.splitlines(keepends=True):
        start = offset
        offset += len(line)
        text = line.strip()
        if not text:
            continue
        obj = json.loads(text)
        events.append(obj)
        line_spans.append(
            {
                "event_index": len(events) - 1,
                "byte_start": start,
                "byte_end": offset,
                "seq": obj.get("seq"),
                "thread": obj.get("thread"),
                "op": obj.get("op"),
            }
        )
    return events, raw, line_spans


def _pair_transitions(states: list[int]) -> list[tuple[int, int]]:
    return [(states[i], states[i + 1]) for i in range(len(states) - 1)]


def _transition_entropy(pairs: list[tuple[int, int]]) -> float:
    if not pairs:
        return 0.0
    counts = Counter(pairs)
    total = len(pairs)
    ent = 0.0
    for c in counts.values():
        p = c / total
        ent -= p * math.log2(p)
    return ent


def _phase_histogram(states: list[int]) -> dict[str, int]:
    counts = Counter(states)
    return {PHASE_LABELS[i]: counts.get(i, 0) for i in range(4)}


def _detect_lost_updates(events: list[dict[str, Any]]) -> dict[str, Any]:
    """Ground truth: overlapping RMW on the same value_before from different threads."""
    writes = [
        e
        for e in events
        if e.get("op") == "write" and "thread" in e and "value_before" in e
    ]
    conflicts: list[dict[str, Any]] = []

    # Pair writes that reused the same counter snapshot (lost update signature).
    by_before: dict[int, list[dict[str, Any]]] = {}
    for w in writes:
        vb = w["value_before"]
        by_before.setdefault(vb, []).append(w)

    for vb, group in by_before.items():
        threads = {w["thread"] for w in group}
        if len(group) >= 2 and len(threads) >= 2:
            conflicts.append(
                {
                    "value_before": vb,
                    "writes": [
                        {
                            "seq": w.get("seq"),
                            "thread": w.get("thread"),
                            "iter": w.get("iter"),
                            "value_after": w.get("value_after"),
                        }
                        for w in sorted(group, key=lambda x: x.get("seq", 0))
                    ],
                    "kind": "duplicate_base_write",
                }
            )

    # Interleaved burst: adjacent events alternate threads (contention proxy).
    interleave_windows: list[dict[str, Any]] = []
    run_start = None
    run_len = 0
    prev_thread = None
    for i, e in enumerate(events):
        tid = e.get("thread")
        if tid is None:
            continue
        if prev_thread is not None and tid != prev_thread:
            if run_start is None:
                run_start = i - 1
            run_len += 1
        else:
            if run_len >= 3 and run_start is not None:
                interleave_windows.append(
                    {
                        "event_start": run_start,
                        "event_end": i,
                        "alternations": run_len,
                    }
                )
            run_start = None
            run_len = 0
        prev_thread = tid
    if run_len >= 3 and run_start is not None:
        interleave_windows.append(
            {
                "event_start": run_start,
                "event_end": len(events) - 1,
                "alternations": run_len,
            }
        )

    summary_evt = next((e for e in events if e.get("type") == "run_summary"), {})
    return {
        "lost_update_conflicts": conflicts,
        "conflict_count": len(conflicts),
        "interleave_windows": interleave_windows[:48],
        "interleave_window_count": len(interleave_windows),
        "run_summary": summary_evt,
        "race_manifested": bool(summary_evt.get("race_manifested")),
        "lost_updates": summary_evt.get("lost_updates", 0),
    }


def _prefix_checksums(states: list[int]) -> tuple[list[int], list[float]]:
    accs = [GROUND_STATE]
    chis = [0.0]
    acc = GROUND_STATE
    chi = 0.0
    for s in states:
        acc = compose(acc, s)
        chi += STATE_SHEET[s]
        accs.append(acc)
        chis.append(chi)
    return accs, chis


def _window_metrics_from_states(
    states: list[int],
    phase_end: int,
    *,
    accs: list[int],
    chis: list[float],
    local_triples: int = 96,
) -> dict[str, Any]:
    """Metrics on a phase prefix; local topology uses trailing triple window only."""
    tail_start = max(0, phase_end - local_triples - 2)
    topo = check_topology(states[tail_start:phase_end], mode=SLIDING)
    pairs = _pair_transitions(states[max(0, phase_end - local_triples - 1) : phase_end])
    s = topo["summary"]
    triples = max(1, topo["count"])
    acc = accs[phase_end]
    chi = chis[phase_end]
    return {
        "phase_end": phase_end,
        "phase_length": phase_end,
        "gpl_checksum": {"acc": acc, "acc_label": PHASE_LABELS[acc], "chi": round(chi, 12)},
        "topology": {
            "same": s["same"],
            "reflection": s["reflection"],
            "error": s["error"],
            "reflection_rate": round(s["reflection"] / triples, 6),
        },
        "transition_entropy": round(_transition_entropy(pairs), 6),
    }


def _byte_to_phase_end(raw: bytes, byte_end: int) -> int:
    return byte_end * 4


def _rolling_windows(
    raw: bytes,
    states: list[int],
    accs: list[int],
    chis: list[float],
    line_spans: list[dict[str, Any]],
    *,
    stride: int,
    max_windows: int,
) -> list[dict[str, Any]]:
    if not line_spans:
        return []
    indices = list(range(0, len(line_spans), stride))
    if len(indices) > max_windows:
        step = max(1, len(indices) // max_windows)
        indices = indices[::step][:max_windows]

    windows: list[dict[str, Any]] = []
    prev_chi: float | None = None
    for i in indices:
        span = line_spans[i]
        phase_end = _byte_to_phase_end(raw, span["byte_end"])
        m = _window_metrics_from_states(
            states, phase_end, accs=accs, chis=chis
        )
        chi = m["gpl_checksum"]["chi"]
        drift = 0.0 if prev_chi is None else abs(chi - prev_chi)
        prev_chi = chi
        windows.append(
            {
                "event_index": span["event_index"],
                "seq": span.get("seq"),
                "thread": span.get("thread"),
                "op": span.get("op"),
                "byte_end": span["byte_end"],
                "chi_drift": round(drift, 12),
                **m,
            }
        )
    return windows


def _flag_anomalies(windows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    if not windows:
        return []
    ref_rates = [w["topology"]["reflection_rate"] for w in windows]
    entropies = [w["transition_entropy"] for w in windows]
    drifts = [w["chi_drift"] for w in windows]

    def _mean(xs: list[float]) -> float:
        return sum(xs) / len(xs) if xs else 0.0

    def _stdev(xs: list[float], mu: float) -> float:
        if len(xs) < 2:
            return 0.0
        var = sum((x - mu) ** 2 for x in xs) / len(xs)
        return math.sqrt(var)

    mu_r, mu_e, mu_d = _mean(ref_rates), _mean(entropies), _mean(drifts)
    sd_r, sd_e, sd_d = _stdev(ref_rates, mu_r), _stdev(entropies, mu_e), _stdev(drifts, mu_d)

    flagged: list[dict[str, Any]] = []
    for w in windows:
        reasons: list[str] = []
        rr = w["topology"]["reflection_rate"]
        ent = w["transition_entropy"]
        drift = w["chi_drift"]
        if sd_r > 0 and rr >= mu_r + sd_r:
            reasons.append("reflection_spike")
        if sd_e > 0 and ent >= mu_e + sd_e:
            reasons.append("entropy_spike")
        if sd_d > 0 and drift >= mu_d + sd_d:
            reasons.append("chi_drift_spike")
        if reasons:
            flagged.append(
                {
                    "event_index": w["event_index"],
                    "seq": w.get("seq"),
                    "reasons": reasons,
                    "reflection_rate": rr,
                    "transition_entropy": ent,
                    "chi_drift": drift,
                }
            )
    return flagged


def _correlate(
    ground: dict[str, Any],
    anomalies: list[dict[str, Any]],
    *,
    event_radius: int,
) -> dict[str, Any]:
    conflict_seqs: set[int] = set()
    for c in ground["lost_update_conflicts"]:
        for w in c["writes"]:
            if w.get("seq") is not None:
                conflict_seqs.add(w["seq"])

    def _near_conflict(seq: int | None) -> bool:
        if seq is None:
            return False
        return any(abs(seq - cs) <= event_radius * 2 for cs in conflict_seqs)

    hits = [a for a in anomalies if _near_conflict(a.get("seq"))]
    return {
        "conflict_seq_count": len(conflict_seqs),
        "anomaly_count": len(anomalies),
        "anomalies_near_conflict": len(hits),
        "overlap_ratio": round(len(hits) / max(1, len(anomalies)), 4),
        "sample_hits": hits[:12],
    }


def analyze_race_log(
    path: Path, *, window_stride: int = 16, max_windows: int = 200
) -> dict[str, Any]:
    events, raw, line_spans = _load_events(path)
    if not events:
        raise ValueError("no events in log")

    states = bytes_to_phase(raw)
    acc, chi = gpl_checksum(states)
    accs, chis = _prefix_checksums(states)
    topo = check_topology(states, mode=SLIDING)

    ground = _detect_lost_updates(events)
    windows = _rolling_windows(
        raw, states, accs, chis, line_spans, stride=window_stride, max_windows=max_windows
    )
    anomalies = _flag_anomalies(windows)
    correlation = _correlate(ground, anomalies, event_radius=window_stride)

    ref_rate = topo["summary"]["reflection"] / max(1, topo["count"])
    verdict = (
        "partial_correlation"
        if correlation["overlap_ratio"] > 0.15 and ground["race_manifested"]
        else "weak_or_no_correlation"
    )
    if correlation["overlap_ratio"] > 0.4 and ground["race_manifested"]:
        verdict = "moderate_correlation"

    return {
        "meta": {
            "tool": "kairos/scripts/race_phase_trace.py",
            "generated_at": datetime.now(timezone.utc).isoformat(),
            "source": str(path),
            "disclaimer": (
                "Phase encoding fingerprints UTF-8 log bytes. Topology/checksum "
                "signals are exploratory heuristics — not guaranteed race detectors."
            ),
        },
        "input": {
            "byte_length": len(raw),
            "event_count": len(events),
            "phase_length": len(states),
            "utf8_preview": raw[:160].decode("utf-8", errors="replace"),
        },
        "encoding": {
            "codec": "golden_phase_runtime.protocol.encoder.bytes_to_phase",
            "phase_histogram": _phase_histogram(states),
            "gpl_checksum": {
                "acc": acc,
                "acc_label": PHASE_LABELS[acc],
                "chi": round(chi, 12),
            },
        },
        "trajectory": {
            "topology_sliding": {
                "triple_count": topo["count"],
                "summary": topo["summary"],
                "reflection_rate": round(ref_rate, 6),
                "sample": topo["results"][:24],
            },
        },
        "ground_truth": ground,
        "rolling_windows": {
            "stride_events": window_stride,
            "max_windows": max_windows,
            "count": len(windows),
            "sample": windows[:24],
        },
        "phase_anomalies": anomalies[:48],
        "correlation": correlation,
        "verdict": {
            "label": verdict,
            "race_manifested": ground["race_manifested"],
            "notes": (
                "Ground truth uses duplicate value_before writes across threads. "
                "Phase anomalies use reflection/entropy/chi-drift spikes vs run baseline."
            ),
        },
    }


def write_html(report: dict[str, Any], path: Path) -> None:
    hist = report["encoding"]["phase_histogram"]
    topo = report["trajectory"]["topology_sliding"]["summary"]
    windows = report["rolling_windows"]["sample"]
    ground = report["ground_truth"]
    corr = report["correlation"]
    verdict = report["verdict"]

    def _bars(values: list[float], color: str, height: float = 180.0) -> str:
        if not values:
            return ""
        vmax = max(values) or 1.0
        bars = []
        w = 640 / max(len(values), 1)
        for i, v in enumerate(values):
            h = height * (v / vmax)
            x = 40 + i * w
            y = 220 - h
            bars.append(
                f'<rect x="{x:.1f}" y="{y:.1f}" width="{max(w - 1, 1):.1f}" '
                f'height="{h:.1f}" fill="{color}"/>'
            )
        return "\n".join(bars)

    ref_series = [w["topology"]["reflection_rate"] for w in windows]
    ent_series = [w["transition_entropy"] for w in windows]
    drift_series = [w["chi_drift"] for w in windows]

    conflict_rows = ""
    for c in ground["lost_update_conflicts"][:8]:
        writes = ", ".join(
            f"t{w['thread']}@{w['seq']}" for w in c["writes"][:4]
        )
        conflict_rows += (
            f"<tr><td>{c['value_before']}</td><td>{writes}</td></tr>"
        )
    if not conflict_rows:
        conflict_rows = "<tr><td colspan='2'>none detected in sample window</td></tr>"

    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Race Phase Trace — {report['meta']['source']}</title>
  <style>
    :root {{ --bg:#0b1020; --panel:#121933; --text:#e8ecff; --muted:#b5c0eb;
             --accent:#7bc4ff; --accent2:#8ef0d1; --warn:#ff9f9f; --line:#31406f; }}
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
    .verdict {{ color:var(--warn); font-weight:bold; }}
    code {{ color:var(--accent); }}
  </style>
</head>
<body>
  <div class="page">
    <h1>Race Phase Trace Report</h1>
    <p class="sub">JSONL race log → GPL phase trajectory · ground truth vs heuristics</p>

    <section>
      <h2>Run summary</h2>
      <table>
        <tr><th>Source</th><td>{report['meta']['source']}</td></tr>
        <tr><th>Events</th><td>{report['input']['event_count']}</td></tr>
        <tr><th>Bytes / phases</th><td>{report['input']['byte_length']} / {report['input']['phase_length']}</td></tr>
        <tr><th>Race manifested</th><td>{ground['race_manifested']} (lost={ground.get('lost_updates', '?')})</td></tr>
        <tr><th>GPL checksum</th><td>acc={report['encoding']['gpl_checksum']['acc_label']} chi={report['encoding']['gpl_checksum']['chi']}</td></tr>
        <tr><th>Verdict</th><td class="verdict">{verdict['label']}</td></tr>
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
        <rect x="40" y="{220 - min(180, hist['Phi0'])}" width="80" height="{min(180, hist['Phi0'])}" fill="#7bc4ff"/>
        <rect x="180" y="{220 - min(180, hist['Phi1'])}" width="80" height="{min(180, hist['Phi1'])}" fill="#8ef0d1"/>
        <rect x="340" y="{220 - min(180, hist['Phi2'])}" width="80" height="{min(180, hist['Phi2'])}" fill="#ffd27d"/>
        <rect x="500" y="{220 - min(180, hist['Phi3'])}" width="80" height="{min(180, hist['Phi3'])}" fill="#ff9f9f"/>
        <line x1="20" y1="230" x2="620" y2="230" stroke="#31406f"/>
      </svg>
    </section>

    <section>
      <h2>Topology (sliding triples)</h2>
      <p>same={topo['same']} · reflection={topo['reflection']} · error={topo['error']}</p>
    </section>

    <section>
      <h2>Ground truth conflicts (duplicate value_before)</h2>
      <table>
        <tr><th>value_before</th><th>writes</th></tr>
        {conflict_rows}
      </table>
      <p>conflicts={ground['conflict_count']} · interleave_windows={ground['interleave_window_count']}</p>
    </section>

    <section>
      <h2>Rolling reflection rate (sample windows)</h2>
      <svg viewBox="0 0 680 260" xmlns="http://www.w3.org/2000/svg">
        <line x1="40" y1="220" x2="640" y2="220" stroke="#31406f"/>
        {_bars(ref_series, "#ff9f9f")}
      </svg>
    </section>

    <section>
      <h2>Rolling transition entropy (sample)</h2>
      <svg viewBox="0 0 680 260" xmlns="http://www.w3.org/2000/svg">
        <line x1="40" y1="220" x2="640" y2="220" stroke="#31406f"/>
        {_bars(ent_series, "#8ef0d1")}
      </svg>
    </section>

    <section>
      <h2>Chi drift between windows (sample)</h2>
      <svg viewBox="0 0 680 260" xmlns="http://www.w3.org/2000/svg">
        <line x1="40" y1="220" x2="640" y2="220" stroke="#31406f"/>
        {_bars(drift_series, "#7bc4ff")}
      </svg>
    </section>

    <section>
      <h2>Correlation</h2>
      <p>anomalies={corr['anomaly_count']} · near conflict seqs={corr['anomalies_near_conflict']} · overlap={corr['overlap_ratio']}</p>
      <p class="note">{verdict['notes']}</p>
    </section>
  </div>
</body>
</html>
"""
    path.write_text(html, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Encode race JSONL logs into GPL phase trajectories with race heuristics."
    )
    parser.add_argument("input", type=Path, help="race_counter JSONL event log")
    parser.add_argument(
        "--json-out",
        type=Path,
        default=None,
        help="JSON report path (default: <input>.race_phase.json)",
    )
    parser.add_argument(
        "--html-out",
        type=Path,
        default=None,
        help="HTML chart path (default: <input>.race_phase.html)",
    )
    parser.add_argument(
        "--window-stride",
        type=int,
        default=16,
        help="Sample every N events for rolling window metrics",
    )
    parser.add_argument(
        "--max-windows",
        type=int,
        default=200,
        help="Cap rolling window samples for large logs",
    )
    args = parser.parse_args()

    if not args.input.is_file():
        print(f"error: input not found: {args.input}", file=sys.stderr)
        return 1

    report = analyze_race_log(
        args.input,
        window_stride=args.window_stride,
        max_windows=args.max_windows,
    )

    json_out = args.json_out or args.input.with_suffix(".race_phase.json")
    html_out = args.html_out or args.input.with_suffix(".race_phase.html")

    json_out.write_text(json.dumps(report, indent=2), encoding="utf-8")
    write_html(report, html_out)

    gt = report["ground_truth"]
    corr = report["correlation"]
    print(f"source      : {args.input}")
    print(f"events      : {report['input']['event_count']}")
    print(f"race        : manifested={gt['race_manifested']} lost={gt.get('lost_updates')}")
    print(f"conflicts   : {gt['conflict_count']}")
    print(f"anomalies   : {len(report['phase_anomalies'])}")
    print(f"correlation : overlap={corr['overlap_ratio']} near={corr['anomalies_near_conflict']}")
    print(f"verdict     : {report['verdict']['label']}")
    print(f"json        : {json_out}")
    print(f"html        : {html_out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
