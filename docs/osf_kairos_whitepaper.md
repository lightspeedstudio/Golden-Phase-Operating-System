# KairOS: A Phase-Kernel Operating System with Verifiable State, ANOM Privacy, and Operator-Grade Tooling

**Version:** 1.0 (post-build milestone)  
**Authors:** Richard Careaga, Metatron Research Project  
**Location:** Thousand Oaks, California, USA  
**License:** Research publication — see repository for code license  
**OSF posting note:** Publish after provisional patent filing. Attach this document, BUILD_SIZE_REPORT.txt, and selected test transcripts.

---

## Abstract

KairOS is a phase-kernel research operating system that treats a verified four-state phase algebra (Phi0–Phi3) as a first-class systems primitive. Process, memory, filesystem, communication, and operator actions are modeled as legal phase transitions recorded in a per-object statechain, aggregated into Merkle roots, and exposed through a unified operator shell. KairOS combines a hosted `std` runtime, a RISC-V baremetal boot target, ANOM privacy controls, PhaseLink proof-framed communication, PhaseNet native phase networking, an AI control layer with Phi3-gated tool authorization, a hosted SSH remote operator channel, and a multicall engineering terminal with 90+ authority-aware commands. Release hosted binaries measure approximately 4.2–4.8 MB on x86_64 Windows; the Rust source tree is approximately 1.2 MB across 278 files.

---

## 1. Introduction

Traditional operating systems represent internal state as opaque mutable structures. KairOS inverts part of that design: **transition legality and audit visibility are co-equal with execution**. Every meaningful state change can be recorded, hashed, checkpointed, and verified against Merkle commitments.

This paper describes the architecture as implemented in the KairOS codebase (kairos v0.1.0), not aspirational hardware claims.

---

## 2. Canonical Phase Core

The single mathematical authority is `core/gphase_core.rs`:

- Four states: Phi0, Phi1, Phi2, Phi3
- `compose`, `reflect`, topology (left/right grouping)
- Threshold and energy projections

No optimized backend may redefine these semantics. The Parallel Phase Math Engine provides batch interfaces, bounded priority lanes, object-sharded worker ownership, and optional accelerator backends while preserving scalar equivalence.

---

## 3. Integrity and Security

### 3.1 Statechain

Append-only per-object transition ledger with event hashing, checkpointing, and watchdog audit.

### 3.2 Merkle Roots

Subsystem heads (process, memory, filesystem, VM, syscall, driver, PhaseLink) roll into verifiable kernel root commitments.

### 3.3 ANOM

Global and per-object privacy: public commitments without exposing protected phase values, passphrases, or object data keys.

### 3.4 Boot and Proof Discipline

- BootSessionRoot binds boot life
- EquationRoot binds mathematical authority (not a secret key)
- PhaseLink quarantines malformed external frames before trusted acceptance

---

## 4. Communication Stack

| Layer | Role |
|-------|------|
| **PhaseNet** | Native phase-state networking (hosted) |
| **PhaseLink v1** | Byte↔phase codec, proof frames, loopback, two-node simulation |
| **SSH operator channel** | Ed25519 host keys, authorized_keys, session audit, default 127.0.0.1:2222 |
| **AI control layer** | TCP JSON bridge; jobs tracked in statechain; tools authorized at Phi3 |

---

## 5. Engineering Terminal (kairos-tool)

A unified multicall registry (`src/userland/tools/`) dispatches 90+ commands across 14 groups before legacy parser fallback:

| Group | Commands (sample) |
|-------|-------------------|
| Shell | help, clear, history, echo, which, version, exit |
| File | pwd, cd, ls, tree, mkdir, touch, cp, mv, rm, cat, head, tail, stat, find |
| Text | edit, grep, sort, uniq, wc, diff, patch |
| Build | build, test, check, format, lint (hosted) |
| Process | ps, run, kill, jobs, wait, sleep |
| System | status, mem, cpu, uptime, clock, trace, log, watchdog |
| Debug | hexdump, strings, checksum, size, symbols, objdump |
| Device | dev, irq, mmio read |
| Network | net, phaselink, phasenet, ping, connect, listen, send, receive, ssh |
| Phase | phase status/compose/reflect/transition/topology/chi/table |
| Security | authority, anom, statechain, audit, quarantine |
| Benchmark | bench, phi-stability, phaseperf, test-system |
| Script | script run, script check |

Authority tiers: Public, Viewer, Operator, Root. PhaseFS-backed paths; session state (cwd, history, env, edit buffer) on Hosted.

---

## 6. Deployment Targets

| Target | Status |
|--------|--------|
| Hosted runtime (kairos_host) | Verified — interactive shell, full feature set |
| SSH server (kairos_ssh) | Verified — 9/9 ssh_server_tests |
| Engineering tools smoke | Verified — 9/9 engineering_tools_tests |
| RISC-V baremetal (kairos_boot) | QEMU path exists; build has known phaselink_eth blockers |
| VirtIO / PHI-STABILITY smokes | Script-backed regression paths |

---

## 7. Build Footprint

See `BUILD_SIZE_REPORT.txt`:

- kairos_host.exe: ~4.15 MB (release, Windows)
- kairos_ssh.exe: ~4.61 MB
- Rust source + tests: ~1.16 MB

---

## 8. Verification

```text
cargo fmt --check
cargo test
cargo clippy -- -D warnings
cargo build --release --bin kairos_host
cargo build --release --bin kairos_ssh
powershell -ExecutionPolicy Bypass -File scripts\test-kairos-ssh.ps1
powershell -ExecutionPolicy Bypass -File scripts\boot-qemu-engineering-tools-smoke.ps1
```

32 test crates cover algebra, memory, process, statechain, StateClock, ANOM, object protection, PhaseNet, PhaseLink, phase math, PHI-STABILITY, SSH, engineering tools, terminal, VM, and boot.

---

## 9. Honest Boundaries

Not claimed as complete:

- Full Internet-scale distributed routing
- Production Ethernet / Wi-Fi / Bluetooth hardware drivers
- Production GPU/FPGA execution backends
- Mandatory custom hardware

Unsupported capabilities are reported as absent or hosted-only.

---

## 10. Conclusion

KairOS demonstrates that a phase-kernel OS with cryptographic audit, privacy-aware visibility, proof-framed networking, AI-gated tooling, remote SSH operations, and a full engineering terminal can be implemented in roughly one megabyte of Rust source and sub-five-megabyte hosted binaries — suitable for kernel research, security prototyping, and reproducible open science on OSF after patent priority is established.

---

## References

- KairOS repository: `kairos/` subtree of FullOS
- Golden Phase Runtime whitepaper (foundational algebra)
- USPTO provisional filing (Metatron Research Project) — cite application number after filing

## Data Availability

Recommended OSF bundle:

1. This whitepaper (PDF export)
2. BUILD_SIZE_REPORT.txt
3. Test transcripts: ssh_server_tests, engineering_tools_tests, phi_stability_tests
4. Architecture diagram export from kairos_whitepaper.html

---

*Metatron Research Project · Richard Careaga · Thousand Oaks, California*
