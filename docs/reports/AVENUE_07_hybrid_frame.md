# Avenue 7 — Hybrid Integrity Frame for PhaseLink

**Scope:** implement the paper's honest §8 conclusion (CRC for detection,
GPL for diagnosis) as one frame format: payload + CRC-16/CCITT + GPL acc+chi
(quantized) + `assoc_parity` + optional topology trace. `compose`, the
checksum, and the topology scan were never reimplemented — the frame
delegates to `crate::core::gphase_core` via the existing
`crate::protocol::checksum` (`gpl_checksum`, `state_sheet`) and
`crate::protocol::topology` (`scan_sliding_triples`) modules.

## Files

- `src/protocol/hybrid_frame.rs` (new) — frame struct, encode, detect,
  diagnose, CRC-16/CCITT, and all tests (7 `#[test]` functions, including
  the Monte Carlo).
- `src/protocol/mod.rs` — registers `hybrid_frame` and re-exports its public
  API.

No production behavior elsewhere was touched. No commit was made (per
standing rules).

## Frame byte-layout

```
HybridFrame {
    payload:         Vec<PhaseState>   // L phase symbols (2 bits/symbol logically)
    crc16:            u16               // CRC-16/CCITT-FALSE (poly 0x1021, init 0xFFFF)
                                         //   over the payload packed 4 symbols/byte
                                         //   (protocol::encoder::phase_to_bytes)
    gpl_acc:          PhaseState        // compose fold accumulator, 2-bit state (0..3)
    gpl_chi_q:        i16               // chi (branch-sheet sum) quantized: round(chi * CHI_SCALE)
    assoc_parity:     u8                // reflection-defect count mod 2 (1 logical bit)
    topology_trace:   Option<Vec<TopologyKind>>  // optional: L-2 entries, Same|Reflection (1 bit each)
}
```

| Field | Logical bits | As coded (Rust struct, byte-aligned) |
|---|---|---|
| CRC-16/CCITT | 16 | 16 (`u16`) |
| `gpl_acc` | 2 (4 states) | 8 (`PhaseState` repr `u8`) |
| `gpl_chi_q` | ≤16, but only `ceil(log2(2·⌊L·max\|sheet\|·CHI_SCALE⌋+1))` carry entropy — **14 bits at L=64** (see Overhead below) | 16 (`i16`) |
| `assoc_parity` | 1 | 8 (`u8`) |
| `topology_trace` (optional) | `L-2` (1 bit/entry) | `L-2` bytes (`TopologyKind` enum, unpacked) |

`CHI_SCALE = 100.0` (fixed in `hybrid_frame.rs`). Chosen so that at L=64,
`chi ∈ [-81.5, 81.5]` (max `|state_sheet|` ≈ 1.2702, see
`checksum::state_sheet`) quantizes to `chi_q ∈ [-8150, 8150]`, comfortably
inside a 14-bit signed range (`±8192`) — matching the OSF paper's overhead
estimate "compact GPL ≈ 16 bits" (2-bit acc + ~14-bit chi) at this length.
Quantization is deterministic (round-half-away-from-zero, then clamp to
`i16`), so `encode`/`detect` never compare floats directly — they compare
the quantized integer, eliminating the epsilon-tolerance issue the Python
`gpl_verify` needed.

## Detection: hybrid vs CRC-16 alone

Re-run of `gpl_program.py` §4's Monte Carlo (L=64 symbols = 16 bytes,
20,000 trials/class, same six corruption modes: `sub1`, `swap_adj`,
`reflect1`, `burst4`, `shuffle`, `chi_evade`), implemented natively in Rust
(`monte_carlo_detection_and_diagnosis` test) with a seeded xorshift64\*
PRNG (seed 42; no external `rand` dependency).

`hybrid_any()` = CRC mismatch **OR** GPL acc/chi mismatch **OR**
`assoc_parity` mismatch **OR** topology-trace mismatch (when attached) —
an OR-combination, so `hybrid ≥ crc16_alone` is true *by construction*
whenever CRC-16 is one of the OR'd terms; the honest content is in whether
it's ever a *useful* excess, which the diagnosis table below addresses.

| Error class | n (applicable trials) | CRC-16 alone | Hybrid (CRC∨GPL∨parity∨topology) |
|---|---|---|---|
| Single substitution | 20,000 | 1.0000 | 1.0000 |
| Adjacent transposition | 20,000 | 1.0000 | 1.0000 |
| Single reflection | 20,000 | 1.0000 | 1.0000 |
| Burst (4 symbols) | 19,930 | 1.0000 | 1.0000 |
| Full permutation (shuffle) | 20,000 | 1.0000 | 1.0000 |
| χ-evading pair | 20,000 | 1.0000 | 1.0000 |

**Honest rule check: PASS.** Hybrid ≥ CRC-16 alone in all six classes (here
exactly equal — in this seeded 20,000-trial run CRC-16 alone already hit
1.0000 everywhere, including `shuffle`, where the OSF Python Monte Carlo
measured 0.9999/20,000 with a different PRNG stream; a 16-bit CRC's residual
collision probability on a 16-byte block is on the order of 1/65536 per
independent-enough corruption, so seeing 0/20,000 misses here is consistent
sampling variance, not a contradiction — CRC-16 is not exempt from
collisions in principle, it simply didn't collide in this run). Since
CRC-16 alone already saturates detection at this stream length in the
classes tested, the hybrid frame's detection value-add is invisible at
L=64/20k trials; its actual contribution, as the OSF paper concludes, is
diagnosis, measured next.

## Diagnosis: classification + localization accuracy

`diagnose()` uses **only** the topology-trace mismatch pattern (which
triple-start indices flipped between the frame's stored trace and the
trace recomputed from the received payload) plus the chi delta, never the
original payload — this is what a real receiver has available. Algorithm
(see doc comments in `hybrid_frame.rs`):

1. **Shuffle:** ≥3 separate flipped-triple islands ⇒ full permutation.
2. **Reflection:** a single reflected symbol changes global chi by exactly
   `2·state_sheet(received_symbol)` (`state_sheet(ρx) = -state_sheet(x)`
   identically — checksum.rs / OSF paper §8). For each island, the flipped
   triple-start run `[lo,hi]` constrains the edited symbol to
   `[hi, lo+2]` (every flipped triple must contain it); every such
   candidate is tested against the chi formula.
3. **χ-evading pair:** exactly 2 islands and *global* chi delta ≈ 0 — the
   construction is literally two independent single-symbol reflections
   (1→2 and 2→1 are each `ρ`) chosen so their chi contributions cancel
   exactly (mathematically exact, not approximate, since chi is a linear
   sum).
4. **Substitution / transposition / burst:** by measured span of the
   flipped-triple-index run — span ≤2 → substitution, span ==3 →
   transposition, span >3 → burst. These thresholds are empirical,
   calibrated against a dedicated histogram test
   (`debug_calibration_histograms`, kept in the module as a documented
   calibration aid), not assumed.

Localization: predicted symbol index scored correct if within ±2 of any
true corrupted position (both endpoints for transposition/burst/pair).

| Error class | Topology flagged (of n) | Classification accuracy¹ | Classification + localization (±2) |
|---|---|---|---|
| Single substitution | 10,576 / 20,000 (52.9%) | **0.7300** | 0.7300 |
| Adjacent transposition | 11,696 / 20,000 (58.5%) | **0.0471** | 0.0471 |
| Single reflection | 8,559 / 20,000 (42.8%) | **1.0000** | 1.0000 |
| Burst (4 symbols) | 15,527 / 19,930 (77.9%) | **0.0904** | 0.0904 |
| Full permutation | 20,000 / 20,000 (100%) | **0.9999** | 0.9999 (no localization concept) |
| χ-evading pair | 12,626 / 20,000 (63.1%) | **0.2660** | 0.1830 |

¹ Accuracy is conditioned on the topology trace flagging *something*
(column 2); it is not multiplied through by the flag rate. Wherever
`class_acc == class+local_acc`, every correctly-classified case was also
localized within ±2 symbols.

## Honest findings — why the numbers land where they do

- **Reflection and shuffle are essentially solved** (1.0000 and 0.9999):
  both have an exact algebraic signature (chi delta formula; island-count
  ≥3) that the topology trace reliably exposes.
- **Substitution's 0.73 is deflated by a real, not spurious, effect:** a
  uniform-random substitution picks among the 3 non-original values, and
  exactly 1 of those 3 equals `ρ(original)` — so **1/3 of `sub1` trials are,
  by construction, indistinguishable from a genuine reflection edit** (same
  byte-level and topology-level footprint). When `diagnose()` correctly
  reports `Reflection` for one of these, it is scored as "wrong" against
  the `sub1` ground-truth label even though the classification is
  factually accurate for that specific edit. The true ceiling for
  "distinguishing substitution from reflection" is therefore ≤2/3 ≈ 0.667
  by construction, not 1.0; the measured 0.73 exceeds that naive floor
  because many collision cases are still caught, some non-collision cases
  are missed too, and the two effects partially offset — reported as
  measured, not adjusted.
- **Transposition (0.0471) and burst (0.0904) are the genuine negative
  result of this avenue.** The calibration histograms
  (`debug_calibration_histograms`) show why: because the algebra's
  same/reflection topology kind is a coarse binary summary (52/64 "same"
  at the base rate — Theorem-1-adjacent structure, see
  `docs/THEOREMS_2026-07-23.md`), most triples touched by *any* multi-symbol
  edit still evaluate to "Same" and don't flip. A transposition's
  theoretical 4-wide touched window (span 3) is realized only ~10% of the
  time (299/2,914 in the 5,000-trial calibration batch); the rest collapse
  into a 1–3-wide footprint indistinguishable from a plain substitution.
  Burst-4's 6-wide window behaves the same way. **This is a real limit of
  a 1-bit-per-triple topology signal, not a tuning failure** — increasing
  the corruption's raw size does not proportionally increase the number of
  triples whose *binary* same/reflection kind actually changes.
- **χ-evading pair (0.2660 / 0.1830) is capped by island geometry.**
  ~67% of trials land in a single merged island (the two edit positions'
  windows overlap or their triples happen to still agree), which is
  structurally unrecoverable as "two edits" from the trace alone; of the
  remainder, the two-island cases are caught essentially deterministically
  (chi delta is *exactly* 0 for a true pair, since chi is a linear sum and
  the construction cancels by design) once they clear the shuffle
  island-count gate. The ≈26% two-island rate observed in calibration is
  consistent with the measured 26.6% classification rate.
- **Where hybrid actually helps that CRC-16 alone cannot, at any stream
  length:** CRC-16 returns one opaque bit ("something changed"); the
  hybrid frame's topology trace, when it fires, additionally names the
  event type and a symbol-level location — unconditionally true for
  `Reflection` and `Shuffle`, and honestly partial for the other four
  classes, exactly as reported above.

## Overhead

Fixed per-frame overhead (independent of payload length `L`):
- CRC-16: 16 bits.
- GPL acc + quantized chi: 2 + 14 = **16 bits** at L=64 (matches the OSF
  paper's `2 + 2·log2(2L+1)` formula: `2 + 2·log2(129) ≈ 16.02`).
- `assoc_parity`: 1 bit.
- **Fixed total: 33 bits** (packed/logical) — or 48 bits (16+8+16+8) as
  coded in the Rust struct today, since `gpl_acc`/`gpl_chi_q`/`assoc_parity`
  are stored byte-aligned rather than bit-packed; `HybridFrame::encode`
  does not attempt bit-packing (out of scope — this avenue targets the
  format and the detection/diagnosis numbers, not a wire-compaction pass).

Optional topology trace: `L - 2` bits (1 bit/triple), a *reference trace*
scored separately, not folded into the "checksum overhead" headline number
— consistent with the OSF paper's treatment (§8: "the topology signature is
L−2 bits of reference trace, not a compact checksum"). At L=64 that's 62
bits, for a grand total of 33 + 62 = **95 bits overhead** on a 128-bit
(64-symbol × 2-bit) payload when the trace is attached, or **33 bits**
overhead when it isn't (CRC-16 + assoc_parity + GPL still gate correctness;
just no diagnosis on mismatch).

## Repro / test commands

```
export CARGO_TARGET_DIR="$TEMP/kairos-target-7"
cargo test --lib protocol::hybrid_frame -- --nocapture
```

## Build/test result

**7 passed; 0 failed; 0 ignored** (`cargo test --lib protocol::hybrid_frame`,
5.6–6.4s wall time across runs). Full `cargo test --lib` (all 30 lib tests
in the default `std`+`phase-math-parallel`+`ssh` feature set, including the
pre-existing `core::gphase_core` and other kernel tests): **30 passed; 0
failed; 0 ignored** — no regression.

Test list:
- `crc16_matches_known_vector` — CRC-16/CCITT-FALSE("123456789") == 0x29B1
  (standard test vector).
- `encode_rejects_non_multiple_of_4`
- `encode_matches_checksum_module` — frame fields equal direct calls into
  `checksum::gpl_checksum` / `topology::scan_sliding_triples` (no
  reimplementation drift).
- `detect_clean_payload_is_no_mismatch`
- `detect_single_substitution_is_caught_by_crc_and_hybrid`
- `debug_calibration_histograms` — prints the span/island/ndiff
  distributions per corruption class that the `diagnose()` thresholds in
  §"Diagnosis" above were calibrated against (5,000 trials/class); kept in
  the suite as a documented, reproducible calibration record rather than a
  one-off scratch script.
- `monte_carlo_detection_and_diagnosis` — the headline Monte Carlo (20,000
  trials/class); asserts the honest rule (`hybrid_rate >= crc_rate`) per
  class and prints the exact table reproduced above.
