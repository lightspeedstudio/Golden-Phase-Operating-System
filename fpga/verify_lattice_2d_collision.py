#!/usr/bin/env python3
"""Host golden + classifier for M=4 collision scenarios."""
import sys

COMPOSE = [
    [0, 1, 2, 3],
    [1, 3, 3, 2],
    [2, 3, 3, 1],
    [3, 2, 1, 0],
]
CLASSES = [
    "PASS_THROUGH",
    "REFLECT",
    "ANNIHILATE",
    "FUSE",
    "REPLICATE",
    "OSCILLATE",
    "SCATTER",
    "DOMAIN_GROWTH",
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


def footprint(G, M, bg=1):
    return sum(1 for i in range(M) for j in range(M) if G[i][j] != bg)


def seed(scenario):
    M = 4
    G = [[1] * M for _ in range(M)]
    if scenario == 0:
        G[0][0] = 2
        G[0][2] = 2
    elif scenario == 1:
        G[0][0] = 2
        G[0][1] = 2
    elif scenario == 2:
        G[2][2] = 2
        G[0][0] = 3
    else:
        G[0][0] = 2
        for j in range(M):
            G[2][j] = 3
    return G


def classify(hist, mb, ma):
    # hist: list of flat strings
    if ma == 0:
        return "ANNIHILATE"
    if ma > mb + 2:
        return "DOMAIN_GROWTH"
    if ma > mb:
        return "REPLICATE"
    if ma < mb and ma >= 1:
        return "FUSE"
    # look for period
    seen = {}
    for i, s in enumerate(hist):
        if s in seen:
            period = i - seen[s]
            if period >= 1 and ma == mb:
                return "PASS_THROUGH" if mb >= 2 else "OSCILLATE"
            if period >= 1:
                return "OSCILLATE"
        seen[s] = i
    return "SCATTER"


def expected_text(scenario=0, gens=16):
    M = 4
    G = seed(scenario)
    mb = footprint(G, M)
    lines = ["COLLISION S%d M4" % scenario]
    hist = []
    for g in range(gens):
        f = footprint(G, M)
        s = flat(G, M)
        hist.append(s)
        lines.append("G%02d S=%s F=%02d" % (g, s, f))
        # FPGA prints G0..G(gens-1) then FINAL without an extra update.
        if g < gens - 1:
            G = rule_a(G, M)
    ma = footprint(G, M)
    cls = classify(hist, mb, ma)
    lines.append("Mb=%02d Ma=%02d" % (mb, ma))
    lines.append("CLASS=%s" % cls)
    lines.append("COLLISION DONE")
    return "\n".join(lines) + "\n", cls, mb, ma


def verify(path, scenario=0, gens=16):
    with open(path, "r", errors="replace") as f:
        cap = f.read().replace("\r\n", "\n").replace("\r", "\n")
    exp, cls, mb, ma = expected_text(scenario, gens)
    # FPGA does not print CLASS; host adds it after matching gens + Mb/Ma
    banner = "COLLISION S%d M4" % scenario
    if banner not in cap:
        print("MISMATCH: banner missing")
        return 1
    for line in exp.splitlines():
        if line.startswith("CLASS="):
            continue
        if line and line not in cap:
            print("MISSING:", line)
            return 1
    print("MATCH: scenario=%d class=%s Mb=%d Ma=%d" % (scenario, cls, mb, ma))
    return 0


def main(argv):
    if len(argv) >= 2 and not argv[1].isdigit():
        sc = int(argv[2]) if len(argv) > 2 else 0
        gens = int(argv[3]) if len(argv) > 3 else 16
        return verify(argv[1], sc, gens)
    sc = int(argv[1]) if len(argv) > 1 else 0
    gens = int(argv[2]) if len(argv) > 2 else 16
    text, cls, _, _ = expected_text(sc, gens)
    sys.stdout.write(text)
    print("# host_class", cls, file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
