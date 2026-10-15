# PhaseSentinel signed onboard `app run` — audit

Audit-only. No code in this commit.
Branch: `feature/phasesentinel-signed-onboard-run-v1` from kairos `main` `09f120c`.
HID / checkpoint-auth working trees and the FullOS kairos pin are not touched.

## Dependency gate

| Tree | Tip | Signed GPA verify / install / run / revoke |
|------|-----|--------------------------------------------|
| FullOS `main` | `8cd10f1` pins kairos **`c9ef629`** | Missing from the pin |
| kairos `main` | **`09f120c`** | Present (COM12 PASS `e1d04c0`) |

Commits not in the FullOS pin (`c9ef629` → `09f120c`):

- `0756440` feat: PhaseStack signed GPA launch with public-pin-only Utility
- `e1d04c0` evidence: COM12 PASS for signed GPA launch on PZ7020
- `27c9ac9` docs: classify unsigned/over-capability as host tests, not UART
- `09f120c` merge: PhaseStack signed GPA verify/install/launch

This work does not reimplement PhaseStack, cherry-pick unknown commits, or bump the FullOS pin.

## Call graph (current)

```
shell parser
  → phasestack_cmd::dispatch_app "run"     userland/phasestack_cmd.rs
  → install::launch(kernel, app_id, fuel)  kernel/phasestack/install.rs
       → AppRegistry.get(app_id)
       → AppState match (Running is allowed)
       → revocation.check_package
       → fs.read_file(store/code.phase)
       → ph_code(store bytes) vs inst.package().code_hash()
       → PhaseAppLoader::load(&inst)       uses in-memory package.code()
       → Kernel::spawn_process(name)       Phi0  *** BEFORE Sentinel ***
       → #[cfg(phasesentinel)] authorize_package_sentinel
            → auto pin_root / install signer / allowed_signers.push
            → register_manifest(sentinel.psm1)
            → launch::authorize_launch     *** skips sentinel_authorize_launch ***
       → PhaseVmRuntime::execute / native
       → registry set Running + ledger Launch
  → shell prints "running pid=… output=… nonce=<package name>"
```

Verify / install (not run):

```
app verify  → load_staged → verifier::verify_package → VerifiedPackage
app install → install::install → re-verify → store lock → registry insert
app stop    → kill_process + state Stopped
app revoke  → stop + revoke_package + state Revoked
```

Legacy unsigned `run` / attach still uses `Kernel::sentinel_authorize_launch` (evidence + statechain). GPA `app run` does not.

## Type mapping (reuse; do not duplicate)

| Spec name | Existing type | Location |
|-----------|---------------|----------|
| PackageId | `AppId` = `StateHash` | `phasestack/types.rs` |
| package version | `AppVersion` | `types.rs` |
| KeyId / signer | `publisher_key_id: [u8; 32]` | `ExecEnvelope` |
| PhaseHash | `tuple_hash` / `ph_*` | `kernel/crypto/phase_hash.rs` |
| CapabilityMask | `Rights` / `GrantSet.caps` (`u32`) | `sentinel/types.rs`, `capability.rs` |
| MemoryEnvelope | `ExecEnvelope` (`mem_bytes`, `entry_point`) | `types.rs` |
| installed record | `VerifiedInstallation` | `types.rs`, `registry.rs` |
| payload hash of executed bytes | `VerifiedPackage::code_hash()` via `ph_code` | **not** `payload_root()` |
| `payload_root()` today | aliases `resource_root` (GPA merkle) | must not be used as executed-bytes binding |

`VerifiedLaunchTicket`, `SentinelLaunchPermit`, `SentinelLaunchDecision` do not exist.
Closest: `VerifiedPackage` (private fields, verifier-only mint), `SentinelDecision` {Allow, Refuse, Quarantine, Isolate}, `CachedLaunch`, `PhaseStackError::SentinelDenied`.

## Where signatures are verified

`verifier::verify_package`:

1. Canonical GPA parse (`parser.rs`)
2. Manifest / hooks / code entries
3. Developer cert chain (`cert.rs` `verify_chain`) against `ROOT_PUBLIC_KEY_PIN`
4. `RETIRED_RESEARCH_CA_PIN` refuse
5. Ed25519 `sign::verify(leaf_pk, package_root, package.sig)`
6. Grant subset: `grants.caps & !requested_caps == 0`

Board image: public pin only. Signing is hosted `phasestack-sign-dev` (`src/bin/phasestack_sign.rs`). No research CA seed in Utility.

## Registry

In-RAM `AppRegistry` (`BTreeMap<AppId, VerifiedInstallation>`). Store path `/phase/packages/{package_root_hex}/` with locked `manifest.phase`, `hooks.phase`, `code.phase`, optional `sentinel.psm1`. Root via `ph_app_registry`. Empty Gate K prefix `975cfcd8`. No `registry_generation` or `install_sequence` (`now_generation` is hardcoded `0`). No `MAX_APPS`.

## Granted capabilities

CLI `--grant-caps` → `GrantSet`. Checked at verify and `commit_installation`. Stored on `VerifiedInstallation.granted_caps`. Not applied as a live captable grant on spawn. Sentinel `policy.max_rights` defaults to `0` (`deny_all`).

## PhaseSentinel on `app run` today

On Utility (`phasesentinel` in `utility`) `authorize_package_sentinel` runs after spawn. It is not logging-only, but it is not a fail-closed independent gate:

- Auto-trusts the PhaseStack publisher (`pin_root` / `install` / `allowed_signers.push`)
- Skips evidence recording used by `sentinel_authorize_launch`
- Uses `chronos_tick` rather than `statechain.tick` for expiry
- Failure kills the already-created pid

Without the feature, `app run` bypasses Sentinel entirely (`#[cfg]`).

## Can `app run` bypass the verified registry?

Shell `app run` takes a 64-hex `AppId` and looks up the registry. It does not construct a ticket from public fields. It does not re-read the original GPA file. It can still launch `AppState::Running` again. `Verified` is also allowed. Spec requires exactly `Installed` (or Installed after stop with `pid == None`).

Name `hello-a7f3c91e` is not accepted; COM12 used the hex id. Physical plan wants the name. No `app registry` / `app status` verbs (`list` / `info` exist).

## Verify vs execute: same bytes?

Install copies GPA entries into the locked store and embeds `code` in `VerifiedPackage`. Launch re-hashes store `code.phase` against `code_hash()`, then executes **in-memory** `package.code()` via `PhaseAppLoader`. GPA file edits after install do not affect run. Store tamper → `LoadDenied` + Quarantine.

TOCTOU remaining: spawn before Sentinel; no ticket binding `ph_code(executed) == ticket.payload_root`; `payload_root()` is `resource_root`, not executed code.

## Revoke vs runnable process

`launch` returns `PhaseStackError::Revoked` if `AppState::Revoked`. Stop/revoke kill the pid and clear `process_id`. They do not invalidate Sentinel permits (none exist) or call `sentinel_contain_task`.

## Hosted / synthetic paths

Packing/signing: hosted SDK + `phasestack_sign`. Unit tests use synthetic seeds. Board path: SD GPA + PhaseVM. Native ELF execute returns `NativeIsolationUnavailable` / plugin path is out of scope. MMU sandbox is not present.

## Output / nonce

Hello program: Load Phi1, Print, Halt → `output=1`. Shell prints `nonce=hello-a7f3c91e` from the **package name**. That is not a cryptographic launch nonce and is not package-emitted. COM12 cannot distinguish shell labels from application output.

PhaseVM `Cpu.memory` is `Vec<u8>`. `Print` still emits `PhaseState` (0–3) into `Cpu.output`. Unique nonce-derived package bytes require a bounded emit of injected memory.

## Legal phase table

```
Phi0 → Phi1
Phi1 → Phi0, Phi2
Phi2 → Phi1, Phi3
Phi3 → Phi0, Phi1
```

`spawn_process` starts Phi0. Current `app run` does not `wake` / `schedule` / `syscall`. Stop via `kill_process` only if not already Phi0.

## Refusal codes today vs required

Present (`SentinelError`): Unauthorized, Unsigned, BadSignature, Revoked, Expired, Generation, RightsEscalation, Quarantined, Isolated, NoAnchor, NoCheckpoint, CheckpointInvalid, MmioDenied, PhaseIllegal.

Present (`PhaseStackError`): NotFound, Revoked, Quarantined, UnknownCert, RetiredSigner, HashMismatch, LoadDenied, GrantNotSubset, ExcessiveMemory, BadEntry, SentinelDenied, …

Missing structured launch reasons: RegistryMissing, NotInstalled, PackageRevoked (as Sentinel reason), PackageQuarantined, SignerUnknown, SignerRetired, ManifestRootMismatch, PayloadRootMismatch, RegistryGenerationMismatch, InstallSequenceMismatch, CapabilityEscalation, MemoryEnvelopeInvalid, EntryNotDeclared, NonceReplayed, PermitConsumed, KernelStateInvalid, ProcessAlreadyRunning.

UART today: `run failed: SentinelDenied`.

## Statechain

PhaseStack install/launch: `ObjectKind::App`, ops `phasestack_install` / ledger `LedgerKind::Launch`.
Sentinel seal exists only on `sentinel_authorize_launch`: `sentinel_evidence_checkpoint`. GPA path does not record SentinelLaunchRequested/Allowed/Refused/Isolated, ProcessSpawned, ApplicationOutput, ApplicationStopped, ApplicationRevoked.

## Host vs hardware already proven

- Gate K / signed GPA COM12: PhaseStack verify→install→run→revoke. Not a dedicated Sentinel gate.
- Gate 8 COM12: unsigned `run` refuse. Explicitly not signed onboard launch.
- Gates D–J host: PhaseStack. No Gate S.

## Gaps this branch must close

1. Construction-restricted `VerifiedLaunchTicket` minted only from PhaseStack live registry snapshot.
2. `SentinelLaunchDecision` with structured reasons; no fall-through to spawn.
3. One-use `SentinelLaunchPermit` + bounded consumed-nonce table.
4. Spawn **after** Allow; consume permit at spawn.
5. Remove auto-pin on the GPA path.
6. Bind executed bytes with `ph_code` as ticket payload root.
7. `registry_generation` / `install_sequence`; refuse stale tickets.
8. `ProcessAlreadyRunning`; launch only from Installed (pid None after stop).
9. Legal Phi0→Phi1→Phi2→Phi3→Phi0 on the real process.
10. Nonce-injected VM emit so `vm_output == launch_nonce`.
11. Stop/revoke: retire process, drop grants, invalidate permits, bump generation.
12. Statechain events on the package path.
13. Host Gates S0–S12; Utility `BAREMETAL_CROSS_BUILD`; COM12 plan without flash in this series.

## Explicit non-claims (unchanged)

Production CA, secure boot, MMU-enforced sandbox, native ELF, third-party ABI, physical key extraction, universal application security, all HID/network/storage paths, production readiness.
