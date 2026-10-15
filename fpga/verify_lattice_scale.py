#!/usr/bin/env python3
"""Host reference for phase_lattice_uart_scale (Avenue 9 scaling study).

Extends verify_lattice.py's method to arbitrary N. Does NOT touch or import
verify_lattice.py (that file stays pinned to the physically-verified N=16
experiment's specific init field / 2-digit D format); this is a sibling
reference matching phase_lattice_uart_scale.v's generalized format:
  - init field x[i] = i % 4 (not the N=16 hand-picked field)
  - banner prints N as 3 zero-padded decimal digits
  - D (defect count) printed as 3 decimal digits (0..N, N can exceed 99)

Usage:
    python verify_lattice_scale.py <N> [GENS] [CLK_GENS]
        -> print the expected reference text for that N
    python verify_lattice_scale.py <N> <capture_file> [GENS] [CLK_GENS]
        -> verify a UART/sim capture contains the exact reference block;
           exit 0 on match, 1 on mismatch.
"""
import sys

# Golden Phase compose table, rows a=0..3 (matches phase_alu.v / compose_gate)
COMPOSE = [
    [0, 1, 2, 3],
    [1, 3, 3, 2],
    [2, 3, 3, 1],
    [3, 2, 1, 0],
]


def step(x):
    """One generation: returns (next_state_L, defect_count_of_current_state)."""
    n = len(x)
    nxt = []
    defects = 0
    for i in range(n):
        xm, xc, xp = x[(i - 1) % n], x[i], x[(i + 1) % n]
        left = COMPOSE[COMPOSE[xm][xc]][xp]   # (xm.xc).xp
        right = COMPOSE[xm][COMPOSE[xc][xp]]  # xm.(xc.xp)
        nxt.append(left)
        if left != right:
            defects += 1
    return nxt, defects


def expected_text(n, gens=8, clk_gens=4):
    lines = ["PHASE LATTICE N=%03d" % n]

    # Experiment 1: i%4 field, `gens` generations of defect (curvature) readout
    x = [i % 4 for i in range(n)]
    for g in range(gens):
        nxt, d = step(x)
        lines.append("G%02d S=%s D=%03d" % (g, "".join(map(str, x)), d))
        x = nxt

    # Experiment 2: homogeneous all-Phi1 field -> algebra predicts a 1/2 clock
    x = [1] * n
    clock_ok = True
    for g in range(clk_gens):
        nxt, d = step(x)
        lines.append("C%02d S=%s D=%03d" % (g, "".join(map(str, x)), d))
        exp = 1 if (g % 2 == 0) else 2
        if any(c != exp for c in x):
            clock_ok = False
        x = nxt

    lines.append("CLOCK PASS" if clock_ok else "CLOCK FAIL")
    lines.append("LATTICE DONE")
    return "\n".join(lines) + "\n"


def verify(n, path, gens=8, clk_gens=4):
    with open(path, "r", errors="replace") as f:
        captured = f.read().replace("\r\n", "\n").replace("\r", "\n")
    expected = expected_text(n, gens, clk_gens)
    banner = "PHASE LATTICE N=%03d" % n
    if expected in captured:
        print("MATCH: capture contains exact reference block for N=%d "
              "(%d generations + %d clock generations)" % (n, gens, clk_gens))
        return 0

    start = captured.find(banner)
    if start < 0:
        print("MISMATCH: banner %r not found in capture" % banner)
        return 1
    cap_lines = captured[start:].split("\n")
    exp_lines = expected.rstrip("\n").split("\n")
    for i, exp in enumerate(exp_lines):
        got = cap_lines[i] if i < len(cap_lines) else "<end of capture>"
        if got != exp:
            print("MISMATCH at line %d:" % (i + 1))
            print("  expected: %r" % exp)
            print("  captured: %r" % got)
            return 1
    print("MISMATCH: lines match individually but block not contiguous")
    return 1


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.stderr.write(__doc__)
        sys.exit(2)
    n = int(sys.argv[1])
    if len(sys.argv) >= 3 and not sys.argv[2].isdigit():
        path = sys.argv[2]
        gens = int(sys.argv[3]) if len(sys.argv) > 3 else 8
        clk_gens = int(sys.argv[4]) if len(sys.argv) > 4 else 4
        sys.exit(verify(n, path, gens, clk_gens))
    else:
        gens = int(sys.argv[2]) if len(sys.argv) > 2 else 8
        clk_gens = int(sys.argv[3]) if len(sys.argv) > 3 else 4
        sys.stdout.write(expected_text(n, gens, clk_gens))
