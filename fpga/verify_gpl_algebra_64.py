#!/usr/bin/env python3
"""Verify GPL algebra 64-triple UART capture against host compose table."""
import sys

COMPOSE = [
    [0, 1, 2, 3],
    [1, 3, 3, 2],
    [2, 3, 3, 1],
    [3, 2, 1, 0],
]


def o(a, b):
    return COMPOSE[a][b]


def reflect(x):
    return 3 - x


def expected_lines():
    lines = ["GPL ALGEBRA 64 TRIPLES"]
    for a in range(4):
        for b in range(4):
            for c in range(4):
                ab = o(a, b)
                L = o(ab, c)
                R = o(a, o(b, c))
                if L == R:
                    p = 0
                elif L == reflect(R):
                    p = 1
                else:
                    p = 2
                status = "PASS" if p in (0, 1) else "FAIL"
                lines.append(
                    "T %d%d%d ab=%d L=%d R=%d p=%d %s" % (a, b, c, ab, L, R, p, status)
                )
    lines.append("ALGEBRA 64/64 PASS")
    lines.append("ALGEBRA DONE")
    return "\n".join(lines) + "\n"


def verify(path):
    with open(path, "r", errors="replace") as f:
        cap = f.read().replace("\r\n", "\n").replace("\r", "\n")
    exp = expected_lines()
    if exp in cap:
        print("MATCH: GPL algebra 64/64")
        return 0
    if "ALGEBRA 64/64 PASS" in cap and "ALGEBRA DONE" in cap:
        # Line-by-line check
        bad = 0
        for line in exp.splitlines():
            if line and line not in cap:
                print("MISSING:", line)
                bad += 1
        if bad == 0:
            print("MATCH: GPL algebra 64/64 (unordered containment)")
            return 0
        print("MISMATCH count", bad)
        return 1
    print("MISMATCH: algebra block not found")
    print(cap[:800])
    return 1


def main(argv):
    if len(argv) >= 2:
        return verify(argv[1])
    sys.stdout.write(expected_lines())
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
