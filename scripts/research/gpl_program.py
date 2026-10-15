"""GPL research program: alpha atlas, sibling algebra, phase lattice CA,
GPL(6) probe, and honest GPL-vs-CRC Monte Carlo (against the real kairos spec).
"""
import math, random
from itertools import product
from functools import lru_cache

PHI = (1 + 5**0.5) / 2
RAIL = math.pi / PHI
ALPHA_STAR = 7.2973525693e-3
ALPHA_MAX = PHI * PHI / 4

def make_table(alpha):
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

T = make_table(ALPHA_STAR)
rho = lambda x: 3 - x

def analyze_table(t, name):
    print(f"\n=== {name} ===")
    print("table:", t)
    comm = all(t[a][b] == t[b][a] for a in range(4) for b in range(4))
    ident = all(t[0][x] == x for x in range(4))
    refl_row = tuple(t[3][x] for x in range(4)) == (3,2,1,0)
    same = refl = other = 0
    for a,b,c in product(range(4), repeat=3):
        L = t[t[a][b]][c]; R = t[a][t[b][c]]
        if L == R: same += 1
        elif R == rho(L): refl += 1
        else: other += 1
    print(f"commutative={comm} identity0={ident} row3=reflection:{refl_row}")
    print(f"topology 64 triples: same={same} reflection={refl} OTHER={other}")
    # orbit compatibility (coherence inductive step)
    orbit_ok = True
    for a,b in product(range(4), repeat=2):
        vals = {t[a][b], t[a][rho(b)], t[rho(a)][b], t[rho(a)][rho(b)]}
        if not any(vals <= {v, 3-v} for v in vals): orbit_ok = False
    # quotient on orbits O={0,3}=0, I={1,2}=1
    orb = lambda x: 0 if x in (0,3) else 1
    quot_welldef = orbit_ok
    quot = [[orb(t[a][b]) for b in (0,1)] for a in (0,1)] if quot_welldef else None
    isZ2 = quot_welldef and all(orb(t[a][b]) == (orb(a)^orb(b))
                                for a in range(4) for b in range(4))
    print(f"orbit-compatible(coherence)={orbit_ok} quotient==Z2(XOR)={isZ2}")
    return dict(same=same, refl=refl, other=other, coh=orbit_ok, z2=isZ2)

# ---------------- 1. ALPHA ATLAS ----------------
print("################ 1. ALPHA ATLAS ################")
N = 400000
tables_seen = []   # (table, alpha_first)
prev = None
boundaries = []
alphas = [ALPHA_MAX * (i+0.5)/N for i in range(N)]
for a in alphas:
    t = make_table(a)
    if t != prev:
        if prev is not None:
            boundaries.append(a)
        if t not in [x[0] for x in tables_seen]:
            tables_seen.append((t, a))
        prev = t
# refine boundaries by bisection
refined = []
for b in boundaries:
    lo, hi = b - ALPHA_MAX/N, b
    tlo = make_table(lo)
    for _ in range(200):
        mid = (lo+hi)/2
        if make_table(mid) == tlo: lo = mid
        else: hi = mid
    refined.append((lo+hi)/2)
print(f"distinct tables across alpha in (0, {ALPHA_MAX:.4f}): {len(tables_seen)}")
print("bifurcation points:", [f"{b:.9f}" for b in refined])
for i, (t, a0) in enumerate(tables_seen):
    stats = analyze_table(t, f"ALGEBRA #{i+1} (first seen near alpha={a0:.6f})")

# ---------------- 2. PHASE LATTICE CA ----------------
print("\n################ 2. PHASE LATTICE CA ################")
def comp(a,b): return T[a][b]
def stepL(cfg):
    n = len(cfg)
    return tuple(comp(comp(cfg[(i-1)%n], cfg[i]), cfg[(i+1)%n]) for i in range(n))
def stepR(cfg):
    n = len(cfg)
    return tuple(comp(cfg[(i-1)%n], comp(cfg[i], cfg[(i+1)%n])) for i in range(n))

# homogeneous behavior
print("homogeneous rings (rule L): ", end="")
for v in range(4):
    seq = [v]
    c = (v,)*4
    for _ in range(6):
        c = stepL(c); seq.append(c[0])
    print(f"{v}->{seq[1:5]}", end="  ")
print()

# exhaustive attractor analysis N=8
n = 8
total = 4**n
nxt = {}
for idx in range(total):
    cfg = tuple((idx >> (2*i)) & 3 for i in range(n))
    nc = stepL(cfg)
    nxt[idx] = sum(v << (2*i) for i, v in enumerate(nc))
# find cycles via functional graph
color = [0]*total  # 0 unvisited 1 in-progress 2 done
on_cycle = set()
cycle_lens = {}
for s in range(total):
    if color[s]: continue
    path = []
    x = s
    while color[x] == 0:
        color[x] = 1; path.append(x); x = nxt[x]
    if color[x] == 1:  # new cycle found
        ci = path.index(x)
        cyc = path[ci:]
        for y in cyc: on_cycle.add(y)
        cycle_lens[len(cyc)] = cycle_lens.get(len(cyc), 0) + 1
    for y in path: color[y] = 2
print(f"N=8 ring, rule L: {total} configs, {len(on_cycle)} on attractors")
print("cycle length -> count of cycles:", dict(sorted(cycle_lens.items())))

# transient length stats (sample)
random.seed(7)
def transient_and_period(cfg):
    seen = {}
    t_ = 0
    while cfg not in seen:
        seen[cfg] = t_; cfg = stepL(cfg); t_ += 1
    return seen[cfg], t_ - seen[cfg]
samp = [tuple(random.randrange(4) for _ in range(n)) for _ in range(2000)]
trs = [transient_and_period(c) for c in samp]
print(f"N=8 sampled transients: max={max(t for t,_ in trs)} "
      f"mean={sum(t for t,_ in trs)/len(trs):.2f}; "
      f"periods seen={sorted(set(p for _,p in trs))}")

# defect field (causal curvature) on N=64
n2 = 64
dens_series = []
for trial in range(50):
    cfg = tuple(random.randrange(4) for _ in range(n2))
    dens = []
    for step in range(40):
        l, r = stepL(cfg), stepR(cfg)
        dens.append(sum(1 for i in range(n2) if l[i] != r[i]) / n2)
        cfg = l
    dens_series.append(dens)
avg = [sum(d[k] for d in dens_series)/len(dens_series) for k in range(40)]
print("N=64 defect density (rule L vs R disagreement), steps 0,1,2,5,10,20,39:")
print("  ", [f"{avg[k]:.3f}" for k in (0,1,2,5,10,20,39)])
# state histogram long-run
hist = [0]*4
for trial in range(50):
    cfg = tuple(random.randrange(4) for _ in range(n2))
    for _ in range(100): cfg = stepL(cfg)
    for v in cfg: hist[v] += 1
tot = sum(hist)
print("long-run state distribution (100 steps):",
      [f"P{i}={hist[i]/tot:.3f}" for i in range(4)])

# ---------------- 3. GPL(6) PROBE ----------------
print("\n################ 3. GPL(6) PROBE ################")
# family: kernel (y^2-z1)(y^2-z2)(y^2-z3), constraint z1+z2+z3 = PHI (golden trace)
def make6(z1, z2, z3):
    zs = sorted([z1, z2, z3])
    if zs[0] <= 0: return None
    r = [math.sqrt(z) for z in zs]
    s = [-r[2], -r[1], -r[0], r[0], r[1], r[2]]
    lo, hi = s[0], s[5]
    st = [(x-lo)/(hi-lo)*RAIL for x in s]
    b = [(st[i]+st[i+1])/2 for i in range(5)]
    fold = lambda x: x if x <= RAIL else 2*RAIL - x
    def snap(x):
        for i in range(5):
            if x <= b[i]: return i
        return 5
    return tuple(tuple(snap(fold(st[a]+st[bb])) for bb in range(6)) for a in range(6))

rho6 = lambda x: 5 - x
def check6(t):
    ident = all(t[0][x] == x for x in range(6))
    refl = tuple(t[5][x] for x in range(6)) == (5,4,3,2,1,0)
    # orbit compatibility for orbits {0,5},{1,4},{2,3}
    orb = lambda x: 0 if x in (0,5) else 1 if x in (1,4) else 2
    coh = True
    for a,b in product(range(6), repeat=2):
        vals = {t[a][b], t[a][rho6(b)], t[rho6(a)][b], t[rho6(a)][rho6(b)]}
        if not any(vals <= {v, 5-v} for v in vals): coh = False; break
    z3q = coh and all(orb(t[a][b]) == (orb(a)+orb(b)) % 3
                      for a in range(6) for b in range(6))
    # associator classification
    same = refl_t = other = 0
    for a,b,c in product(range(6), repeat=3):
        L = t[t[a][b]][c]; R = t[a][t[b][c]]
        if L == R: same += 1
        elif R == rho6(L): refl_t += 1
        else: other += 1
    return ident, refl, coh, z3q, same, refl_t, other

best = []
G = 70
found_coh = 0; found_z3 = 0; total_pts = 0
for i in range(1, G):
    for j in range(i+1, G):
        z2 = PHI * i / G; z3 = PHI * j / G
        z1 = PHI - z2 - z3
        if z1 <= 0 or z1 >= z2: continue
        t = make6(z1, z2, z3)
        if t is None: continue
        total_pts += 1
        ident, refl, coh, z3q, same, refl_t, other = check6(t)
        if ident and refl and coh:
            found_coh += 1
            if z3q: found_z3 += 1
            if len(best) < 3 or z3q:
                best.append((z1, z2, z3, z3q, same, refl_t, other))
print(f"grid points tested: {total_pts}")
print(f"with identity+reflection-row+COHERENCE: {found_coh}")
print(f"...of those, quotient == Z3: {found_z3}")
for z1,z2,z3,z3q,s_,r_,o_ in best[:6]:
    print(f"  z=({z1:.4f},{z2:.4f},{z3:.4f}) Z3quot={z3q} "
          f"topology216: same={s_} refl={r_} other={o_}")

# ---------------- 4. GPL vs CRC (honest Monte Carlo) ----------------
print("\n################ 4. GPL vs CRC MONTE CARLO ################")
# real kairos spec: chi weights
delta = math.sqrt(PHI*PHI - 4*ALPHA_STAR)
lam_p = math.sqrt((PHI+delta)/2); lam_m = math.sqrt((PHI-delta)/2)
sqa = math.sqrt(ALPHA_STAR)
SHEET = {0: sqa/(-lam_p), 1: sqa/(-lam_m), 2: sqa/lam_m, 3: sqa/lam_p}
def gpl_checksum(sym):
    acc = 0; chi = 0.0
    for x in sym:
        acc = T[acc][x]; chi += SHEET[x]
    return acc, round(chi, 12)
def topo_sig(sym):
    out = []
    for i in range(len(sym)-2):
        a,b,c = sym[i:i+3]
        L = T[T[a][b]][c]; R = T[a][T[b][c]]
        out.append(0 if L == R else 1)
    return tuple(out)
def pack(sym):
    out = []
    for i in range(0, len(sym), 4):
        out.append(sym[i]<<6 | sym[i+1]<<4 | sym[i+2]<<2 | sym[i+3])
    return bytes(out)
def crc8(data):
    c = 0
    for byte in data:
        c ^= byte
        for _ in range(8):
            c = ((c << 1) ^ 0x07) & 0xFF if c & 0x80 else (c << 1) & 0xFF
    return c
def crc16(data):
    c = 0xFFFF
    for byte in data:
        c ^= byte << 8
        for _ in range(8):
            c = ((c << 1) ^ 0x1021) & 0xFFFF if c & 0x8000 else (c << 1) & 0xFFFF
    return c

L = 64  # symbols = 16 bytes
NT = 20000
random.seed(42)
def corrupt(sym, mode):
    s = list(sym)
    if mode == "sub1":
        i = random.randrange(L); s[i] = random.choice([v for v in range(4) if v != s[i]])
    elif mode == "swap_adj":
        for _ in range(50):
            i = random.randrange(L-1)
            if s[i] != s[i+1]: s[i], s[i+1] = s[i+1], s[i]; break
    elif mode == "reflect1":
        i = random.randrange(L); s[i] = 3 - s[i]
    elif mode == "burst4":
        i = random.randrange(L-3)
        for k in range(4): s[i+k] = random.randrange(4)
    elif mode == "shuffle":
        random.shuffle(s)
    elif mode == "chi_evade":  # rho on a {1} site and on a {2} site: chi unchanged
        ones = [i for i,v in enumerate(sym) if v == 1]
        twos = [i for i,v in enumerate(sym) if v == 2]
        if not ones or not twos: return None
        s[random.choice(ones)] = 2; s[random.choice(twos)] = 1
    return tuple(s)

modes = ["sub1", "swap_adj", "reflect1", "burst4", "shuffle", "chi_evade"]
det = {m: {"gpl": 0, "gpl_topo": 0, "crc8": 0, "crc16": 0, "n": 0} for m in modes}
for _ in range(NT):
    sym = tuple(random.randrange(4) for _ in range(L))
    ref_gpl = gpl_checksum(sym); ref_topo = topo_sig(sym)
    pk = pack(sym); ref_c8 = crc8(pk); ref_c16 = crc16(pk)
    for m in modes:
        c = corrupt(sym, m)
        if c is None or c == sym: continue
        det[m]["n"] += 1
        g = gpl_checksum(c) != ref_gpl
        if g: det[m]["gpl"] += 1
        if g or topo_sig(c) != ref_topo: det[m]["gpl_topo"] += 1
        pc = pack(c)
        if crc8(pc) != ref_c8: det[m]["crc8"] += 1
        if crc16(pc) != ref_c16: det[m]["crc16"] += 1
print(f"stream = {L} symbols (16 bytes), {NT} trials/class")
print(f"{'error class':<12}{'GPL(acc+chi)':>14}{'GPL+topology':>14}{'CRC-8':>10}{'CRC-16':>10}")
for m in modes:
    d = det[m]; nn = d["n"]
    if nn == 0: print(f"{m:<12}  (no applicable trials)"); continue
    print(f"{m:<12}{d['gpl']/nn:>13.4f} {d['gpl_topo']/nn:>13.4f}"
          f"{d['crc8']/nn:>10.4f}{d['crc16']/nn:>10.4f}")
# chi information content honesty
print(f"\noverhead honesty: GPL acc=2 bits + chi lattice ~2*log2(2L+1)="
      f"{2*math.log2(2*L+1):.1f} bits ~= {2+2*math.log2(2*L+1):.0f} bits total; "
      f"CRC-8 = 8 bits; CRC-16 = 16 bits; topology signature = {L-2} bits (trace, not checksum)")
