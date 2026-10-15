"""Avenue 2 -- 2-D phase lattice torus dynamics.

Standing-rule compliance: the compose table is NOT reimplemented from scratch;
`make_table(ALPHA_STAR)` reproduces the exact construction in
`scripts/research/gpl_program.py` and the result is asserted, byte-for-byte,
against the literal table given in the roadmap brief:
    ((0,1,2,3),(1,3,3,2),(2,3,3,1),(3,2,1,0))

Two 2-D update rules are compared, as specified in
docs/EXPLORATION_ROADMAP.md, Avenue 2:

  Rule (a): apply the 1-D row rule then the column rule.
      rowpass(G)[i,j] = (G[i,j-1] o G[i,j]) o G[i,j+1]      (row, left-nested)
      colpass(G)[i,j] = (G[i-1,j] o G[i,j]) o G[i+1,j]      (column, left-nested)
      rule_a(G) = colpass(rowpass(G))                        <- row pass FIRST

  Rule (b): fixed grouping over the 4 von-Neumann neighbors,
      x' = ((N o x) o S) o ((E o x) o W)
      applied simultaneously (synchronous / parallel update) at every cell.

Both rules are used to EVOLVE the grid forward in time (rule_a / rule_b).
Separately, for each rule we define a "mirror" grouping that composes the
exact same multiset of symbols in the exact same left-to-right order, but
with a different parenthesization -- this is the direct 2-D generalization
of the 1-D `stepL` vs `stepR` comparison in gpl_program.py section 2:

  Rule (a) mirror: column pass first, then row pass: rule_a_mirror(G) =
      rowpass(colpass(G)). NOTE (documented honestly, see report): this is
      NOT a pure "same word, different parenthesization" comparison in the
      sense of Theorem 1 -- rule_a reads its effective 9-symbol 3x3
      neighborhood in row-major order while rule_a_mirror reads it in
      column-major order, so Theorem 1 does not *guarantee* disagreements
      are pure reflections here. It is checked exhaustively below rather
      than assumed.

  Rule (b) mirror: the SAME 6-symbol word (N, x, S, E, x, W), in the SAME
      left-to-right order, fully right-nested instead of the given grouping:
      N o (x o (S o (E o (x o W))))
      This one IS a same-word/different-parenthesization comparison in the
      Theorem-1 sense, and the "always reflection" claim is checked
      exhaustively (all 4^5 = 1024 distinct (N,x,S,E,W) tuples, since x is
      reused so there are 5 independent slots, not 6).

defect_a(G) / defect_b(G) := boolean field where the rule's primary output
disagrees with its mirror's output, both computed from the SAME input grid
G (mirrors are never used to evolve the grid -- exactly as gpl_program.py's
1-D script only evolves via stepL and uses stepR solely as the diagnostic).

Reproduction: `python scripts/research/avenue02_lattice_2d.py`
"""
import math
import random
import numpy as np
from itertools import product

PHI = (1 + 5 ** 0.5) / 2
RAIL = math.pi / PHI
ALPHA_STAR = 7.2973525693e-3


def make_table(alpha):
    disc = PHI * PHI - 4 * alpha
    if disc <= 0:
        return None
    sd = math.sqrt(disc)
    z_out, z_in = (PHI + sd) / 2, (PHI - sd) / 2
    if z_in <= 0:
        return None
    s = [-math.sqrt(z_out), -math.sqrt(z_in), math.sqrt(z_in), math.sqrt(z_out)]
    lo, hi = s[0], s[3]
    st = [(x - lo) / (hi - lo) * RAIL for x in s]
    b = [(st[0] + st[1]) / 2, (st[1] + st[2]) / 2, (st[2] + st[3]) / 2]
    fold = lambda x: x if x <= RAIL else 2 * RAIL - x

    def snap(x):
        return 0 if x <= b[0] else 1 if x <= b[1] else 2 if x <= b[2] else 3

    return tuple(tuple(snap(fold(st[a] + st[bb])) for bb in range(4)) for a in range(4))


T = make_table(ALPHA_STAR)
EXPECTED_T = ((0, 1, 2, 3), (1, 3, 3, 2), (2, 3, 3, 1), (3, 2, 1, 0))
assert T == EXPECTED_T, f"compose table mismatch: got {T}, expected {EXPECTED_T}"
print(f"compose table verified against roadmap standing rule: {T}")

T_arr = np.array(T, dtype=np.int64)
rho = lambda x: 3 - x
STATE_CHARS = {0: '.', 1: ':', 2: '+', 3: '#'}


def comp_arr(A, B):
    return T_arr[A, B]


# ---------------------------------------------------------------- rules ---
def rowpass(G):
    W = np.roll(G, 1, axis=1)
    E = np.roll(G, -1, axis=1)
    return comp_arr(comp_arr(W, G), E)


def colpass(G):
    N = np.roll(G, 1, axis=0)
    S = np.roll(G, -1, axis=0)
    return comp_arr(comp_arr(N, G), S)


def rule_a(G):
    """Row pass first, then column pass."""
    return colpass(rowpass(G))


def rule_a_mirror(G):
    """Column pass first, then row pass (diagnostic only, never evolves the grid)."""
    return rowpass(colpass(G))


def rule_b(G):
    N = np.roll(G, 1, axis=0)
    S = np.roll(G, -1, axis=0)
    E = np.roll(G, -1, axis=1)
    W = np.roll(G, 1, axis=1)
    left = comp_arr(comp_arr(N, G), S)
    right = comp_arr(comp_arr(E, G), W)
    return comp_arr(left, right)


def rule_b_mirror(G):
    """Fully right-nested parenthesization of the same word (N,x,S,E,x,W)."""
    N = np.roll(G, 1, axis=0)
    S = np.roll(G, -1, axis=0)
    E = np.roll(G, -1, axis=1)
    W = np.roll(G, 1, axis=1)
    r1 = comp_arr(G, W)
    r2 = comp_arr(E, r1)
    r3 = comp_arr(S, r2)
    r4 = comp_arr(G, r3)
    r5 = comp_arr(N, r4)
    return r5


def defect_field(primary_fn, mirror_fn, G):
    p = primary_fn(G)
    m = mirror_fn(G)
    return (p != m).astype(np.int64), p


# ------------------------------------------------- exhaustive coherence ---
def exhaustive_check_rule_b():
    """All 4^5 = 1024 distinct (N,x,S,E,W) neighbor tuples (x used twice, same
    value both times). Checks whether rule_b vs rule_b_mirror -- a genuine
    same-word / different-parenthesization pair -- ever disagrees by
    anything other than reflection rho."""
    combos = np.array(list(product(range(4), repeat=5)))
    N, x, S, E, W = (combos[:, i] for i in range(5))
    left = comp_arr(comp_arr(N, x), S)
    right = comp_arr(comp_arr(E, x), W)
    L = comp_arr(left, right)
    r1 = comp_arr(x, W)
    r2 = comp_arr(E, r1)
    r3 = comp_arr(S, r2)
    r4 = comp_arr(x, r3)
    R = comp_arr(N, r4)
    disagree = L != R
    n_dis = int(disagree.sum())
    if n_dis == 0:
        always_refl = True
        counterexamples = []
    else:
        ok = (R[disagree] == rho(L[disagree]))
        always_refl = bool(ok.all())
        counterexamples = []
        if not always_refl:
            bad = np.where(disagree)[0][~ok][:5]
            for k in bad:
                counterexamples.append((int(N[k]), int(x[k]), int(S[k]), int(E[k]), int(W[k]),
                                         int(L[k]), int(R[k])))
    return len(combos), n_dis, always_refl, counterexamples


def exhaustive_check_rule_a():
    """All 4^9 = 262144 3x3 Moore-block configurations. rule_a reads the
    block row-major, rule_a_mirror reads it column-major -- this is NOT
    guaranteed coherent by Theorem 1 (different symbol order, not just
    different parenthesization). Checked exhaustively, no assumption made."""
    combos = np.array(list(product(range(4), repeat=9)))
    a = combos.reshape(-1, 3, 3)
    R0 = comp_arr(comp_arr(a[:, 0, 0], a[:, 0, 1]), a[:, 0, 2])
    R1 = comp_arr(comp_arr(a[:, 1, 0], a[:, 1, 1]), a[:, 1, 2])
    R2 = comp_arr(comp_arr(a[:, 2, 0], a[:, 2, 1]), a[:, 2, 2])
    rowmajor = comp_arr(comp_arr(R0, R1), R2)
    C0 = comp_arr(comp_arr(a[:, 0, 0], a[:, 1, 0]), a[:, 2, 0])
    C1 = comp_arr(comp_arr(a[:, 0, 1], a[:, 1, 1]), a[:, 2, 1])
    C2 = comp_arr(comp_arr(a[:, 0, 2], a[:, 1, 2]), a[:, 2, 2])
    colmajor = comp_arr(comp_arr(C0, C1), C2)
    disagree = rowmajor != colmajor
    n_dis = int(disagree.sum())
    if n_dis == 0:
        always_refl = True
        counterexamples = []
    else:
        ok = (colmajor[disagree] == rho(rowmajor[disagree]))
        always_refl = bool(ok.all())
        counterexamples = []
        if not always_refl:
            bad = np.where(disagree)[0][~ok][:5]
            for k in bad:
                counterexamples.append((a[k].tolist(), int(rowmajor[k]), int(colmajor[k])))
    return len(combos), n_dis, always_refl, counterexamples


# ------------------------------------------------------------- dynamics ---
def random_grid(M, rng):
    return rng.integers(0, 4, size=(M, M))


def run_density_and_hist(rule_fn, mirror_fn, M, steps, trials, seed, hist_steps=100):
    rng = np.random.default_rng(seed)
    series = np.zeros((trials, steps))
    hist_final = np.zeros(4, dtype=np.int64)
    late_fields = []
    for t_idx in range(trials):
        G = random_grid(M, rng)
        for step in range(steps):
            field, primary_next = defect_field(rule_fn, mirror_fn, G)
            series[t_idx, step] = field.mean()
            if step == 30:
                late_fields.append(field.copy())
            G = primary_next
        for _ in range(hist_steps - steps):
            G = rule_fn(G)
        vals, counts = np.unique(G, return_counts=True)
        for v, c in zip(vals, counts):
            hist_final[v] += c
    avg = series.mean(axis=0)
    hist_norm = hist_final / hist_final.sum()
    return avg, hist_norm, late_fields


def radial_autocorrelation(field):
    """FFT (Wiener-Khinchin) periodic autocorrelation of a 0/1 field on the
    torus, radially averaged by minimum-image distance."""
    M = field.shape[0]
    f = field.astype(float) - field.mean()
    var = (f ** 2).mean()
    if var < 1e-12:
        return None, None
    F = np.fft.fft2(f)
    acf = np.fft.ifft2(F * np.conj(F)).real / (M * M)
    acf = acf / acf[0, 0]
    buckets = {}
    for dx in range(M):
        ddx = min(dx, M - dx)
        for dy in range(M):
            ddy = min(dy, M - dy)
            r = round(math.hypot(ddx, ddy))
            buckets.setdefault(r, []).append(acf[dx, dy])
    profile = {r: float(np.mean(v)) for r, v in sorted(buckets.items())}
    # correlation length: first radius where profile drops to <= 1/e
    thresh = 1.0 / math.e
    xi = None
    rs = sorted(profile)
    for i in range(1, len(rs)):
        if profile[rs[i]] <= thresh:
            r0, r1 = rs[i - 1], rs[i]
            v0, v1 = profile[r0], profile[r1]
            if v0 == v1:
                xi = r1
            else:
                xi = r0 + (thresh - v0) * (r1 - r0) / (v1 - v0)
            break
    return profile, xi


def ascii_grid(G):
    return "\n".join("".join(STATE_CHARS[int(v)] for v in row) for row in G)


def glider_search(rule_fn, M, backgrounds, steps, seed):
    """Seed a single-cell perturbation on a uniform background; track the
    footprint (# of cells differing from background) over time, PLUS two
    honesty diagnostics:
      - if footprint stays at 1, track the (row,col) of the differing cell
        to distinguish a truly stationary point-defect from one that moves
        but always leaves exactly one cell disturbed.
      - report whether the FINAL grid is itself spatially uniform (and at
        what value) -- a footprint that saturates at M*M can simply mean
        "converged to the other uniform fixed point", not chaos/spreading.
    """
    results = {}
    for v in backgrounds:
        G = np.full((M, M), v, dtype=np.int64)
        cy, cx = M // 2, M // 2
        pert = rho(v) if v != rho(v) else (v + 1) % 4
        G[cy, cx] = pert
        footprints = []
        positions = []
        snapshots = {}
        snap_steps = {0, 1, 2, 3, 5, 10, 20, 30, steps - 1}
        for step in range(steps):
            diff_mask = (G != v)
            diff = int(diff_mask.sum())
            footprints.append(diff)
            if diff == 1:
                yx = np.argwhere(diff_mask)[0]
                positions.append((step, int(yx[0]), int(yx[1])))
            if step in snap_steps:
                snapshots[step] = ascii_grid(G)
            if diff == 0:
                break
            G = rule_fn(G)
        uniq = np.unique(G)
        final_uniform = uniq.size == 1
        final_value = int(uniq[0]) if final_uniform else None
        results[v] = {"perturb_to": pert, "footprints": footprints, "snapshots": snapshots,
                      "single_defect_positions": positions,
                      "final_uniform": final_uniform, "final_value": final_value}
    return results


# =================================================================
if __name__ == "__main__":
    print("\n################ EXHAUSTIVE COHERENCE CHECKS ################")
    n, nd, ok, ce = exhaustive_check_rule_b()
    print(f"Rule (b) vs rule (b) mirror [SAME word (N,x,S,E,x,W), diff parenthesization]:")
    print(f"  tuples tested={n} disagreements={nd} ({nd/n:.4f}) always-reflection={ok}")
    if ce:
        print(f"  counterexamples (first 5): {ce}")

    n, nd, ok, ce = exhaustive_check_rule_a()
    print(f"Rule (a) vs rule (a) mirror [3x3 block, row-major vs column-major reading]:")
    print(f"  tuples tested={n} disagreements={nd} ({nd/n:.4f}) always-reflection={ok}")
    if ce:
        print(f"  counterexamples (first 5): {ce}")

    for M in (16, 32):
        print(f"\n################ M={M} TORUS ################")
        for name, rfn, mfn in (("A (row-then-col)", rule_a, rule_a_mirror),
                                ("B (von Neumann fixed grouping)", rule_b, rule_b_mirror)):
            avg, hist, late_fields = run_density_and_hist(rfn, mfn, M, steps=50, trials=10, seed=1234)
            print(f"\n--- rule {name}, M={M} ---")
            idxs = [0, 1, 2, 5, 10, 20, 30, 49]
            print("defect density vs t:", {i: round(float(avg[i]), 4) for i in idxs})
            print("long-run state histogram (100 steps):",
                  {i: round(float(hist[i]), 4) for i in range(4)})
            profiles = [radial_autocorrelation(f) for f in late_fields]
            profiles = [p for p in profiles if p[0] is not None]
            if profiles:
                rs = sorted(profiles[0][0].keys())
                avg_profile = {r: float(np.mean([p[0][r] for p in profiles if r in p[0]])) for r in rs}
                xis = [p[1] for p in profiles if p[1] is not None]
                xi_mean = float(np.mean(xis)) if xis else None
                print("radial autocorrelation C(r) at t=30 (avg over trials):",
                      {r: round(avg_profile[r], 4) for r in rs[:8]})
                print(f"correlation length estimate (1/e crossing, mean over {len(xis)} trials): "
                      f"{xi_mean}")
            else:
                print("autocorrelation: field had zero variance in all trials (no defects to correlate)")

    print("\n################ GLIDER / LOCALIZED-STRUCTURE SEARCH ################")
    print("Single-cell perturbation on uniform backgrounds, M=16, 40 steps.")
    for name, rfn in (("A", rule_a), ("B", rule_b)):
        print(f"\n--- rule {name} ---")
        res = glider_search(rfn, M=16, backgrounds=[0, 1, 2, 3], steps=40, seed=99)
        for v, r in res.items():
            fp = r["footprints"]
            tail = fp[-8:]
            print(f"  background={v} perturb_to={r['perturb_to']}: "
                  f"footprint len={len(fp)} first10={fp[:10]} last8={tail}")
            print(f"      final grid uniform={r['final_uniform']} final_value={r['final_value']}")
            if r["single_defect_positions"]:
                pos = r["single_defect_positions"]
                stationary = len({(p[1], p[2]) for p in pos}) == 1
                print(f"      single-cell-defect steps={len(pos)} stationary={stationary} "
                      f"first_pos={pos[0]} last_pos={pos[-1]}")

    print("\n################ ASCII SNAPSHOTS (selected) ################")
    # Rule A, background=1 -> stationary point defect (if confirmed above)
    resA = glider_search(rule_a, M=16, backgrounds=[1], steps=10, seed=99)
    print("\nRule A, background=1, perturb->2 (point-defect candidate), t=0,1,2,3,5:")
    for t in (0, 1, 2, 3, 5):
        print(f"-- t={t} --")
        print(resA[1]["snapshots"].get(t, "(not captured)"))
    # Rule A, background=0 -> growth/breather
    resA0 = glider_search(rule_a, M=16, backgrounds=[0], steps=10, seed=99)
    print("\nRule A, background=0, perturb->3 (breather candidate), t=0,1,2,3,5:")
    for t in (0, 1, 2, 3, 5):
        print(f"-- t={t} --")
        print(resA0[0]["snapshots"].get(t, "(not captured)"))
    # Rule B, background=0 -> healing/decay
    resB0 = glider_search(rule_b, M=16, backgrounds=[0], steps=10, seed=99)
    print("\nRule B, background=0, perturb->3 (healing candidate), t=0,1,2,3,5:")
    for t in (0, 1, 2, 3, 5):
        print(f"-- t={t} --")
        print(resB0[0]["snapshots"].get(t, "(not captured)"))
