#!/usr/bin/env python3
"""Host golden for Avenue 2 rule_a glider (parameterized Phase 1).

Usage:
  python verify_lattice_2d_glider.py [M] [GENS] [VACUUM] [DEFECT] [SR] [SC]
  python verify_lattice_2d_glider.py <capture> [M] [GENS] [VACUUM] [DEFECT] [SR] [SC]
"""
import sys

COMPOSE = [
    [0, 1, 2, 3],
    [1, 3, 3, 2],
    [2, 3, 3, 1],
    [3, 2, 1, 0],
]


def o(a, b):
    return COMPOSE[a][b]


def rowpass(G, M):
    out = [[0] * M for _ in range(M)]
    for i in range(M):
        for j in range(M):
            L, x, R = G[i][(j - 1) % M], G[i][j], G[i][(j + 1) % M]
            out[i][j] = o(o(L, x), R)
    return out


def colpass(G, M):
    out = [[0] * M for _ in range(M)]
    for i in range(M):
        for j in range(M):
            N, x, S = G[(i - 1) % M][j], G[i][j], G[(i + 1) % M][j]
            out[i][j] = o(o(N, x), S)
    return out


def rule_a(G, M):
    return colpass(rowpass(G, M), M)


def flat(G, M):
    return "".join(str(G[i][j]) for i in range(M) for j in range(M))


def footprint(G, M, bg):
    return sum(1 for i in range(M) for j in range(M) if G[i][j] != bg)


def expected_text(M=4, GENS=8, VACUUM=1, DEFECT=2, SR=None, SC=None):
    if SR is None:
        SR = M // 2
    if SC is None:
        SC = M // 2
    G = [[VACUUM] * M for _ in range(M)]
    G[SR][SC] = DEFECT
    lines = [
        "PHASE LATTICE 2D GLIDER %02dx%02d V%dD%d S%d,%d"
        % (M, M, VACUUM, DEFECT, SR, SC)
    ]
    track_ok = True
    mismatches = 0
    for g in range(GENS):
        f = footprint(G, M, VACUUM)
        er, ec = (SR - g) % M, (SC - g) % M
        if f != 1 or G[er][ec] != DEFECT:
            track_ok = False
            mismatches += 1
        lines.append("G%05d S=%s F=%02d" % (g, flat(G, M), f))
        G = rule_a(G, M)
    lines.append("GLIDER PASS" if track_ok else "GLIDER FAIL")
    lines.append("LATTICE DONE")
    return "\n".join(lines) + "\n", mismatches, track_ok


def verify(path, M=4, GENS=8, VACUUM=1, DEFECT=2, SR=None, SC=None):
    with open(path, "r", errors="replace") as f:
        captured = f.read().replace("\r\n", "\n").replace("\r", "\n")
    expected, mismatches, ok = expected_text(M, GENS, VACUUM, DEFECT, SR, SC)
    if expected in captured:
        print(
            "MATCH: glider M=%d GENS=%d V=%d D=%d S=%s,%s mismatches=0"
            % (M, GENS, VACUUM, DEFECT, SR if SR is not None else M // 2, SC if SC is not None else M // 2)
        )
        return 0
    banner = expected.split("\n", 1)[0]
    start = captured.find(banner)
    if start < 0:
        print("MISMATCH: banner %r not found" % banner)
        return 1
    print("MISMATCH: capture does not contain exact golden block (host mismatches would be %d)" % mismatches)
    print("--- expected (first 500) ---")
    print(expected[:500])
    print("--- captured (from banner, first 500) ---")
    print(captured[start : start + 500])
    return 1


def main(argv):
    def parse_args(start):
        M = int(argv[start]) if len(argv) > start else 4
        GENS = int(argv[start + 1]) if len(argv) > start + 1 else (2 * M)
        VACUUM = int(argv[start + 2]) if len(argv) > start + 2 else 1
        DEFECT = int(argv[start + 3]) if len(argv) > start + 3 else 2
        SR = int(argv[start + 4]) if len(argv) > start + 4 else M // 2
        SC = int(argv[start + 5]) if len(argv) > start + 5 else M // 2
        return M, GENS, VACUUM, DEFECT, SR, SC

    if len(argv) >= 2 and not argv[1].isdigit():
        path = argv[1]
        return verify(path, *parse_args(2))
    text, _, _ = expected_text(*parse_args(1))
    sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
