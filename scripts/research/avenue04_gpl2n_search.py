"""AVENUE 4: Wider GPL(2n) search — coherent 6- or 8-state algebra, or a
strengthened negative result.

Generalizes make6() from gpl_program.py §3. Construction: kernel roots come
in +/- pairs; for 2n states we take n free positive "root-squares" z_1..z_n,
r_i = sqrt(z_i), the 2n root positions s = (-r_n,...,-r_1, r_1,...,r_n) are
mapped affinely onto the rail [0, RAIL] with midpoint snap boundaries between
adjacent positions, and

    a . b = snap(fold(s_a + s_b)),   fold(x) = x if x <= RAIL else 2*RAIL-x.

(a) GPL(6), n=3: DROP the golden-trace constraint z1+z2+z3=phi used in the
    original gpl_program.py probe. 3 free positive root-squares, random
    sampling >= 1,000,000 points.
(b) GPL(8), n=4: 4 free positive root-squares, random sampling >= 1,000,000
    points.

Note on the sampling family: the construction is invariant under a common
positive rescaling of the z_i (the affine normalization to [0, RAIL] divides
out both any additive shift and any overall scale of the root positions).
So the table depends only on the *ratios* z_1 : z_2 : ... : z_n, i.e. a
point on the (n-1)-simplex, not on the n raw magnitudes. Drawing z_i i.i.d.
Uniform(0,1) and sorting is a standard, if not perfectly uniform-on-simplex,
way to cover that ratio space broadly; we use it because it matches the
"random sampling" alternative explicitly offered by the roadmap brief.

For every candidate table we check, purely computationally (no assumptions):
  1. identity        : t[0][x] == x for all x
  2. reflection row   : row (2n-1) is the exact reverse (2n-1, 2n-2, ..., 0)
  3. orbit-coherence  : for all a,b, {a.b, a.rho(b), rho(a).b, rho(a).rho(b)}
                        lies inside one reflection orbit {v, 2n-1-v}
  4. quotient-group   : the orbit quotient (n elements, built from the
                        canonical low-half representatives 0..n-1, orb(x) =
                        x if x<n else 2n-1-x) is associative AND has
                        identity/inverses, i.e. is a genuine group. For n=3
                        the only group of order 3 (up to iso) is Z3; for n=4
                        the only groups of order 4 (up to iso) are Z4 and
                        Z2xZ2 -- so "is a group" and "matches a named target"
                        are mathematically equivalent for these n, but we
                        verify the explicit isomorphism anyway rather than
                        assume it.

A "near-miss" is a candidate that fails EXACTLY ONE of the four checks
above. We log the best (rarest-failure-class) near-misses with their z
parameters for the report.

Standing rule grounding: this script does not touch the primary 4-state GPL
compose at all (it explores a *different*, larger family of algebras), but
per the "never reimplement compose" rule we still import/replicate and
assert the exact primary table as a grounding sanity check, exactly as
avenue01_attractor_census.py does.

Reproduction:
    python scripts/research/avenue04_gpl2n_search.py
"""
import math
import random
import time
from itertools import product, permutations
from collections import Counter

PHI = (1 + 5**0.5) / 2
RAIL = math.pi / PHI
ALPHA_STAR = 7.2973525693e-3


def make_table(alpha):
    """Replicate the primary GPL(4) table from gpl_program.py (grounding check)."""
    disc = PHI*PHI - 4*alpha
    if disc <= 0: return None
    sd = math.sqrt(disc)
    z_out, z_in = (PHI+sd)/2, (PHI-sd)/2
    if z_in <= 0: return None
    s = [-math.sqrt(z_out), -math.sqrt(z_in), math.sqrt(z_in), math.sqrt(z_out)]
    lo, hi = s[0], s[3]
    st = [(x-lo)/(hi-lo)*RAIL for x in s]
    b = [(st[0]+st[1])/2, (st[1]+st[2])/2, (st[2]+st[3])/2]
    fold = lambda x: x if x <= RAIL else 2*RAIL - x
    def snap(x):
        return 0 if x <= b[0] else 1 if x <= b[1] else 2 if x <= b[2] else 3
    return tuple(tuple(snap(fold(st[a]+st[bb])) for bb in range(4)) for a in range(4))

EXACT_TABLE = ((0,1,2,3),(1,3,3,2),(2,3,3,1),(3,2,1,0))
assert make_table(ALPHA_STAR) == EXACT_TABLE, "primary GPL(4) table mismatch — grounding check failed"


# ---------------- generalized GPL(2n) construction ----------------

def make2n(zs):
    """zs: length-n list of positive floats (free root-squares).
    Returns the 2n x 2n compose table, or None if degenerate (non-positive
    or numerically tied root magnitudes -> ill-defined boundaries)."""
    n = len(zs)
    zs_sorted = sorted(zs)
    if zs_sorted[0] <= 1e-15:
        return None
    for i in range(n - 1):
        if zs_sorted[i+1] - zs_sorted[i] < 1e-12:
            return None
    r = [math.sqrt(z) for z in zs_sorted]
    s = [-x for x in reversed(r)] + r          # length 2n, strictly ascending
    lo, hi = s[0], s[-1]
    if hi - lo < 1e-12:
        return None
    m = 2 * n
    st = [(x - lo) / (hi - lo) * RAIL for x in s]
    b = [(st[i] + st[i+1]) / 2 for i in range(m - 1)]
    def fold(x):
        return x if x <= RAIL else 2*RAIL - x
    def snap(x):
        for i in range(m - 1):
            if x <= b[i]:
                return i
        return m - 1
    return tuple(tuple(snap(fold(st[a] + st[bb])) for bb in range(m)) for a in range(m))


def identify_group(qop_rows, n):
    """qop_rows: n x n table (list of lists / tuple of tuples) of an
    associative magma with identity 0 on {0..n-1}. Return the list of named
    targets it is isomorphic to (permutation search fixing 0, exhaustive
    since n <= 4)."""
    elems = list(range(1, n))
    targets = {}
    if n == 3:
        targets['Z3'] = lambda i, j: (i + j) % 3
    elif n == 4:
        targets['Z4'] = lambda i, j: (i + j) % 4
        targets['Z2xZ2'] = lambda i, j: i ^ j
    matches = []
    for name, target in targets.items():
        for perm in permutations(elems):
            mapping = {0: 0}
            for k, e in enumerate(elems):
                mapping[e] = perm[k]
            ok = True
            for i in range(n):
                for j in range(n):
                    if mapping[qop_rows[i][j]] != target(mapping[i], mapping[j]):
                        ok = False
                        break
                if not ok:
                    break
            if ok:
                matches.append(name)
                break
    return matches


def check_candidate(t, n):
    """Compute the four properties + full associator topology for a 2n-state
    candidate table t. Returns a dict."""
    m = 2 * n
    rho = lambda x: m - 1 - x

    ident = all(t[0][x] == x for x in range(m))
    refl_row = tuple(t[m-1][x] for x in range(m)) == tuple(range(m-1, -1, -1))

    coh = True
    for a in range(m):
        for b in range(m):
            vals = {t[a][b], t[a][rho(b)], t[rho(a)][b], t[rho(a)][rho(b)]}
            if not any(vals <= {v, m-1-v} for v in vals):
                coh = False
                break
        if not coh:
            break

    # quotient built from canonical low-half representatives 0..n-1
    orb = lambda x: x if x < n else m - 1 - x
    qop = tuple(tuple(orb(t[i][j]) for j in range(n)) for i in range(n))

    assoc = True
    for i, j, k in product(range(n), repeat=3):
        if qop[qop[i][j]][k] != qop[i][qop[j][k]]:
            assoc = False
            break
    quot_ident = all(qop[0][i] == i and qop[i][0] == i for i in range(n))
    has_inv = quot_ident and all(any(qop[i][j] == 0 for j in range(n)) for i in range(n))
    is_group = assoc and quot_ident and has_inv
    group_names = identify_group(qop, n) if is_group else []

    # associator topology over the full 2n^3 triples (same/reflection/other)
    same = refl_t = other = 0
    for a, b, c in product(range(m), repeat=3):
        L = t[t[a][b]][c]
        R = t[a][t[b][c]]
        if L == R:
            same += 1
        elif R == rho(L):
            refl_t += 1
        else:
            other += 1

    return dict(ident=ident, refl=refl_row, coh=coh, assoc=assoc,
                is_group=is_group, group_names=group_names,
                same=same, reflT=refl_t, other=other)


def run_search(n, n_samples, seed, z_range=(0.0, 1.0)):
    """Random search over n free positive root-squares for the 2n-state family."""
    rng = random.Random(seed)
    m = 2 * n
    lo_z, hi_z = z_range

    counts = Counter()          # (ident, refl, coh, is_group) -> count
    group_hits = Counter()      # group name -> count (only when is_group)
    degenerate = 0
    tested = 0
    near_misses = []            # list of dicts, one per failing-exactly-one candidate
    fully_coherent = []         # coherent AND is_group

    t0 = time.time()
    for _ in range(n_samples):
        zs = [rng.uniform(lo_z, hi_z) for _ in range(n)]
        t = make2n(zs)
        if t is None:
            degenerate += 1
            continue
        tested += 1
        res = check_candidate(t, n)
        key = (res['ident'], res['refl'], res['coh'], res['is_group'])
        counts[key] += 1
        if res['is_group']:
            for gname in res['group_names']:
                group_hits[gname] += 1
        n_fail = sum(1 for v in key if not v)
        if n_fail == 0:
            fully_coherent.append((sorted(zs), res))
        elif n_fail == 1:
            failed = [name for name, ok in zip(('ident', 'refl', 'coh', 'is_group'), key) if not ok][0]
            if len(near_misses) < 5000:  # cap memory; we only keep/report a handful anyway
                near_misses.append(dict(z=sorted(zs), failed=failed, res=res))
    elapsed = time.time() - t0

    return dict(n=n, m=m, n_samples=n_samples, tested=tested, degenerate=degenerate,
                counts=counts, group_hits=group_hits, near_misses=near_misses,
                fully_coherent=fully_coherent, elapsed=elapsed, seed=seed, z_range=z_range)


def summarize(result):
    n, m = result['n'], result['m']
    print(f"\n================ GPL({m})  (n={n} free root-squares) ================")
    print(f"seed={result['seed']}  z_i ~ Uniform{result['z_range']} iid, n={n} per draw")
    print(f"requested samples={result['n_samples']}  tested(non-degenerate)={result['tested']}  "
          f"degenerate/rejected={result['degenerate']}  elapsed={result['elapsed']:.1f}s")
    print(f"\nproperty combination counts (ident, reflection-row, orbit-coherence, quotient-is-group):")
    for key, c in sorted(result['counts'].items(), key=lambda kv: -kv[1]):
        print(f"  {key}: {c}")
    print(f"\nquotient group-identity hits (only counted when quotient IS a group): {dict(result['group_hits'])}")
    print(f"fully coherent AND quotient-is-group candidates found: {len(result['fully_coherent'])}")
    for zs, res in result['fully_coherent'][:10]:
        print(f"  z={['%.6f'%z for z in zs]}  groups={res['group_names']}  "
              f"topology(same/refl/other)={res['same']}/{res['reflT']}/{res['other']}")
    print(f"\nnear-misses (fail exactly one of the four checks): {len(result['near_misses'])}")
    by_failed = Counter(nm['failed'] for nm in result['near_misses'])
    print(f"  breakdown by which check failed: {dict(by_failed)}")
    # best near-misses: prioritize coh-failures and is_group-failures (the interesting ones),
    # since ident/refl failures are essentially always numerical/degenerate artifacts.
    interesting = [nm for nm in result['near_misses'] if nm['failed'] in ('coh', 'is_group')]
    print(f"  best near-misses (failed in {{coh, is_group}} only, showing up to 8):")
    for nm in interesting[:8]:
        z = nm['z']; res = nm['res']
        print(f"    failed={nm['failed']:<9} z={['%.6f'%v for v in z]} "
              f"ident={res['ident']} refl={res['refl']} coh={res['coh']} is_group={res['is_group']} "
              f"groups={res['group_names']} topology={res['same']}/{res['reflT']}/{res['other']}")
    return result


if __name__ == '__main__':
    print("Avenue 4 — wider GPL(2n) search (random sampling family)")
    print(f"PHI={PHI:.12f}  RAIL={RAIL:.12f}  grounding: primary GPL(4) table OK")

    res6 = summarize(run_search(n=3, n_samples=1_000_000, seed=406, z_range=(0.0, 1.0)))
    res8 = summarize(run_search(n=4, n_samples=1_000_000, seed=408, z_range=(0.0, 1.0)))

    print("\n================ VERDICT ================")
    for res in (res6, res8):
        m = res['m']
        n_coh_group = len(res['fully_coherent'])
        verdict = "COHERENT GPL FOUND" if n_coh_group > 0 else "NOT found in this family/sample"
        print(f"GPL({m}): {verdict}  "
              f"(tested={res['tested']}, coherent+group={n_coh_group}, "
              f"near-misses={len(res['near_misses'])})")
