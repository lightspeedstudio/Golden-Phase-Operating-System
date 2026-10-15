//! `system selftest` — built-in full-suite self-test of every Golden Phase OS
//! system and subsystem. Runs each area's verify checks, prints PASS/FAIL/N-A to
//! the terminal, tallies the result, and (on the board) saves the full log to the
//! SD card as `SELFTEST.LOG` so every run leaves durable evidence.
//!
//! Each check reuses an existing verify function — it does not reimplement the
//! logic it is testing. Areas absent on this build (e.g. a std-only transport on
//! baremetal) report `N/A` and are named as build-out candidates.

use crate::platform::{String, Vec};
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

enum Status {
    Pass,
    Fail,
    Na,
}

struct Outcome {
    area: &'static str,
    status: Status,
    detail: String,
}

fn ok(area: &'static str, detail: String) -> Outcome {
    Outcome {
        area,
        status: Status::Pass,
        detail,
    }
}
fn bad(area: &'static str, detail: String) -> Outcome {
    Outcome {
        area,
        status: Status::Fail,
        detail,
    }
}
fn na(area: &'static str, detail: String) -> Outcome {
    Outcome {
        area,
        status: Status::Na,
        detail,
    }
}

/// Run the whole suite: header, per-area result, final tally, and (on the board)
/// persist the log to the SD card.
pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let mut r: Vec<Outcome> = Vec::new();

    // ── core: phase math / clock / phi-stability / state-domain ──
    r.push(check_phase_math());
    r.push(check_stateclock());
    r.push(check_phi_stability());
    r.push(check_phaseguard());
    r.push(check_state_domain());
    // ── feature areas ──
    r.push(check_statechain(ctx.kernel));
    r.push(check_crypto_primitives());
    r.push(check_anom_overlay(ctx.kernel));
    r.push(check_phaselink_phasenet());
    r.push(check_ssh());
    r.push(check_phasefs_ram(ctx.kernel));
    r.push(check_sd_fat());
    r.push(check_networking());
    r.push(check_usb());
    r.push(check_engineering());
    r.push(check_ai());
    r.push(check_ai_live());
    r.push(check_ai_control(ctx.kernel));
    r.push(check_phase_transport());
    r.push(check_isa_vm());
    r.push(check_fault_tolerance(ctx.kernel));
    r.push(check_memory_scheduler());

    let mut log = String::from("=== Golden Phase OS — system selftest ===\n");
    ctx.writeln_line("=== Golden Phase OS — system selftest ===")?;

    let mut passed = 0u32;
    let mut total = 0u32;
    for o in &r {
        let tag = match o.status {
            Status::Pass => "[ PASS ]",
            Status::Fail => "[ FAIL ]",
            Status::Na => "[ N/A  ]",
        };
        let line = format!("{} {}: {}", tag, o.area, o.detail);
        ctx.writeln_line(&line)?;
        log.push_str(&line);
        log.push('\n');
        if !matches!(o.status, Status::Na) {
            total += 1;
            if matches!(o.status, Status::Pass) {
                passed += 1;
            }
        }
    }

    let verdict = if passed == total {
        "ALL PASS"
    } else {
        "FAILURES PRESENT"
    };
    let summary = format!("system selftest: {passed}/{total} subsystems PASS — {verdict}");
    ctx.writeln_line(&summary)?;
    log.push_str(&summary);
    log.push('\n');

    // Persist the log to the SD card (board only; reuses the P1 filesystem).
    #[cfg(all(target_arch = "arm", feature = "baremetal"))]
    {
        let report = crate::userland::fs_sd::save("SELFTEST.LOG", log.as_bytes());
        ctx.writeln_line(&report)?;
    }

    Ok(())
}

// ── core checks ────────────────────────────────────────────────────────────

/// Phase Math — reflect is an involution and the runtime tables match the
/// canonical `gphase_core` algebra (compose + topology).
fn check_phase_math() -> Outcome {
    use crate::core::gphase_core::{self, PhaseState};
    let mut refl_ok = true;
    for p in PhaseState::ALL {
        if gphase_core::reflect(gphase_core::reflect(p)) != p {
            refl_ok = false;
        }
    }
    let tables = crate::kernel::phase_math::tables::PhaseMathTables::build();
    let tables_ok = crate::kernel::phase_math::reference::verify_tables(&tables);
    if refl_ok && tables_ok {
        ok(
            "Phase Math",
            "reflect involution + compose/topology tables match core".into(),
        )
    } else {
        bad(
            "Phase Math",
            format!("reflect_ok={refl_ok} tables_ok={tables_ok}"),
        )
    }
}

/// StateClock / Dual-Clock — chronos + kairos snapshot present and monotonic.
fn check_stateclock() -> Outcome {
    use crate::stateclock::StateClock;
    let c = StateClock::new();
    let s1 = c.snapshot();
    let s2 = c.snapshot();
    if s2.proper_tick >= s1.proper_tick && s2.kairos_tick >= s1.kairos_tick {
        ok(
            "StateClock / Dual-Clock",
            format!(
                "chronos={} kairos={} (dual-clock snapshot monotonic)",
                s2.proper_tick, s2.kairos_tick
            ),
        )
    } else {
        bad(
            "StateClock / Dual-Clock",
            "non-monotonic dual-clock snapshots".into(),
        )
    }
}

/// Phi-Stability — the trajectory-root construction is deterministic.
fn check_phi_stability() -> Outcome {
    use crate::kernel::phi_stability::proof::trajectory_root;
    let hs = [[3u8; 32], [5u8; 32], [8u8; 32]];
    let a = trajectory_root(&hs);
    let b = trajectory_root(&hs);
    if a == b {
        ok(
            "Phi-Stability",
            format!("trajectory_root deterministic {:02x}{:02x}", a[0], a[1]),
        )
    } else {
        bad("Phi-Stability", "trajectory_root non-deterministic".into())
    }
}

/// PhaseGuard (Gate-4 read-only observer) — verifies the squeeze predictor is
/// deterministic and 10-phase cyclic. Per docs/research/PHASEGUARD_KERNEL_INTEGRATION.md
/// the squeeze *law* itself is an open hypothesis (observed=NaN on live hardware),
/// so we test the observer math, not the physical law.
fn check_phaseguard() -> Outcome {
    #[cfg(feature = "phaseguard-research")]
    {
        use crate::stateclock::phase_disk::{
            phase_index_from_logical_tick, predict_squeeze_from_logical_tick,
        };
        let a = predict_squeeze_from_logical_tick(3);
        let b = predict_squeeze_from_logical_tick(3);
        let c = predict_squeeze_from_logical_tick(13); // +10 ticks → same phase index
        let diff = if a > c { a - c } else { c - a };
        let deterministic = a == b;
        let cyclic =
            diff < 1e-12 && phase_index_from_logical_tick(3) == phase_index_from_logical_tick(13);
        if deterministic && cyclic {
            ok(
                "PhaseGuard (observer)",
                "predictor deterministic + 10-phase cyclic (squeeze law = open hypothesis)".into(),
            )
        } else {
            bad(
                "PhaseGuard (observer)",
                format!("deterministic={deterministic} cyclic={cyclic}"),
            )
        }
    }
    #[cfg(not(feature = "phaseguard-research"))]
    {
        na(
            "PhaseGuard (observer)",
            "gated off (phaseguard-research feature)".into(),
        )
    }
}

/// State-Domain VM — the USB phase domain builds with its state lifecycle and a
/// stable domain root.
fn check_state_domain() -> Outcome {
    use crate::kernel::state_domain::build_usb_domain;
    let root = [0u8; 32];
    let d1 = build_usb_domain(&root);
    let d2 = build_usb_domain(&root);
    let n = d1.states.len();
    if n >= 4 && d1.domain_root == d2.domain_root {
        ok(
            "State-Domain VM",
            format!(
                "usb domain: {n} states, domain_root {:02x}{:02x} stable",
                d1.domain_root[0], d1.domain_root[1]
            ),
        )
    } else {
        bad(
            "State-Domain VM",
            format!(
                "states={n} root_stable={}",
                d1.domain_root == d2.domain_root
            ),
        )
    }
}

// ── feature-area checks ────────────────────────────────────────────────────

/// Statechain + Watchdog — run the real `StatechainWatchdog::audit` against the
/// live ledger: equation root, chain links, kernel root, and HMAC integrity checkpoints.
fn check_statechain(kernel: &crate::kernel::Kernel) -> Outcome {
    use crate::kernel::statechain_watchdog::StatechainWatchdog;
    let wd = StatechainWatchdog::new();
    let rep = wd.audit(&kernel.statechain);
    let detail = format!(
        "watchdog audit: equation={} chains={} kernel_root={} checkpoints={}",
        yn(rep.equation_ok),
        yn(rep.chains_ok),
        yn(rep.kernel_root_ok),
        yn(rep.signed_checkpoints_ok),
    );
    if !rep.tamper_detected {
        ok("Statechain + Watchdog", detail)
    } else {
        bad(
            "Statechain + Watchdog",
            format!("{detail} -> {}", rep.message),
        )
    }
}

#[inline]
fn yn(b: bool) -> &'static str {
    if b {
        "ok"
    } else {
        "FAIL"
    }
}

/// Crypto primitives — AES-256-GCM + ed25519 + tamper reject (not the ANOM overlay).
fn check_crypto_primitives() -> Outcome {
    #[cfg(feature = "baremetal")]
    {
        let report = crate::kernel::crypto::selftest();
        if report.contains("=> PASS") {
            ok("Crypto primitives", report)
        } else {
            bad("Crypto primitives", report)
        }
    }
    #[cfg(not(feature = "baremetal"))]
    {
        na(
            "Crypto primitives",
            "crypto selftest is baremetal-only (std build)".into(),
        )
    }
}

/// ANOM overlay — enter/status/exit (or already-active), fail-closed.
fn check_anom_overlay(kernel: &mut crate::kernel::Kernel) -> Outcome {
    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        use crate::kernel::anom::protocol;
        if !kernel.anom.is_crypto_available() {
            return bad(
                "ANOM overlay",
                "crypto not available (AnomStatus not Available/Armed/Active)".into(),
            );
        }
        if kernel.anom.is_active() {
            return ok(
                "ANOM overlay",
                "already active; crypto available (enter skipped to preserve session)".into(),
            );
        }
        const PASS: &[u8] = b"selftest-anom-overlay";
        match protocol::enter_anom_mode(kernel, PASS) {
            Ok(_) => {
                let active = kernel.anom.is_active();
                let exited = protocol::exit_anom_mode(kernel, PASS).is_ok();
                if active && exited {
                    ok(
                        "ANOM overlay",
                        "enter + active + exit OK (PhaseFS object store; not SD durable)".into(),
                    )
                } else {
                    bad(
                        "ANOM overlay",
                        format!("enter_active={active} exit_ok={exited}"),
                    )
                }
            }
            Err(e) => bad("ANOM overlay", format!("enter failed: {e:?}")),
        }
    }
    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    {
        let _ = kernel;
        na("ANOM overlay", "overlay requires std or baremetal".into())
    }
}

/// PhaseLink & PhaseNet — PhaseLink Merkle root deterministic over a fixed set.
fn check_phaselink_phasenet() -> Outcome {
    use crate::kernel::phaselink::merkle::phaselink_root;
    let leaves = [[7u8; 32], [9u8; 32]];
    let a = phaselink_root(&leaves);
    let b = phaselink_root(&leaves);
    if a == b {
        ok(
            "PhaseLink & PhaseNet",
            format!("phaselink_root deterministic {:02x}{:02x}", a[0], a[1]),
        )
    } else {
        bad(
            "PhaseLink & PhaseNet",
            "phaselink_root non-deterministic".into(),
        )
    }
}

/// Secure Remote Operation — the ed25519 host-key crypto the SSH server uses.
fn check_ssh() -> Outcome {
    #[cfg(feature = "baremetal")]
    {
        use crate::kernel::crypto::sign;
        use crate::kernel::hash::tagged_hash;
        let seed = tagged_hash("KAIROS_SELFTEST_SSH_HOSTKEY", &[b"seed"]);
        let kp = sign::SignKeypair::from_seed(&seed);
        let pk = kp.public_bytes();
        let msg = b"ssh host-key selftest";
        let sig = kp.sign(msg);
        if sign::verify(&pk, msg, &sig) && !sign::verify(&pk, b"tampered", &sig) {
            ok(
                "Secure Remote Op (ssh-bm)",
                "sunset/ssh-bm compiled; ed25519 host-key gen+sign/verify+tamper-reject; live serve is `ssh start` / `net ssh serve`".into(),
            )
        } else {
            bad(
                "Secure Remote Op (ssh-bm)",
                "host-key sign/verify failed".into(),
            )
        }
    }
    #[cfg(not(feature = "baremetal"))]
    {
        na(
            "Secure Remote Op (ssh-bm)",
            "ssh-bm host-key crypto (std build)".into(),
        )
    }
}

/// RAM PhaseFS — write/read round-trip (not SD durable).
fn check_phasefs_ram(kernel: &mut crate::kernel::Kernel) -> Outcome {
    let path = "/selftest-phasefs.txt";
    match kernel.fs.write_file(path, b"gpos-phasefs-ram") {
        Ok(()) => match kernel.fs.read_file(path) {
            Ok(data) if data.as_slice() == b"gpos-phasefs-ram" => ok(
                "PhaseFS (RAM)",
                "write+read round-trip on kernel.fs (not SD durable)".into(),
            ),
            Ok(data) => bad(
                "PhaseFS (RAM)",
                format!("mismatch len={}", data.len()),
            ),
            Err(e) => bad("PhaseFS (RAM)", format!("read failed: {e:?}")),
        },
        Err(e) => bad("PhaseFS (RAM)", format!("write failed: {e:?}")),
    }
}

/// SD FAT — persistent P1 path (board only).
fn check_sd_fat() -> Outcome {
    #[cfg(all(target_arch = "arm", feature = "baremetal"))]
    {
        let _ = crate::userland::fs_sd::save("STPROBE.TXT", b"gpos-selftest-probe");
        let read = crate::userland::fs_sd::cat("STPROBE.TXT");
        if read.contains("gpos-selftest-probe") {
            ok(
                "SD FAT",
                "SD write+read round-trip (persistent, not PhaseFS)".into(),
            )
        } else {
            bad("SD FAT", format!("SD round-trip mismatch: {read}"))
        }
    }
    #[cfg(not(all(target_arch = "arm", feature = "baremetal")))]
    {
        na(
            "SD FAT",
            "SD persistence is board-only (host build)".into(),
        )
    }
}

/// Networking (Ethernet) — GEM0 controller register presence.
fn check_networking() -> Outcome {
    #[cfg(all(target_arch = "arm", feature = "bm-net"))]
    {
        let (nwctrl, _b, _c) = crate::board::zynq7000::gem::probe_registers();
        if nwctrl != 0xFFFF_FFFF {
            ok(
                "Networking (Ethernet)",
                format!("GEM0 NWCTRL=0x{nwctrl:08x} (controller present)"),
            )
        } else {
            bad(
                "Networking (Ethernet)",
                "GEM0 NWCTRL=0xFFFFFFFF (absent?)".into(),
            )
        }
    }
    #[cfg(not(all(target_arch = "arm", feature = "bm-net")))]
    {
        na(
            "Networking (Ethernet)",
            "GEM0 is board-only (host build)".into(),
        )
    }
}

/// USB Host (HID) — the host controller brings up and reads the root port.
fn check_usb() -> Outcome {
    #[cfg(all(target_arch = "arm", feature = "usb"))]
    {
        let (report, st) = crate::board::zynq7000::usb::probe();
        if report.contains("host up") {
            let dev = if st.connected {
                format!("device {}", st.speed)
            } else {
                "no device".into()
            };
            ok(
                "USB Host (HID)",
                format!("host controller up @ 0xE0002000; {dev}"),
            )
        } else {
            bad("USB Host (HID)", report)
        }
    }
    #[cfg(not(all(target_arch = "arm", feature = "usb")))]
    {
        na(
            "USB Host (HID)",
            "USB host is board-only (host build)".into(),
        )
    }
}

/// Engineering Terminal — G1P phase engine sw-vs-hw compose equivalence.
fn check_engineering() -> Outcome {
    #[cfg(all(target_arch = "arm", feature = "zynq-a9"))]
    {
        use crate::board::zynq7000::g1p::{G1p, DEFAULT_TIMEOUT_SPINS};
        use crate::core::gphase_core::{self, PhaseState};
        let g = G1p::default_window();
        if g.probe_identity().is_err() {
            return bad("Engineering Terminal (G1P)", "G1P identity mismatch".into());
        }
        match g.exec(1, 2, 0, DEFAULT_TIMEOUT_SPINS) {
            Ok(res) => {
                let sw = gphase_core::compose(
                    PhaseState::from_u8(1).unwrap(),
                    PhaseState::from_u8(2).unwrap(),
                )
                .as_u8() as u32;
                if res.compose_ab == sw {
                    ok(
                        "Engineering Terminal (G1P)",
                        format!(
                            "G1P compose sw={sw} hw={} MATCH; devices uart0/gem0/g1p (no virtio)",
                            res.compose_ab
                        ),
                    )
                } else {
                    bad(
                        "Engineering Terminal (G1P)",
                        format!("G1P compose sw={sw} hw={} MISMATCH", res.compose_ab),
                    )
                }
            }
            Err(_) => bad("Engineering Terminal (G1P)", "G1P exec failed".into()),
        }
    }
    #[cfg(not(all(target_arch = "arm", feature = "zynq-a9")))]
    {
        na(
            "Engineering Terminal (G1P)",
            "G1P/G64 hardware is board-only (host build)".into(),
        )
    }
}

/// AI Control Layer — the portable protocol codec runs on baremetal; the smoltcp
/// transport (`ai ask`) is the on-board client. Verifies encode+decode round-trip.
fn check_ai() -> Outcome {
    use crate::kernel::ai::protocol::{decode_response, encode_request, AiRequestMessage};
    let req = AiRequestMessage {
        request_id: 7,
        process_id: 1,
        model: "default".into(),
        input: "phi".into(),
        input_hash: "ab".into(),
        allowed_tools: Vec::new(),
        deadline: 1000,
        current_phase: "Phi1".into(),
        parent_chain_hash: "00".into(),
    };
    let wire = encode_request(&req);
    let enc_ok = wire.contains("\"request_id\":7") && wire.contains("\"input\":\"phi\"");
    let resp = decode_response(
        "{\"request_id\":7,\"status\":\"ok\",\"output\":\"done\",\"output_hash\":\"91ac\",\"runtime\":5,\"error\":\"\",\"tool_requests\":[]}",
    );
    let dec_ok = matches!(&resp, Ok(r) if r.request_id == 7 && r.output == "done");
    if enc_ok && dec_ok {
        ok(
            "AI protocol codec",
            "JSON encode+decode round-trip (not a live model)".into(),
        )
    } else {
        bad(
            "AI protocol codec",
            format!("encode_ok={enc_ok} decode_ok={dec_ok}"),
        )
    }
}

fn check_ai_live() -> Outcome {
    na(
        "AI live client_bm",
        "outbound GEM TCP client; not the inbound control layer".into(),
    )
}

/// Inbound AI control: PhaseLink loopback job + Phi3 observe + deny + cancel.
fn check_ai_control(kernel: &mut crate::kernel::Kernel) -> Outcome {
    use crate::core::gphase_core::PhaseState;
    use crate::kernel::ai::control::{
        self, build_ai_frame, ingest_frame, parent_prefixed, OP_ACT, OP_CANCEL, OP_JOB,
    };
    use crate::kernel::ai::error::AiError;
    use crate::kernel::phaselink::FrameValidationResult;

    control::ensure_node(kernel);
    let job_frame = match build_ai_frame(
        kernel,
        OP_JOB,
        PhaseState::Phi0,
        PhaseState::Phi1,
        "new",
        b"selftest-intent",
    ) {
        Ok(f) => f,
        Err(e) => return bad("AI control layer", format!("AI_JOB build: {e:?}")),
    };
    match ingest_frame(kernel, &job_frame) {
        Ok(_) => {}
        Err(e) => return bad("AI control layer", format!("AI_JOB ingest: {}", e.reason())),
    }
    let job = match kernel.ai.jobs.last() {
        Some(j) => j,
        None => return bad("AI control layer", "no job after AI_JOB".into()),
    };
    if job.phase != PhaseState::Phi2 || job.input_hash == [0u8; 32] {
        return bad(
            "AI control layer",
            format!("job phase={:?} hash empty", job.phase),
        );
    }
    if kernel.scheduler.get_process(job.process_id).is_err() {
        return bad("AI control layer", "job process missing".into());
    }
    let job_id = job.job_id;
    let parent = job.parent_chain_hash;
    let pid = job.process_id;

    let illegal = match build_ai_frame(
        kernel,
        OP_ACT,
        PhaseState::Phi2,
        PhaseState::Phi0,
        &format!("{job_id}/system.version"),
        &parent_prefixed(&parent, b""),
    ) {
        Ok(f) => f,
        Err(e) => return bad("AI control layer", format!("illegal build: {e:?}")),
    };
    match crate::kernel::phaselink::accept_or_quarantine(kernel, &illegal) {
        FrameValidationResult::Quarantined(_) => {}
        other => {
            return bad(
                "AI control layer",
                format!("illegal Phi2->Phi0 not quarantined: {other:?}"),
            )
        }
    }

    let act = match build_ai_frame(
        kernel,
        OP_ACT,
        PhaseState::Phi2,
        PhaseState::Phi3,
        &format!("{job_id}/system.version"),
        &parent_prefixed(&parent, b""),
    ) {
        Ok(f) => f,
        Err(e) => return bad("AI control layer", format!("AI_ACT build: {e:?}")),
    };
    match ingest_frame(kernel, &act) {
        Ok(msg) if msg.contains("Golden Phase") || msg.contains("allowed") => {}
        Ok(msg) => return bad("AI control layer", format!("Phi3 observe unexpected: {msg}")),
        Err(e) => return bad("AI control layer", format!("Phi3 observe: {}", e.reason())),
    }

    let job2_frame = match build_ai_frame(
        kernel,
        OP_JOB,
        PhaseState::Phi0,
        PhaseState::Phi1,
        "new",
        b"selftest-deny",
    ) {
        Ok(f) => f,
        Err(e) => return bad("AI control layer", format!("deny-job build: {e:?}")),
    };
    if ingest_frame(kernel, &job2_frame).is_err() {
        return bad("AI control layer", "deny-job ingest failed".into());
    }
    let job2 = match kernel.ai.jobs.last() {
        Some(j) => j,
        None => return bad("AI control layer", "no deny job".into()),
    };
    let job2_id = job2.job_id;
    let parent2 = job2.parent_chain_hash;
    let deny = match build_ai_frame(
        kernel,
        OP_ACT,
        PhaseState::Phi2,
        PhaseState::Phi3,
        &format!("{job2_id}/shell.execute"),
        &parent_prefixed(&parent2, b"id"),
    ) {
        Ok(f) => f,
        Err(e) => return bad("AI control layer", format!("deny build: {e:?}")),
    };
    match ingest_frame(kernel, &deny) {
        Err(AiError::ToolDenied) => {}
        Ok(msg) => {
            return bad(
                "AI control layer",
                format!("shell.execute executed: {msg}"),
            )
        }
        Err(e) => {
            return bad(
                "AI control layer",
                format!("shell.execute wrong err: {}", e.reason()),
            )
        }
    }

    let cancel_frame = match build_ai_frame(
        kernel,
        OP_CANCEL,
        PhaseState::Phi1,
        PhaseState::Phi0,
        &format!("{job_id}"),
        b"",
    ) {
        Ok(f) => f,
        Err(e) => return bad("AI control layer", format!("AI_CANCEL build: {e:?}")),
    };
    match ingest_frame(kernel, &cancel_frame) {
        Ok(_) => {}
        Err(e) => return bad("AI control layer", format!("AI_CANCEL: {}", e.reason())),
    }
    let cancelled = kernel
        .ai
        .jobs
        .iter()
        .find(|j| j.job_id == job_id)
        .map(|j| j.status);
    if cancelled != Some(crate::kernel::ai::types::AiJobStatus::Cancelled) {
        return bad("AI control layer", "cancel did not isolate".into());
    }
    let after = match build_ai_frame(
        kernel,
        OP_ACT,
        PhaseState::Phi2,
        PhaseState::Phi3,
        &format!("{job_id}/system.version"),
        &parent_prefixed(&parent, b""),
    ) {
        Ok(f) => f,
        Err(e) => return bad("AI control layer", format!("post-cancel build: {e:?}")),
    };
    match ingest_frame(kernel, &after) {
        Err(AiError::Cancelled) => {}
        Ok(msg) => {
            return bad(
                "AI control layer",
                format!("act after cancel executed: {msg}"),
            )
        }
        Err(e) => {
            return bad(
                "AI control layer",
                format!("act after cancel wrong err: {}", e.reason()),
            )
        }
    }
    let _ = pid;
    ok(
        "AI control layer",
        "PhaseLink loopback: job process+hash, illegal hop quarantined, Phi3 observe allowed, shell.execute denied, cancel Phi0, further act refused".into(),
    )
}

/// Phase Transport — build a packet, verify its checksum, then tamper with the
/// payload and confirm the checksum verify rejects it.
fn check_phase_transport() -> Outcome {
    use crate::kernel::phase_transport::packet::{PacketOpcode, PhasePacket};
    let mut pkt = PhasePacket::new(
        1,
        (0, 0),
        (1, 1),
        PacketOpcode::Data,
        b"selftest-payload",
        0,
    );
    let good = pkt.verify_checksum().is_ok();
    pkt.payload[0] ^= 0xFF; // tamper
    let tamper_rejected = pkt.verify_checksum().is_err();
    if good && tamper_rejected {
        ok(
            "Phase Transport",
            "packet checksum verify OK + tamper rejected".into(),
        )
    } else {
        bad(
            "Phase Transport",
            format!("checksum_ok={good} tamper_rejected={tamper_rejected}"),
        )
    }
}

/// RISC-V ISA VM — load a tiny phase program, run it, and verify the results
/// against the canonical algebra.
fn check_isa_vm() -> Outcome {
    use crate::core::gphase_core::{self, PhaseState};
    use crate::vm::cpu::Cpu;
    use crate::vm::instruction::{Instruction, Register};

    // C = compose(A, B); D = reflect(A); then HALT.
    let program = [
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi1,
        },
        Instruction::Load {
            reg: Register::B,
            imm: PhaseState::Phi2,
        },
        Instruction::Compose {
            ra: Register::A,
            rb: Register::B,
            rc: Register::C,
        },
        Instruction::Reflect {
            ra: Register::A,
            rb: Register::D,
        },
        Instruction::Halt,
    ];
    let mut cpu = Cpu::new(256);
    let mut cycles = 0u32;
    while !cpu.halted && cycles < 64 {
        if cpu.step(&program).is_err() {
            break;
        }
        cycles += 1;
    }
    let want_c = gphase_core::compose(PhaseState::Phi1, PhaseState::Phi2);
    let want_d = gphase_core::reflect(PhaseState::Phi1);
    if cpu.halted && cpu.get(Register::C) == want_c && cpu.get(Register::D) == want_d {
        ok(
            "RISC-V ISA VM",
            format!(
                "load/compose/reflect/halt OK: C={} D={} ({} cycles)",
                cpu.get(Register::C).as_u8(),
                cpu.get(Register::D).as_u8(),
                cycles
            ),
        )
    } else {
        bad(
            "RISC-V ISA VM",
            format!(
                "halted={} C={} (want {}) D={} (want {})",
                cpu.halted,
                cpu.get(Register::C).as_u8(),
                want_c.as_u8(),
                cpu.get(Register::D).as_u8(),
                want_d.as_u8()
            ),
        )
    }
}

/// Fault Tolerance — live StateClock tick inject + snapshot restore on the
/// kernel clock. This is the PZ7020 `fault inject tick` path, not TMR/witness
/// vote and not the deleted synthetic copy/string "recovery".
fn check_fault_tolerance(kernel: &mut crate::kernel::Kernel) -> Outcome {
    #[cfg(feature = "phaseguard-research")]
    {
        use crate::kernel::fault_recovery::{inject_live, recover, FaultKind};
        let clock = kernel.get_clock_mut();
        let pre = clock.snapshot();
        inject_live(clock, FaultKind::TickSkip);
        let mut report = recover(clock, &pre);
        report.fault = FaultKind::TickSkip;
        if report.recovered {
            ok(
                "Fault Tolerance",
                format!(
                    "live StateClock tick inject+snapshot restore RECOVERED (not TMR vote); tau {} -> {} -> {}; continuation tick OK",
                    report.before.kairos_tick,
                    report.faulted.kairos_tick,
                    report.restored.kairos_tick
                ),
            )
        } else {
            bad(
                "Fault Tolerance",
                format!(
                    "live tick inject ISOLATED: changed={} restored==source={} reverify={} continuation={}",
                    report.state_changed,
                    report.restored_equals_source,
                    report.reverify_valid,
                    report.continuation_ok
                ),
            )
        }
    }
    #[cfg(not(feature = "phaseguard-research"))]
    {
        let _ = kernel;
        na(
            "Fault Tolerance",
            "live inject compiled out (phaseguard-research off)".into(),
        )
    }
}

/// Memory & Scheduler — heap allocator is operational (this Vec allocated).
fn check_memory_scheduler() -> Outcome {
    let probe: Vec<u8> = (0u8..32).collect();
    if probe.len() == 32 && probe[31] == 31 {
        ok(
            "Memory & Scheduler",
            "heap allocator operational (dynamic alloc OK)".into(),
        )
    } else {
        bad("Memory & Scheduler", "heap allocation anomaly".into())
    }
}
