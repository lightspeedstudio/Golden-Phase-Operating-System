"""Two new computations on the Golden Phase algebra.

1. COHERENCE: the Associator-Reflection Theorem says any TWO parenthesizations
   of a 3-letter word differ by at most the reflection rho(x)=3-x.
   Question: does that stay true for ALL parenthesizations of words of
   length 4, 5, 6, 7?  (i.e. do all evaluation orders of any word land in a
   single reflection orbit {v, 3-v}?)  If yes, that is a NEW theorem.

2. ALPHA ROBUSTNESS: the compose table is built from alpha = 7.2973525693e-3.
   Question: for which interval of alpha does the SAME 4x4 table emerge?
   This answers honestly whether the fine-structure value is special or
   whether any small alpha gives the same algebra (and finds the exact
   bifurcation points where the table would change).
"""
import math
from itertools import product
from functools import lru_cache

PHI = (1 + 5**0.5) / 2
RAIL = math.pi / PHI
ALPHA_STAR = 7.2973525693e-3

def make_table(alpha):
    disc = PHI*PHI - 4*alpha
    if disc < 0:
        return None
    sd = math.sqrt(disc)
    z_out, z_in = (PHI + sd)/2, (PHI - sd)/2
    if z_in < 0:
        return None
    s = [-math.sqrt(z_out), -math.sqrt(z_in), math.sqrt(z_in), math.sqrt(z_out)]
    lo, hi = s[0], s[3]
    st = [(x - lo)/(hi - lo)*RAIL for x in s]
    b = [(st[0]+st[1])/2, (st[1]+st[2])/2, (st[2]+st[3])/2]
    def fold(x):
        return x if x <= RAIL else 2*RAIL - x
    def snap(x):
        if x <= b[0]: return 0
        if x <= b[1]: return 1
        if x <= b[2]: return 2
        return 3
    return tuple(tuple(snap(fold(st[a]+st[bb])) for bb in range(4)) for a in range(4))

T = make_table(ALPHA_STAR)
print("Canonical table:", T)

def compose(a, b):
    return T[a][b]

# ---------- 1. coherence over all parenthesizations ----------
@lru_cache(maxsize=None)
def all_evals(word):
    """Set of values reachable by every parenthesization of word (tuple)."""
    if len(word) == 1:
        return frozenset(word)
    out = set()
    for i in range(1, len(word)):
        for l in all_evals(word[:i]):
            for r in all_evals(word[i:]):
                out.add(compose(l, r))
    return frozenset(out)

print("\n--- Coherence check (all parenthesizations in one rho-orbit?) ---")
for n in range(3, 8):
    bad = []
    orbit_stats = {1: 0, 2: 0}
    for w in product(range(4), repeat=n):
        vals = all_evals(w)
        # is vals inside a single reflection orbit {v, 3-v}?
        ok = any(vals <= frozenset({v, 3-v}) for v in vals)
        if ok:
            orbit_stats[len(vals)] += 1
        else:
            bad.append((w, sorted(vals)))
    total = 4**n
    print(f"len {n}: {total} words | single-value {orbit_stats[1]} | "
          f"rho-orbit-pair {orbit_stats[2]} | VIOLATIONS {len(bad)}")
    for w, v in bad[:5]:
        print("   violation:", w, "->", v)

# ---------- 2. alpha robustness / bifurcation points ----------
print("\n--- Alpha robustness ---")
alpha_max = PHI*PHI/4  # discriminant zero

def same_table(alpha):
    t = make_table(alpha)
    return t == T

# scan then bisect the upper edge of the region containing ALPHA_STAR
lo, hi = ALPHA_STAR, alpha_max
# first confirm near-zero behaviour
eps_ok = same_table(1e-12)
print(f"table at alpha=1e-12 identical: {eps_ok}")
print(f"table at alpha=0 limit: {make_table(0.0)} "
      f"({'same' if make_table(0.0)==T else 'DIFFERENT'})")

# find first alpha above ALPHA_STAR where table changes (scan + bisect)
step_hit = None
N = 200000
for i in range(1, N+1):
    a = ALPHA_STAR + (alpha_max - ALPHA_STAR) * i / N
    if not same_table(a):
        step_hit = a
        break
if step_hit:
    lo_b, hi_b = step_hit - (alpha_max - ALPHA_STAR)/N, step_hit
    for _ in range(200):
        mid = (lo_b + hi_b)/2
        if same_table(mid):
            lo_b = mid
        else:
            hi_b = mid
    crit = (lo_b + hi_b)/2
    print(f"UPPER bifurcation alpha* = {crit:.12f}")
    print(f"  ratio to fine-structure alpha: {crit/ALPHA_STAR:.3f}x")
    print(f"  ratio to phi^2/4 (max): {crit/alpha_max:.6f}")
    print(f"  new table just above: {make_table(crit*1.0001)}")
else:
    print("table never changes up to discriminant limit")

# how wide is the whole valid window as fraction of possible alpha range
print(f"\nfine-structure alpha = {ALPHA_STAR:.6e}, "
      f"max allowed = {alpha_max:.6f}")
