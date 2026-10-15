#!/usr/bin/env python3
"""Host reference for phase_lattice_uart (16-cell phase ring, causal curvature).

Recomputes the CA independently and renders the exact UART text the FPGA must
emit. Modes:
    python verify_lattice.py                 -> print the expected reference text
    python verify_lattice.py <capture_file>  -> verify a UART/sim capture contains
                                                the exact reference block; exit 0
                                                on match, 1 on mismatch.
"""
import sys

# Golden Phase compose table, rows a=0..3 (matches phase_alu.v)
COMPOSE = [
    [0, 1, 2, 3],
    [1, 3, 3, 2],
    [2, 3, 3, 1],
    [3, 2, 1, 0],
]

N = 16
GENS = 40
CLOCK_GENS = 4
INIT = [0, 1, 2, 3, 3, 1, 0, 2, 1, 2, 3, 0, 2, 0, 1, 3]


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


def expected_text():
    lines = ["PHASE LATTICE N=16"]

    # Experiment 1: mixed field, 40 generations of defect (curvature) readout
    x = list(INIT)
    for g in range(GENS):
        nxt, d = step(x)
        lines.append("G%02d S=%s D=%02d" % (g, "".join(map(str, x)), d))
        x = nxt

    # Experiment 2: homogeneous all-Phi1 field -> algebra predicts a 1/2 clock
    x = [1] * N
    clock_ok = True
    for g in range(CLOCK_GENS):
        nxt, d = step(x)
        lines.append("C%02d S=%s D=%02d" % (g, "".join(map(str, x)), d))
        exp = 1 if (g % 2 == 0) else 2
        if any(c != exp for c in x):
            clock_ok = False
        x = nxt

    lines.append("CLOCK PASS" if clock_ok else "CLOCK FAIL")
    lines.append("LATTICE DONE")
    return "\n".join(lines) + "\n"


def verify(path):
    with open(path, "r", errors="replace") as f:
        captured = f.read().replace("\r\n", "\n").replace("\r", "\n")
    expected = expected_text()
    if expected in captured:
        print("MATCH: capture contains exact reference block "
              "(%d generations + %d clock generations)" % (GENS, CLOCK_GENS))
        return 0

    # Diagnostic: line-by-line diff against the first banner occurrence
    start = captured.find("PHASE LATTICE N=16")
    if start < 0:
        print("MISMATCH: banner 'PHASE LATTICE N=16' not found in capture")
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
    if len(sys.argv) > 1:
        sys.exit(verify(sys.argv[1]))
    sys.stdout.write(expected_text())
