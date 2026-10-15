# KairOS Statechain Wire/Hash Format — Formal Spec

**Status:** reverse-engineered from source, not a design doc. Every claim below
cites the file/function it was read from. Source of truth remains the code;
this spec must be re-derived if any of the cited files change.

Sources: `src/kernel/statechain.rs`, `src/kernel/state_proof.rs`,
`src/kernel/state_root.rs`, `src/kernel/statechain_checkpoint.rs`,
`src/kernel/statechain_watchdog.rs`, `src/kernel/hash.rs`,
`src/kernel/scheduler.rs`, `src/kernel/kernel.rs`, `src/core/gphase_core.rs`.

---

## 1. Primitives

### 1.1 `StateHash`
`src/kernel/hash.rs`. `pub type StateHash = [u8; 32]` — a raw SHA-256 digest.

### 1.2 `tagged_hash(tag, parts)`
```
SHA256( tag.as_bytes() || parts[0] || parts[1] || ... || parts[n-1] )
```
Plain concatenation — **no length prefixes, no delimiters** between `tag` and
the parts, or between parts themselves (`src/kernel/hash.rs:7-14`). Domain
separation across the different hash families in this codebase comes only
from the leading tag string differing (`KAIROS_STATE_EVENT_V1`,
`KAIROS_PAYLOAD_V1`, `KAIROS_EQUATION_ROOT_V1`, `KAIROS_MERKLE_NODE_V1`,
`KAIROS_KERNEL_ROOT_V1`, `KAIROS_PRIME_LOG_V1`, `KAIROS_SIGNED_CHECKPOINT_V1`,
`KAIROS_STATECHAIN_SIGNING_KEY_V1`).

**Note (non-obvious):** because there are no length prefixes, two adjacent
*variable-length* parts fed to the same `tagged_hash` call would in principle
be ambiguous (`"ab"+"c"` vs `"a"+"bc"`). In `compute_event_hash` (below) the
one variable-length field, `operation_name`, is sandwiched between two
fixed-width 1-byte fields on either side, so it is not actually exploitable
in the current schema — but this is a property of field placement, not of
the hashing primitive, and would need re-checking if the schema changes.

### 1.3 `hash_pair(tag, left, right)`
`tagged_hash(tag, &[left, right])` — used only for Merkle nodes.

---

## 2. `StateTransitionProof` (`src/kernel/state_proof.rs:48-61`)

```rust
pub struct StateTransitionProof {
    pub object_kind: ObjectKind,          // 1 byte on the wire (enum as u8)
    pub object_id: u32,                   // 4 bytes LE
    pub sequence_number: u64,             // 8 bytes LE
    pub previous_state: PhaseState,       // 1 byte (0..=3)
    pub requested_state: PhaseState,      // 1 byte (0..=3)
    pub operation_name: String,           // variable-length UTF-8 bytes
    pub compose_result: Option<PhaseState>,
    pub topology_status: TopologyProofStatus,
    pub payload_hash: StateHash,          // 32 bytes — NOT the raw payload
    pub previous_event_hash: StateHash,   // 32 bytes — chain link
    pub equation_root: StateHash,         // 32 bytes
}
```

`ObjectKind` (`state_proof.rs:10-19`) is a fieldless enum; its `as u8` cast
uses Rust's default discriminant order: `Process=0, MemoryPage=1, File=2,
Directory=3, Block=4, Syscall=5, Vm=6, Driver=7`.

`TopologyProofStatus` (`state_proof.rs:21-46`) is derived from
`gphase_core::topology_status(a,b,c)` on the associator triple, but is
**not itself independently re-verified** by `verify_proof`/`ObjectStatechain::verify`
— it is only carried into the hash (§3) as opaque committed metadata:
`None=0, Same=1, Reflection=2`.

`compose_result: Option<PhaseState>` is also uncommitted-by-recomputation in
the chain-verify path: `verify_compose_result` exists
(`state_proof.rs:110-122`, checks `result == compose(left,right)`) but is
**not called** from `ObjectStatechain::append`/`verify`. It is only exercised
directly in unit tests. It is still hashed into `event_hash` (§3), so a
value the caller sends for it does become tamper-evident once committed —
it just isn't cross-checked for *correctness* against `compose()` inside the
chain integrity path itself.

### 2.1 Payload commitment
```rust
pub fn hash_payload(data: &[u8]) -> StateHash {
    tagged_hash("KAIROS_PAYLOAD_V1", &[data])
}
```
The chain **never stores the raw payload** — only `payload_hash`. Recovering
or auditing the original payload bytes requires an out-of-band record; the
statechain alone can only prove *a* payload matching the committed hash was
used, not exhibit it.

---

## 3. Event hash — `compute_event_hash` (`state_proof.rs:76-95`)

```rust
pub const EVENT_TAG: &str = "KAIROS_STATE_EVENT_V1";

compute_event_hash(proof, assoc_parity) =
  SHA256(
    EVENT_TAG
    || proof.equation_root                                    // 32 B
    || proof.previous_event_hash                               // 32 B
    || [proof.object_kind as u8]                                // 1 B
    || proof.object_id.to_le_bytes()                            // 4 B
    || proof.sequence_number.to_le_bytes()                      // 8 B
    || [proof.previous_state.as_u8()]                           // 1 B
    || [proof.requested_state.as_u8()]                          // 1 B
    || proof.operation_name.as_bytes()                          // variable
    || [proof.compose_result.map(as_u8).unwrap_or(255)]         // 1 B (255 = None sentinel)
    || [proof.topology_status.as_u8()]                          // 1 B (0/1/2)
    || [assoc_parity]                                            // 1 B — NOT a field of the proof struct itself
    || proof.payload_hash                                       // 32 B
  )
```

Key facts:
- **`assoc_parity` is not a field of `StateTransitionProof`.** It is passed
  as a separate argument to `compute_event_hash`/`verify_event_hash`/
  `ObjectStatechain::append`, and stored alongside the proof on `StateEvent`
  (§4). It is fully committed into the event hash (position: right before
  `payload_hash`), so any bit flip of the stored `assoc_parity` byte breaks
  the hash exactly like any other tampered field.
- `compose_result = None` is encoded as the sentinel byte `255`, which is
  outside the valid `PhaseState` range `0..=3`, so `None` cannot collide with
  a real `Some(state)` encoding.
- `verify_event_hash(proof, assoc_parity, event_hash)` (`state_proof.rs:124-130`)
  simply recomputes `compute_event_hash` and compares for equality — pure
  recomputation, no separate MAC/signature at the per-event level.

---

## 4. `StateEvent` (`src/kernel/statechain.rs:16-25`)

```rust
pub struct StateEvent {
    pub proof: StateTransitionProof,
    pub event_hash: StateHash,   // = compute_event_hash(proof, assoc_parity)
    pub tick: u64,               // ledger-global tick counter, not hashed
    pub subsystem: SubsystemKind,// not hashed (routing metadata only)
    pub assoc_parity: u8,        // hashed (see §3)
    pub accepted: bool,          // not hashed
    pub reason: String,          // not hashed
}
```

`tick`, `subsystem`, `accepted`, `reason` are **not** part of the hash input
in §3 — they ride alongside the proof for bookkeeping/UI/audit-report
purposes but are not cryptographically bound to `event_hash`. A tamperer who
can write these fields directly (i.e. anyone with the same in-process access
that a fuzz test has) can change them without invalidating `event_hash`. This
is a genuine, code-verified property, not a hash weakness in the append-only
path, since production code never re-serializes these fields into anything
audited elsewhere — noted here for completeness.

---

## 5. Chain linkage — `ObjectStatechain` (`statechain.rs:36-100`)

Each `(ObjectKind, object_id)` pair has its own append-only
`ObjectStatechain { object_kind, object_id, events: Vec<StateEvent> }`.

### 5.1 Append (`append`, lines 55-81)
```
expected_prev      = self.head().unwrap_or([0u8; 32])   // genesis = all-zero
proof.previous_event_hash = expected_prev                // caller's value is OVERWRITTEN
proof.sequence_number     = self.events.len() as u64      // caller's value is OVERWRITTEN
verify_proof(&proof, &expected_prev)?                     // sanity (see 5.3)
event_hash = compute_event_hash(&proof, assoc_parity)
verify_event_hash(...)?                                    // self-check, should never fail here
push StateEvent{ proof, event_hash, tick, subsystem, assoc_parity, accepted: true, reason: "" }
```
So on the honest append path, `previous_event_hash` and `sequence_number` are
**not attacker/caller-controlled** — they are derived from chain state at
append time. This matters for the fuzz harness: to construct an *invalid*
linkage you must mutate `events` directly after the fact (bypassing
`append`), which is exactly what tampering does.

### 5.2 Verify (`verify`, lines 83-99)
```
prev = [0u8; 32]
for (seq, event) in events.iter().enumerate():
    if event.proof.sequence_number != seq            -> Err(Reorder)
    if event.proof.previous_event_hash != prev        -> Err(Fork)
    verify_proof(&event.proof, &prev)?                 -> Err(Proof(_))
    if !verify_event_hash(&event.proof, event.assoc_parity, &event.event_hash)
                                                        -> Err(EventHash)
    prev = event.event_hash   // NB: takes the STORED hash, not a recomputed one
```
Important: `prev` advances using the **stored** `event.event_hash`, not a
freshly recomputed one. This is safe *because* the loop already rejected the
event at this index via `EventHash` if the stored hash didn't match its own
proof — verification short-circuits on the first bad event and never reaches
the next iteration with a poisoned `prev`.

### 5.3 `verify_proof` (`state_proof.rs:97-108`)
```
if proof.previous_event_hash != expected_prev_hash -> Err(PreviousHashMismatch)
if proof.equation_root == [0u8; 32]                 -> Err(EquationRootMismatch)
```
This is a narrow sanity check (redundant with the `Fork` check in §5.2 for
the linkage part); it does not verify `compose_result` or `topology_status`.

---

## 6. Ledger — `StatechainLedger` (`statechain.rs:102-329`)

- Holds `chains: HashMap<(ObjectKind, u32), ObjectStatechain>`, plus one
  `Vec<StateHash>` of latest heads per `SubsystemKind` (`process_heads`,
  `memory_heads`, `filesystem_heads`, `vm_heads`, `syscall_heads`,
  `driver_heads`) and a single scalar `kernel_root`.
- `equation_root`, `signing_key`, `prime_log` are computed once in `new()`
  from `compute_equation_root()` (§7) and are effectively constants for the
  process lifetime (deterministic function of `ALPHA`, `phi()`, and the
  compose/reflection/topology tables — see §7).
- **`assoc_parity` is a single kernel-wide bit**, not a per-object or
  per-triple value:
  ```rust
  pub fn assoc_parity(&self) -> u8 { (self.reflection_defects % 2) as u8 }
  ```
  `reflection_defects` is synced from `Scheduler::reflection_count()`
  (`src/kernel/scheduler.rs:105-107`, `sum` of `topology_events.len()` over
  all processes — i.e. a running count of Reflection-classified associator
  triples, per Theorem 1) via
  `self.statechain.sync_reflection_defects(self.scheduler.reflection_count())`
  called from `Kernel::record_process` (`src/kernel/kernel.rs:271-274`)
  **before** the transition for that tick is recorded. So the `assoc_parity`
  byte baked into a given `StateEvent` is "cumulative reflection-defect count
  so far, mod 2, as of just before this event" — confirmed by
  `tests/statechain_tests.rs::assoc_parity_flips_on_odd_defects_and_holds_on_even`.
- `record_transition` (`statechain.rs:255-292`): increments `tick`,
  builds a fresh `StateTransitionProof` (`previous_event_hash` and
  `sequence_number` are placeholders overwritten inside `append`, exactly as
  in §5.1), fetches `self.assoc_parity()`, calls `chain.append(...)`, then
  refreshes the relevant `SubsystemKind` head cache entry
  (`refresh_subsystem_head`, resizing the `Vec` with zero-hashes as needed),
  then calls `recompute_kernel_root()`.
  - **Caching caveat:** the per-subsystem head `Vec`s are updated *only*
    inside `record_transition`/`refresh_subsystem_head`. If an
    `ObjectStatechain`'s `events` are mutated directly (as the fuzz harness
    below does, since it operates on standalone chains, and as any in-process
    tamperer with field access could do to a live ledger's `chains` map)
    without going through `record_transition`, the cached head — and hence
    `kernel_root` — will **not** automatically reflect the mutation. This is
    a code-inspection observation, not something exercised by the fuzz tests
    below (which test at the `ObjectStatechain::verify()` layer, the layer
    `StatechainLedger::verify_all()` actually delegates to for `chains_ok`).

### 6.1 Kernel root (`state_root.rs:181-218`)
```
process_root   = merkle_root(process_heads)
memory_root    = merkle_root(memory_heads)
filesystem_root= merkle_root(filesystem_heads)
vm_root        = merkle_root(vm_heads)
syscall_root   = merkle_root(syscall_heads)
driver_root    = merkle_root(driver_heads)
kernel_root    = tagged_hash(KAIROS_KERNEL_ROOT_V1,
                    equation_root || process_root || memory_root ||
                    filesystem_root || vm_root || syscall_root || driver_root)
```
`merkle_root` (`state_root.rs:158-179`): standard binary Merkle tree over
`StateHash` leaves using `hash_pair(KAIROS_MERKLE_NODE_V1, left, right)`;
odd node at a level is paired with **itself** (duplicate-last rule), not
dropped. Empty leaf set maps to `tagged_hash(MERKLE_TAG, &[&[0u8;32]])`
(a fixed non-zero sentinel root), not `[0u8;32]`.

`heads_for_audit` (`statechain.rs:314-324`) filters out any all-zero
placeholder hashes before computing the Merkle root, so unused/never-written
`object_id` slots in a resized head `Vec` don't pollute the root.

---

## 7. Equation root (`state_root.rs:112-136`)

`compute_equation_root()` hashes, under tag `KAIROS_EQUATION_ROOT_V1`: the
literal equation string `"y^4 - phi*y^2 + alpha = 0"`, `phi()`, `ALPHA`,
`ln(phi)`, `ln(ALPHA)`, the four real roots of the quartic (`f64` little
-endian, 8 bytes each), `ln` of the two positive-`y²` roots, the full 4×4
`compose` table (`compose_table_bytes`, 16 bytes), the 4-entry reflection
rule (`reflection_rule_bytes`, `rho(x)=3-x`), and the full 64-entry
associator-topology table (`topology_table_bytes`, Same=0/Reflection=1 per
`(a,b,c)` triple). This root is what gates `verify_proof` (§5.3) — any proof
carrying a stale/foreign `equation_root` is rejected outright, and it is also
what every `StateEvent` hash is chained from (§3). It is a pure function of
constants in `gphase_core.rs` / `state_root.rs`; it is **not a secret**.

---

## 8. Checkpoint sealing (`statechain_checkpoint.rs`)

```
signing_key = tagged_hash(KAIROS_STATECHAIN_SIGNING_KEY_V1, &[equation_root])

checkpoint_body(seq, tick, kernel_root, equation_root, prime_log) =
  tagged_hash(KAIROS_SIGNED_CHECKPOINT_V1,
    seq.to_le_bytes() || tick.to_le_bytes() || kernel_root ||
    equation_root || prime_log)

signature = HMAC-SHA256(key = signing_key, message = checkpoint_body(...))

SignedCheckpoint { sequence, tick, kernel_root, equation_root, prime_log, signature }
```

`verify_checkpoint` recomputes the body, HMACs with the (recomputed)
`signing_key`, compares to the stored `signature`, and additionally rejects
an all-zero `kernel_root` or `equation_root`.

**Security note (honest, code-verified):** `derive_signing_key` computes the
"signing key" as a plain `tagged_hash` of the (public, deterministic)
`equation_root` — there is no external secret, nonce, or out-of-band key
material anywhere in this derivation. Since `equation_root` is reproducible
by anyone who can run `compute_equation_root()` (it depends only on public
constants `ALPHA`, `phi()`, and the compose/reflection/topology tables), the
HMAC here functions as a **domain-separated integrity re-hash**, not a
secret-keyed MAC in the cryptographic sense: it detects accidental
corruption and confirms internal self-consistency (the checkpoint really was
produced by *this* build's equation root), but it does not, by itself, resist
a forger who has read access to the source/binary, since they can rederive
`signing_key` exactly. `StatechainWatchdog::new()` independently recomputes
`equation_root`/`prime_log` from source (`statechain_watchdog.rs:34-42`) —
its guarantee is "this ledger's data is internally consistent with a rebuild
of the reference constants," not "this data was sealed by a party holding a
private key." This is a structural property of the current design, not a
bug found by the fuzz harness — flagged here because the roadmap's honesty
rule extends to design description, not just PASS/FAIL numbers.

---

## 9. `assoc_parity` end-to-end summary (new since last spec revision)

1. `Scheduler` tallies `topology_events` (Reflection-classified associator
   triples, per `gphase_core::topology_status`) per process as transitions
   happen (outside the files read for this spec, but its accumulator is read
   at `scheduler.rs:105-107`).
2. `Kernel::record_process` syncs the kernel-wide sum into
   `StatechainLedger.reflection_defects` **before** calling into the
   protected-object/masking path that eventually calls `record_transition`
   (`kernel.rs:271-274`).
3. `StatechainLedger::record_transition` reads `self.assoc_parity()`
   (`reflection_defects % 2`) and passes it into `chain.append(...)`
   (`statechain.rs:284-286`).
4. `ObjectStatechain::append` stores it verbatim on `StateEvent.assoc_parity`
   and folds it into `compute_event_hash` (§3) as a hashed, tamper-evident
   byte — flipping it, or flipping any bit of `event_hash` after the fact,
   is caught by `verify_event_hash` inside `ObjectStatechain::verify()`
   (§5.2), the mechanism the adversarial fuzz harness (§10 of the companion
   report, `docs/reports/AVENUE_06_statechain_fuzz.md`) exercises directly.

---

## 10. Field-by-field wire summary (event hash preimage, in order)

| # | Field | Size | Source |
|---|---|---|---|
| 0 | tag `"KAIROS_STATE_EVENT_V1"` | 22 B | constant |
| 1 | `equation_root` | 32 B | `StateTransitionProof.equation_root` |
| 2 | `previous_event_hash` | 32 B | chain-derived, not caller-set |
| 3 | `object_kind as u8` | 1 B | enum discriminant |
| 4 | `object_id` | 4 B LE | `u32` |
| 5 | `sequence_number` | 8 B LE | chain-derived, not caller-set |
| 6 | `previous_state.as_u8()` | 1 B | `0..=3` |
| 7 | `requested_state.as_u8()` | 1 B | `0..=3` |
| 8 | `operation_name` | variable | UTF-8 bytes, not length-prefixed |
| 9 | `compose_result` byte | 1 B | `Some(s)=s.as_u8()`, `None=255` |
| 10 | `topology_status.as_u8()` | 1 B | `None=0, Same=1, Reflection=2` |
| 11 | `assoc_parity` | 1 B | external arg, reflection-defect parity |
| 12 | `payload_hash` | 32 B | `hash_payload(raw_payload)`, not the raw payload |

Output: SHA-256 digest, 32 bytes, stored as `StateEvent.event_hash`.
