#!/usr/bin/env python3
"""Host golden for live UART packet: I/M/A/P (PROP_ARM)."""
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


def footprint(G, M, bg=1):
    return sum(1 for i in range(M) for j in range(M) if G[i][j] != bg)


def verify(path, gens=2, require_prop=True):
    with open(path, "r", errors="replace") as f:
        cap = f.read().replace("\r\n", "\n")
    toks = ["PKT READY", "PKT INJECT", "PKT RECV", "PKT ACK", "PKT DONE"]
    if require_prop:
        toks.insert(-1, "PKT PROP_ARM OK")
    for tok in toks:
        if tok not in cap:
            print("MISSING:", tok)
            return 1
    M = 4
    G = [[1] * M for _ in range(M)]
    G[2][2] = 2
    for g in range(gens):
        f = footprint(G, M)
        s = flat(G, M)
        line = "G%02d S=%s F=%02d" % (g, s, f)
        if line not in cap:
            print("MISSING:", line)
            return 1
        if g < gens - 1:
            G = rule_a(G, M)
    recv = "PKT RECV S=%s" % flat(G, M)
    if recv not in cap:
        print("MISSING:", recv)
        return 1
    label = "I/M/A/P PROP_ARM" if require_prop else "I/M/A"
    print("MATCH: live packet %s gens=%d" % (label, gens))
    return 0


if __name__ == "__main__":
    path = sys.argv[1]
    gens = int(sys.argv[2]) if len(sys.argv) > 2 else 2
    require_prop = True
    if len(sys.argv) > 3 and sys.argv[3] in ("0", "no", "false", "IMA"):
        require_prop = False
    sys.exit(verify(path, gens, require_prop))
