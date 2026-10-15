"""AVENUE 3: Sibling algebras #2 and #3 deep map.

Reproduce the two non-coherent algebras above α*=0.096820784 (algebra #2)
and above 0.418885438 (algebra #3). For each:
  (1) List all 10 "other" associator triples and their (L,R) pairs
  (2) Search all 24 permutations: does ANY involution sigma control defect?
  (3) Word-coherence failure rate for lengths 3-7
  (4) Check for associative quotients under every partition

"""
import math
import random
from itertools import product, combinations, permutations

PHI = (1 + 5**0.5) / 2
RAIL = math.pi / PHI
ALPHA_STAR = 7.2973525693e-3
ALPHA_MAX = PHI * PHI / 4

def make_table(alpha):
    """Generate GPL table at given alpha."""
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

# ========== Find exact siblings ==========
print("=" * 70)
print("AVENUE 3: SIBLING ALGEBRAS #2 AND #3")
print("=" * 70)

# Locate siblings via known boundaries
alphas_to_check = [
    0.11,   # within algebra #2 (range 0.0968..0.4188)
    0.50,   # within algebra #3 (range > 0.4188)
]

sibling_tables = {}
for test_alpha in alphas_to_check:
    t = make_table(test_alpha)
    if t:
        # Determine which sibling by comparing to boundaries
        if test_alpha < 0.096820784:
            label = "GPL (#1)"
        elif test_alpha < 0.418885438:
            label = "SIBLING #2"
        else:
            label = "SIBLING #3"
        sibling_tables[label] = (t, test_alpha)

print(f"\nLocated:")
for label, (t, alpha) in sibling_tables.items():
    print(f"  {label} at alpha = {alpha}")

# ========== Analysis function ==========
def analyze_sibling(table, name, alpha_val):
    """Deep analysis of a sibling algebra."""
    print(f"\n{'='*70}")
    print(f"{name} (alpha = {alpha_val})")
    print(f"{'='*70}")

    rho = lambda x: 3 - x

    # 1. ASSOCIATOR CLASSIFICATION
    print("\n--- PART 1: Associator Classification ---")

    assoc_same = []
    assoc_refl = []
    assoc_other = []

    for a, b, c in product(range(4), repeat=3):
        L = table[table[a][b]][c]
        R = table[a][table[b][c]]
        if L == R:
            assoc_same.append((a,b,c))
        elif R == rho(L):
            assoc_refl.append((a,b,c))
        else:
            assoc_other.append((a,b,c))

    print(f"Total triples: {len(assoc_same)} same + {len(assoc_refl)} reflection + {len(assoc_other)} other = 64")
    print(f"\nOTHER associator triples (defect not controlled by rho):")
    print(f"Count: {len(assoc_other)}")

    # Collect (L,R) pairs for each "other" triple
    other_lrs = []
    for a, b, c in assoc_other:
        L = table[table[a][b]][c]
        R = table[a][table[b][c]]
        other_lrs.append(((a,b,c), L, R))
        print(f"  {(a,b,c)}: L={L} R={R} (neither same nor rho-related)")

    # 2. INVOLUTION SEARCH
    print(f"\n--- PART 2: Involution Control (search all 24 permutations) ---")

    involution_found = None
    involution_defect_controlled = False

    # All 24 permutations of {0,1,2,3}
    for perm in permutations(range(4)):
        sigma = lambda x: perm[x]

        # Check if this sigma is an involution: sigma(sigma(x)) = x for all x
        is_involution = all(sigma(sigma(x)) == x for x in range(4))
        if not is_involution:
            continue

        # Check if R = sigma(L) for ALL "other" triples
        all_controlled = True
        for a, b, c in assoc_other:
            L = table[table[a][b]][c]
            R = table[a][table[b][c]]
            if R != sigma(L):
                all_controlled = False
                break

        if all_controlled:
            involution_found = perm
            involution_defect_controlled = True
            break

    if involution_defect_controlled:
        sigma_str = f"sigma(x)={[involution_found[x] for x in range(4)]}"
        print(f"[OK] DEFECT CONTROLLED by involution: {sigma_str}")
        print(f"  For all {len(assoc_other)} 'other' triples: R = sigma(L) HOLDS")
    else:
        print(f"[FAIL] NO involution controls the defect")
        print(f"  Tried all 24 permutations, checked only the 8 involutions:")
        involutions_tried = [list(p) for p in permutations(range(4))
                             if all(p[p[x]] == x for x in range(4))]
        for inv_perm in involutions_tried[:3]:  # Show first 3
            print(f"    sigma={inv_perm}: fails on some triple")

    # 3. REFLECTION-ORBIT COHERENCE (GPL-STYLE)
    print(f"\n--- PART 3: Reflection-Orbit Coherence (GPL-style, lengths 3-7) ---")

    def eval_word(word, table):
        """Evaluate a word (list of indices) under all parenthesizations."""
        if len(word) == 1:
            return {word[0]}

        values = set()
        for i in range(1, len(word)):
            left_vals = eval_word(word[:i], table)
            right_vals = eval_word(word[i:], table)
            for lv in left_vals:
                for rv in right_vals:
                    values.add(table[lv][rv])
        return values

    # Test reflection-orbit coherence (GPL-style: rho(x)=3-x)
    # Orbits are {0,3} and {1,2}
    def get_orbit(x):
        """Map element to its reflection orbit ID."""
        if x in (0, 3):
            return 0
        else:
            return 1

    reflection_orbits = {
        'O': {0, 3},  # orbit 0
        'I': {1, 2}   # orbit 1
    }

    failure_by_length = {}
    for length in range(3, 8):
        # Sample words of this length
        num_words = min(4**length, 5000)
        random.seed(42)
        words = [tuple(random.randrange(4) for _ in range(length)) for _ in range(num_words)]

        # For small lengths, enumerate all
        if length <= 3:
            words = list(product(range(4), repeat=length))

        failures = 0
        for word in words:
            values = eval_word(word, table)

            # Check if all values land in single reflection orbit
            orbits_hit = set()
            for v in values:
                orbits_hit.add(get_orbit(v))

            # Failure if we hit more than one orbit
            if len(orbits_hit) > 1:
                failures += 1

        failure_rate = failures / len(words)
        failure_by_length[length] = (failures, len(words), failure_rate)
        print(f"  Length {length}: {failures}/{len(words)} words fail = {failure_rate:.4f}")

    # 4. ASSOCIATIVE QUOTIENTS
    print(f"\n--- PART 4: Associative Quotient Search ---")

    # Generate all partitions of {0,1,2,3}
    def generate_partitions(items):
        """Generate all partitions of a set."""
        if len(items) == 0:
            yield []
            return
        if len(items) == 1:
            yield [frozenset(items)]
            return

        first = items[0]
        rest = items[1:]
        for smaller in generate_partitions(rest):
            # Put first in each existing part
            for i, part in enumerate(smaller):
                new_partition = smaller[:i] + [frozenset(part | {first})] + smaller[i+1:]
                yield new_partition
            # Put first in its own part
            yield smaller + [frozenset({first})]

    partitions = list(generate_partitions([0,1,2,3]))
    print(f"Testing all {len(partitions)} partitions for associativity of quotient table")

    associative_quotient_found = False
    for partition in partitions:
        # Map each element to its partition index
        elem_to_idx = {}
        for idx, part in enumerate(partition):
            for elem in part:
                elem_to_idx[elem] = idx

        # Build quotient table
        quot_table = {}
        well_defined = True
        for a in range(4):
            for b in range(4):
                idx_a = elem_to_idx[a]
                idx_b = elem_to_idx[b]
                result = table[a][b]
                idx_result = elem_to_idx[result]

                # For this to be well-defined, all (a,b) pairs with same indices
                # must map to elements in the same partition
                key = (idx_a, idx_b)
                if key not in quot_table:
                    quot_table[key] = idx_result
                elif quot_table[key] != idx_result:
                    well_defined = False
                    break
            if not well_defined:
                break

        if not well_defined:
            continue

        # Check if quotient table is associative
        n_parts = len(partition)
        associative = True
        for i, j, k in product(range(n_parts), repeat=3):
            L = quot_table.get((quot_table.get((i,j), None), k), None)
            R = quot_table.get((i, quot_table.get((j,k), None)), None)
            if L is not None and R is not None and L != R:
                associative = False
                break

        if associative:
            partition_str = [list(p) for p in partition]
            print(f"[OK] ASSOCIATIVE QUOTIENT FOUND: partition {partition_str}")
            print(f"  Quotient is associative under partition")
            associative_quotient_found = True

    if not associative_quotient_found:
        print(f"[FAIL] No associative quotient found under any partition")

    return {
        'name': name,
        'alpha': alpha_val,
        'same': len(assoc_same),
        'refl': len(assoc_refl),
        'other': len(assoc_other),
        'other_lrs': other_lrs,
        'involution_controlled': involution_defect_controlled,
        'involution': involution_found,
        'failure_by_length': failure_by_length,
        'associative_quotient_found': associative_quotient_found,
    }

# ========== RUN ANALYSIS ON BOTH SIBLINGS ==========
results = []

for label in ["SIBLING #2", "SIBLING #3"]:
    if label in sibling_tables:
        t, alpha = sibling_tables[label]
        result = analyze_sibling(t, label, alpha)
        results.append(result)

# ========== REPORT SUMMARY ==========
print(f"\n{'='*70}")
print("AVENUE 3 SUMMARY")
print(f"{'='*70}")

for result in results:
    print(f"\n{result['name']} (alpha={result['alpha']}):")
    print(f"  Topology: {result['same']}/0/{result['other']} (same/reflection/other)")

    if result['involution_controlled']:
        sigma = result['involution']
        print(f"  [OK] DEFECT CONTROLLED: sigma = {sigma}")
    else:
        print(f"  [FAIL] Defect UNCONTROLLED (no involution found)")

    print(f"  Reflection-orbit coherence failure (all evaluations in single {{0,3}} or {{1,2}}):")
    for length in range(3, 8):
        if length in result['failure_by_length']:
            fails, total, rate = result['failure_by_length'][length]
            print(f"    Length {length}: {rate:.4f} ({fails}/{total})")

    if result['associative_quotient_found']:
        print(f"  [OK] Associative quotient EXISTS")
    else:
        print(f"  [FAIL] No associative quotient found")

print(f"\n{'='*70}")
print("Reproduction command:")
print(f"  python scripts/research/avenue03_siblings.py")
print(f"{'='*70}")
