"""Avenue 5 -- Associator cocycle & loop-theory placement.

Places GPL precisely in the non-associative taxonomy:
  1. omega(a,b,c) in {0,1} associator-defect function (0=same, 1=reflection;
     "other" would be a well-definedness failure -- must be 0 for the real table).
  2. Standard magma/loop identities tested exhaustively over ALL applicable
     tuples against the exact production table: flexible, left/right
     alternative, power-associativity, left/right Bol, four Moufang variants,
     commutativity (sanity anchor).
  3. Orbit-projection cocycle probe: does omega descend to a well-defined
     cochain on the Z2 quotient, and if so (or via canonical lifts if not),
     does it satisfy the standard inhomogeneous 3-cocycle / pentagon identity
     used to classify associativity constraints (group cohomology
     H^3(G, Z2), trivial action)?

STANDING RULE: never reimplement compose. The table below is asserted
verbatim against the exact tuple given in the roadmap / THEOREMS doc and
against scripts/research/gpl_program.py's derivation from alpha* before any
other computation runs.
"""
import math
from itertools import product

# ---------------------------------------------------------------------------
# 0. THE TABLE -- asserted, not reimplemented.
# ---------------------------------------------------------------------------
TABLE_LITERAL = ((0, 1, 2, 3), (1, 3, 3, 2), (2, 3, 3, 1), (3, 2, 1, 0))

PHI = (1 + 5 ** 0.5) / 2
RAIL = math.pi / PHI
ALPHA_STAR = 7.2973525693e-3


def make_table(alpha):
    """Exact derivation copied from gpl_program.py / gpl_next_level.py
    (same formula, not a reimplementation of compose -- this reproduces the
    generator so we can assert the literal table matches the derived one)."""
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


DERIVED = make_table(ALPHA_STAR)
assert DERIVED == TABLE_LITERAL, (
    f"table mismatch: derived {DERIVED} != literal {TABLE_LITERAL}"
)
T = TABLE_LITERAL


def compose(a, b):
    return T[a][b]


rho = lambda x: 3 - x

REPORT_LINES = []


def log(s=""):
    print(s)
    REPORT_LINES.append(s)


log("=== 0. Table assertion ===")
log(f"table = {T}  (matches derivation from alpha*={ALPHA_STAR}: True)")
log("")

# ---------------------------------------------------------------------------
# 1. omega(a,b,c) -- the associator-defect function
# ---------------------------------------------------------------------------
log("=== 1. omega(a,b,c) definition & well-definedness ===")


def omega_raw(a, b, c):
    """Returns 0 (same), 1 (reflection), or None (OTHER -- would break
    well-definedness of omega as a {0,1}-valued function)."""
    Lv = compose(compose(a, b), c)
    Rv = compose(a, compose(b, c))
    if Lv == Rv:
        return 0
    if Rv == rho(Lv):
        return 1
    return None


omega_table = {}
other_count = 0
same_count = 0
refl_count = 0
for (a, b, c) in product(range(4), repeat=3):
    w = omega_raw(a, b, c)
    if w is None:
        other_count += 1
    else:
        omega_table[(a, b, c)] = w
        if w == 0:
            same_count += 1
        else:
            refl_count += 1

n_triples = 4 ** 3
log(f"triples checked: {n_triples}")
log(f"same (omega=0): {same_count}   reflection (omega=1): {refl_count}   "
    f"OTHER (undefined): {other_count}")
OMEGA_WELL_DEFINED = other_count == 0
log(f"omega well-defined on {{0,1}} for ALL triples: "
    f"{'PASS' if OMEGA_WELL_DEFINED else 'FAIL'}")
log("")

# ---------------------------------------------------------------------------
# 2. Standard identities, each PASS/FAIL over ALL applicable tuples
# ---------------------------------------------------------------------------
log("=== 2. Standard non-associative-algebra identities ===")

results = []  # (name, formula_str, n_checked, n_pass, verdict)


def check(name, formula, n_vars, pred):
    n_checked = 0
    n_fail = 0
    first_fail = None
    for tup in product(range(4), repeat=n_vars):
        n_checked += 1
        if not pred(*tup):
            n_fail += 1
            if first_fail is None:
                first_fail = tup
    verdict = "PASS" if n_fail == 0 else "FAIL"
    results.append((name, formula, n_checked, n_checked - n_fail, n_fail, verdict, first_fail))
    return verdict


# -- sanity anchor: commutativity --
check(
    "commutativity",
    "a o b = b o a",
    2,
    lambda a, b: compose(a, b) == compose(b, a),
)

# -- flexible law --
check(
    "flexible",
    "(a o b) o a = a o (b o a)",
    2,
    lambda a, b: compose(compose(a, b), a) == compose(a, compose(b, a)),
)

# -- left alternative --
check(
    "left alternative",
    "(a o a) o b = a o (a o b)",
    2,
    lambda a, b: compose(compose(a, a), b) == compose(a, compose(a, b)),
)

# -- right alternative --
check(
    "right alternative",
    "(b o a) o a = b o (a o a)",
    2,
    lambda a, b: compose(compose(b, a), a) == compose(b, compose(a, a)),
)

# -- power-associativity --
# well-defined powers: every parenthesization of a^n (n copies of a) gives
# the SAME value (not merely the same reflection orbit -- this is a strictly
# stronger requirement than Theorem 1 coherence).
from functools import lru_cache


@lru_cache(maxsize=None)
def all_evals_word(word):
    if len(word) == 1:
        return frozenset(word)
    out = set()
    for i in range(1, len(word)):
        for l in all_evals_word(word[:i]):
            for r in all_evals_word(word[i:]):
                out.add(compose(l, r))
    return frozenset(out)


pa_rows = []
pa_all_pass = True
for a in range(4):
    for n in range(2, 8):
        word = tuple([a] * n)
        vals = all_evals_word(word)
        ok = len(vals) == 1
        pa_all_pass &= ok
        pa_rows.append((a, n, sorted(vals), ok))
results.append((
    "power-associativity",
    "all parenthesizations of a^n give a single value, n=2..7",
    4 * 6,
    sum(1 for r in pa_rows if r[3]),
    sum(1 for r in pa_rows if not r[3]),
    "PASS" if pa_all_pass else "FAIL",
    next((r for r in pa_rows if not r[3]), None),
))

# -- left Bol --
# a o (b o (a o c)) = (a o (b o a)) o c
check(
    "left Bol",
    "a o (b o (a o c)) = (a o (b o a)) o c",
    3,
    lambda a, b, c: compose(a, compose(b, compose(a, c)))
    == compose(compose(a, compose(b, a)), c),
)

# -- right Bol --
# ((c o a) o b) o a = c o ((a o b) o a)
check(
    "right Bol",
    "((c o a) o b) o a = c o ((a o b) o a)",
    3,
    lambda a, b, c: compose(compose(compose(c, a), b), a)
    == compose(c, compose(compose(a, b), a)),
)

# -- Moufang identities (4 standard forms, Bruck/Pflugfelder conventions) --
# M1 (left Moufang):   (a o (a o b)) o c = a o (a o (b o c))  -- wait, use canonical:
# Using Pflugfelder "Quasigroups and Loops" standard forms:
#   (M1) ((x y) x) z = x (y (x z))              left Moufang
#   (M2) z (x (y x)) = ((z x) y) x               right Moufang
#   (M3) (x y)(z x)  = x ((y z) x)               middle / "Moufang identity"
#   (M4) (x y)(z x)  = (x (y z)) x               middle, alternate bracketing
check(
    "Moufang M1 (left)",
    "((a o b) o a) o c = a o (b o (a o c))",
    3,
    lambda a, b, c: compose(compose(compose(a, b), a), c)
    == compose(a, compose(b, compose(a, c))),
)
check(
    "Moufang M2 (right)",
    "c o (a o (b o a)) = ((c o a) o b) o a",
    3,
    lambda a, b, c: compose(c, compose(a, compose(b, a)))
    == compose(compose(compose(c, a), b), a),
)
check(
    "Moufang M3 (middle)",
    "(a o b) o (c o a) = a o ((b o c) o a)",
    3,
    lambda a, b, c: compose(compose(a, b), compose(c, a))
    == compose(a, compose(compose(b, c), a)),
)
check(
    "Moufang M4 (middle, alt bracketing)",
    "(a o b) o (c o a) = (a o (b o c)) o a",
    3,
    lambda a, b, c: compose(compose(a, b), compose(c, a))
    == compose(compose(a, compose(b, c)), a),
)

log(f"{'identity':<38}{'tuples':>8}{'pass':>8}{'fail':>8}  verdict")
for (name, formula, n_checked, n_pass, n_fail, verdict, first_fail) in results:
    log(f"{name:<38}{n_checked:>8}{n_pass:>8}{n_fail:>8}  {verdict}")
log("")
log("Formulas tested (exact strings):")
for (name, formula, *_rest) in results:
    log(f"  {name}: {formula}")
log("")
log("Power-associativity detail (first failure per element, if any):")
by_a = {}
for (a, n, vals, ok) in pa_rows:
    by_a.setdefault(a, []).append((n, vals, ok))
for a in range(4):
    rows = by_a[a]
    fails = [(n, vals) for (n, vals, ok) in rows if not ok]
    if fails:
        log(f"  a={a}: FAIL first at n={fails[0][0]}, values={fails[0][1]}")
    else:
        log(f"  a={a}: PASS for n=2..7 (all single-valued)")
log("")

# ---------------------------------------------------------------------------
# 3. Cocycle probe: does omega descend to the Z2 orbit quotient, and does
#    the descended (or canonical-lift) cochain satisfy the standard
#    inhomogeneous 3-cocycle / pentagon identity?
# ---------------------------------------------------------------------------
log("=== 3. Cocycle probe on the Z2 orbit projection ===")
orb = lambda x: 0 if x in (0, 3) else 1  # O={0,3}->0, I={1,2}->1 (threshold_energy)
log("orbit map: orb(0)=orb(3)=0 [O], orb(1)=orb(2)=1 [I]  (== threshold_energy)")

# 3a. Is omega fiber-constant over the orbit projection? i.e. is
# omega(a,b,c) determined solely by (orb(a),orb(b),orb(c))?
fiber_map = {}
fiber_constant = True
conflicts = []
for (a, b, c), w in omega_table.items():
    key = (orb(a), orb(b), orb(c))
    if key not in fiber_map:
        fiber_map[key] = set()
    fiber_map[key].add(w)
for key, vals in fiber_map.items():
    if len(vals) > 1:
        fiber_constant = False
        conflicts.append((key, sorted(vals)))

log(f"omega fiber-constant over orbit triples (well-defined omega_bar: Z2^3->Z2 "
    f"induced directly from omega): {'PASS' if fiber_constant else 'FAIL'}")
if not fiber_constant:
    log(f"  conflicting orbit-triples (orbit-triple -> omega values seen): "
        f"{len(conflicts)} of {len(fiber_map)} orbit-triples conflict")
    for key, vals in conflicts:
        log(f"    orbits{key} -> omega values {vals}")
log("")

# 3b. Since (per 3a, expected) omega is NOT fiber-constant, there is no
# canonical omega_bar induced directly. We test the pentagon/3-cocycle
# identity two ways, both documented honestly:
#   (i)  CANONICAL-LIFT cochain: omega_bar(g,h,k) := omega(rep(g),rep(h),rep(k))
#        with rep(0)=0 (representative of O), rep(1)=1 (representative of I).
#        This is *a* choice, not *the* induced map -- flagged as such.
#   (ii) EXISTENCE probe: does there EXIST any lift-independent value, i.e.
#        for each orbit-triple grab the majority/first value -- already
#        covered by (i) being one concrete instance; we also check whether
#        using the OTHER representative pair rep(0)=3, rep(1)=2 changes the
#        verdict (representative-independence check).

rep_pairs = {
    "rep(O)=0, rep(I)=1": {0: 0, 1: 1},
    "rep(O)=3, rep(I)=2": {0: 3, 1: 2},
    "rep(O)=0, rep(I)=2": {0: 0, 1: 2},
    "rep(O)=3, rep(I)=1": {0: 3, 1: 1},
}


def omega_bar_from_rep(rep):
    def f(g, h, k):
        return omega_table[(rep[g], rep[h], rep[k])]
    return f


# Standard inhomogeneous 3-cocycle / pentagon identity for a 3-cochain
# omega_bar: G x G x G -> A (G=Z2 written additively, A=Z2, TRIVIAL action):
#   omega_bar(h,k,l) + omega_bar(g,h+k,l) + omega_bar(g,h,k)
#     == omega_bar(g,h,k+l) + omega_bar(g+h,k,l)      (mod 2)
# This is exactly the pentagon condition used to classify associativity
# constraints (associator 3-cocycles) for group-graded quasialgebras /
# twisted group algebras, e.g. Albuquerque-Majid's Z2^3-cocycle
# construction of the octonions from the group algebra of Z2^3.

def cocycle_identity_holds(omega_bar):
    n_checked = 0
    n_fail = 0
    first_fail = None
    for g, h, k, l in product(range(2), repeat=4):
        lhs = (omega_bar(h, k, l) + omega_bar(g, (h + k) % 2, l) + omega_bar(g, h, k)) % 2
        rhs = (omega_bar(g, h, (k + l) % 2) + omega_bar((g + h) % 2, k, l)) % 2
        n_checked += 1
        if lhs != rhs:
            n_fail += 1
            if first_fail is None:
                first_fail = (g, h, k, l, lhs, rhs)
    return n_checked, n_fail, first_fail


log("Exact identity tested (inhomogeneous 3-cocycle / pentagon condition, "
    "G=Z2 additive, trivial action, all mod 2):")
log("  omega_bar(h,k,l) + omega_bar(g,h+k,l) + omega_bar(g,h,k)")
log("    == omega_bar(g,h,k+l) + omega_bar(g+h,k,l)   for all g,h,k,l in Z2")
log("")
log(f"{'representative choice':<28}{'tuples':>8}{'fail':>8}  verdict")
for name, rep in rep_pairs.items():
    ob = omega_bar_from_rep(rep)
    n_checked, n_fail, first_fail = cocycle_identity_holds(ob)
    verdict = "PASS" if n_fail == 0 else "FAIL"
    log(f"{name:<28}{n_checked:>8}{n_fail:>8}  {verdict}"
        + (f"   first fail @ {first_fail}" if first_fail else ""))
log("")
log("Representative-independence: since omega is NOT fiber-constant (3a), "
    "these four canonical-lift cochains are four genuinely different "
    "3-cochains, not the same map computed four ways. Agreement/disagreement "
    "across them is itself part of the verdict, not noise.")
log("")

# ---------------------------------------------------------------------------
# 4. Summary block for the report
# ---------------------------------------------------------------------------
log("=== 4. Machine-readable summary ===")
log(f"OMEGA_WELL_DEFINED={OMEGA_WELL_DEFINED}")
log(f"SAME={same_count} REFLECTION={refl_count} OTHER={other_count} (of {n_triples})")
for (name, formula, n_checked, n_pass, n_fail, verdict, first_fail) in results:
    log(f"IDENTITY[{name}]={verdict} checked={n_checked} fail={n_fail}")
log(f"FIBER_CONSTANT_OMEGA_BAR={fiber_constant}")

if __name__ == "__main__":
    pass
