#!/usr/bin/env python3
"""
race_counter.py — Intentional shared-counter data race with JSONL event log.

Two threads perform read-modify-write increments on a shared counter without
a lock. A structured event stream records every read/write for later phase
trace analysis. Spin delay between read and write widens the race window.
"""

from __future__ import annotations

import argparse
import json
import sys
import threading
import time
from datetime import datetime, timezone
from pathlib import Path


class EventLogger:
    """Thread-safe log writer only — the counter itself is intentionally unlocked."""

    def __init__(self, sink) -> None:
        self._sink = sink
        self._lock = threading.Lock()
        self._seq = 0

    def emit(self, **fields) -> dict:
        with self._lock:
            self._seq += 1
            rec = {
                "seq": self._seq,
                "ts": datetime.now(timezone.utc).isoformat(),
                **fields,
            }
            line = json.dumps(rec, separators=(",", ":"), sort_keys=True)
            self._sink.write(line + "\n")
            self._sink.flush()
            return rec


def _spin_ns(ns: int) -> None:
    if ns <= 0:
        return
    deadline = time.perf_counter_ns() + ns
    while time.perf_counter_ns() < deadline:
        pass


def _worker(
    thread_id: int,
    iterations: int,
    counter: list[int],
    logger: EventLogger,
    spin_ns: int,
    start_barrier: threading.Barrier,
) -> None:
    start_barrier.wait()
    for i in range(iterations):
        before = counter[0]
        logger.emit(
            thread=thread_id,
            op="read",
            iter=i,
            value_before=before,
            value_after=before,
        )
        _spin_ns(spin_ns)
        after = before + 1
        counter[0] = after
        logger.emit(
            thread=thread_id,
            op="write",
            iter=i,
            value_before=before,
            value_after=after,
        )


def run_race(
    *,
    iterations: int,
    spin_ns: int,
    threads: int,
    out_path: Path,
) -> dict:
    counter = [0]
    barrier = threading.Barrier(threads + 1)

    with out_path.open("w", encoding="utf-8", newline="\n") as sink:
        logger = EventLogger(sink)
        workers = [
            threading.Thread(
                target=_worker,
                args=(tid, iterations, counter, logger, spin_ns, barrier),
                name=f"race-t{tid}",
            )
            for tid in range(threads)
        ]
        for t in workers:
            t.start()
        barrier.wait()
        for t in workers:
            t.join()

        expected = iterations * threads
        actual = counter[0]
        summary = {
            "type": "run_summary",
            "seq": logger._seq + 1,
            "ts": datetime.now(timezone.utc).isoformat(),
            "threads": threads,
            "iterations_per_thread": iterations,
            "spin_ns": spin_ns,
            "expected_count": expected,
            "actual_count": actual,
            "lost_updates": expected - actual,
            "race_manifested": actual < expected,
        }
        line = json.dumps(summary, separators=(",", ":"), sort_keys=True)
        sink.write(line + "\n")
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run intentional counter race and emit JSONL events."
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path("kairos/scripts/samples/race_events.jsonl"),
        help="Output JSONL path",
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=2000,
        help="RMW iterations per thread",
    )
    parser.add_argument(
        "--threads",
        type=int,
        default=2,
        help="Worker thread count",
    )
    parser.add_argument(
        "--spin-ns",
        type=int,
        default=800,
        help="Nanoseconds to spin between read and write (widens race window)",
    )
    parser.add_argument(
        "--trials",
        type=int,
        default=1,
        help="Run multiple trials; keep last manifesting race or last run",
    )
    args = parser.parse_args()

    args.out.parent.mkdir(parents=True, exist_ok=True)

    best: dict | None = None
    for trial in range(args.trials):
        trial_path = args.out
        if args.trials > 1:
            trial_path = args.out.with_name(
                f"{args.out.stem}_trial{trial:02d}{args.out.suffix}"
            )
        summary = run_race(
            iterations=args.iterations,
            spin_ns=args.spin_ns,
            threads=args.threads,
            out_path=trial_path,
        )
        summary["trial"] = trial
        summary["log_path"] = str(trial_path)
        print(
            f"trial {trial}: expected={summary['expected_count']} "
            f"actual={summary['actual_count']} "
            f"lost={summary['lost_updates']} "
            f"race={summary['race_manifested']} -> {trial_path}",
            flush=True,
        )
        if summary["race_manifested"]:
            best = summary
            args.out.write_text(trial_path.read_text(encoding="utf-8"), encoding="utf-8")
            break
        best = summary

    if best is None:
        return 1

    if args.trials == 1 or not best.get("race_manifested"):
        if args.trials > 1 and not best.get("race_manifested"):
            last = Path(best["log_path"])
            args.out.write_text(last.read_text(encoding="utf-8"), encoding="utf-8")

    print(
        f"final       : expected={best['expected_count']} "
        f"actual={best['actual_count']} race={best['race_manifested']}",
        flush=True,
    )
    print(f"log         : {args.out}", flush=True)
    return 0 if best["race_manifested"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
