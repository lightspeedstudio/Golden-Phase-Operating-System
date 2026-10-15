# G64 Math → Fabric Mapping

## Why 64

Ordered triples over Φ = {0,1,2,3}: `|Φ|³ = 64`. Each cell of the atlas is one `(a,b,c)`.

## Structures

| Structure | Meaning |
|-----------|---------|
| Compose LUT / `g64_compose` | Computation: `a ∘ b` |
| Reflect / `g64_reflect` | Computation: `ρ(x)=3-x` |
| Triple unit | Computation: left/right associators + class bits |
| 64 lanes / slots | Parallel or resident atlas positions (runtime-loaded) |
| Identity/reflection/error masks | Topology reduction over the 64 positions |
| 8×8 grid indexing | Logical layout for row/col OR-reduction; **placement value unproven** until unconstrained timing/routing measured |
| Resident fabric (C7) | State, not just atlas recompute |

## Reflection

Canonical: `right == ρ(left)` (equiv. `left == ρ(right)`). Hardware must match `gphase_core::topology_status`, not a second table.

## Compression vs fidelity

A single pipelined engine can compute the same 64 classifications sequentially with identical math; lane count trades throughput/area, not algebraic meaning — if and only if inputs remain runtime-variable (no constant-folded fake lanes).
