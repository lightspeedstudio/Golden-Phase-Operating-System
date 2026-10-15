# Utility Patent — Claim Outline (Nonprovisional Draft Aid)

**Docket:** CAREGAGA-GPOS-2026-001  
**Inventor:** Richard Alberto Careaga  
**Related provisional draft:** `02_provisional_specification.md`

---

## Disclaimer

Informal claim outline for attorney drafting. **Not** final claims. Not legal advice. Proper claim language, antecedent basis, and §112 support must be prepared by counsel for nonprovisional filing within 12 months of provisional filing.

---

## Independent Claim Families

### Family 1 — Method: Dual Verify then Commit (primary)

**1.** A computer-implemented method for verified phase-state computation in an operating system, comprising:

(a) maintaining, in a kernel, a canonical four-state phase algebra defining a compose operation and a reflect operation;  
(b) receiving a request to perform a phase operation on one or more phase states;  
(c) computing a first result of the phase operation using software implementing said canonical algebra;  
(d) computing a second result of the phase operation using programmable logic configured to implement said phase operation;  
(e) comparing the first result and the second result for exact equality; and  
(f) treating the phase operation as verified only when the first result equals the second result.

**2.** The method of claim 1, wherein treating the phase operation as verified is a precondition to committing a corresponding kernel state transition.

**3.** The method of claim 1, wherein the programmable logic is accessed via memory-mapped I/O at a base address distinct from a second phase accelerator window.

**4.** The method of claim 3, wherein the programmable logic base address is `0x40001000` and the second phase accelerator window is at `0x40000000`.

**5.** The method of claim 1, wherein the phase operation is compose of two phase states.

**6.** The method of claim 1, wherein the phase operation is reflect of a phase state.

**7.** The method of claim 1, further comprising executing an atlas operation that evaluates all ordered triples of the four phase states and produces an identity mask, a reflection mask, and an error mask.

**8.** The method of claim 7, wherein a successful atlas self-test requires the error mask to be zero and identity/reflection counts to match a software census of said canonical algebra.

---

### Family 2 — System: Algebra-Root OS + Accelerators

**9.** A computing system comprising:

(a) a processor configured to execute a phase-kernel operating system that uses a canonical four-state phase algebra as a root of trust for state transitions;  
(b) a first hardware phase accelerator coupled to the processor; and  
(c) a second hardware phase-fabric engine coupled to the processor and configured to perform compose, reflect, and atlas operations;  
wherein the operating system is configured to compare a software phase result from the canonical algebra with a hardware phase result from at least one of the first accelerator and the second engine.

**10.** The system of claim 9, wherein the processor is an ARM Cortex-A9 within a Zynq-7000 SoC and the accelerators reside in programmable logic on an AXI interconnect.

**11.** The system of claim 9, further comprising a mission fault-detection layer that consumes phase-tagged subsystem states.

---

### Family 3 — Method: Kernel Algebra Root + Statechain

**12.** A computer-implemented method comprising:

(a) representing kernel objects with phase tags drawn from a four-state algebra;  
(b) permitting or rejecting state transitions according to legality under said algebra;  
(c) appending hashed transition events to a per-object statechain; and  
(d) aggregating subsystem integrity roots into a kernel state root.

**13.** The method of claim 12, further comprising replacing public disclosure of a protected object’s phase with a cryptographic commitment.

**14.** The method of claim 12, further comprising authorizing an external AI tool invocation only when a process phase equals a designated high-trust state and root authority is present.

---

### Family 4 — Apparatus: Atlas Integrity Signal

**15.** A hardware apparatus comprising programmable logic configured to:

(a) accept phase inputs over a memory-mapped interface;  
(b) compute compose and reflect results;  
(c) evaluate all ordered triples of a four-element phase set; and  
(d) output identity, reflection, and error masks usable by an operating system as an integrity signal.

---

### Family 5 — CRM

**16.** A non-transitory computer-readable medium storing instructions that, when executed by a processor, cause the processor to perform the method of any of claims 1–8 or 12–14.

---

## Embodiment support pointers (for §112)

| Element | Code / evidence |
|---------|-----------------|
| Canonical algebra | `src/core/gphase_core.rs` |
| G64 driver | `src/board/zynq7000/g64.rs` |
| G1P driver | `src/board/zynq7000/g1p.rs` |
| Address map | `src/board/zynq7000/memory_map.rs` |
| Physical atlas 52/12/0 | `fpga/zynq/evidence/.../run_20260730_175025/` |
| Spec narrative | `02_provisional_specification.md` |

---

*Attorney: rewrite into formal USPTO claim style; add means-plus-function care; align drawings.*
