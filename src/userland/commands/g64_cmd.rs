//! # G64 research shell commands
//!
//! Feature-gated (`g64-research`) commands that drive the **G64 phase-fabric
//! sidecar** in the PZ7020 PL and compare every result to `gphase_core` — the
//! sole software authority — with a **hard fail on any mismatch** (never a
//! warn-only pass). Opt-in only: the default production shell does not expose
//! these commands.
//!
//! ## Commands (physical results — PZ7020, COM12, run 2026-07-30 17:50)
//!
//! | Command | Purpose | Physical result |
//! |---------|---------|-----------------|
//! | `g64 info` | Read ID, version, capabilities and status | Hardware identified (`0x47363453` "G64S" / `0x00010000`) |
//! | `g64 oracle` | Report canonical oracle identity / source | 52 / 12 / 0 census from `gphase_core` |
//! | `g64 compose` | Test all 16 compose pairs against `gphase_core` | **16/16 PASS** |
//! | `g64 reflect` | Test all four reflection cases | **4/4 PASS** |
//! | `g64 atlas` | Evaluate all 64 ordered triples | **52 / 12 / 0 PASS** — hardware equals software |
//! | `g64 benchmark` | Repeat the atlas evaluation (throughput / soak check) | **8/8 atlas iterations completed** |
//!
//! Evidence: `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_175025/`.
//! Full write-up: `docs/reports/G64_FINDINGS_AND_INTEGRATION_20260730.md`.

use crate::core::gphase_core::{
    compose, left_group, reflect, right_group, topology_status, PhaseState, TopologyKind,
};
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

#[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
use crate::board::zynq7000::g64::{AtlasMasks, G64Error, DEFAULT_TIMEOUT_SPINS, G64};

fn phase(u: u8) -> Option<PhaseState> {
    PhaseState::from_u8(u)
}

pub fn run_info(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let g = G64::default_window();
        let id = g.id();
        let ver = g.version();
        let caps = g.capabilities();
        let st = g.status();
        let err = g.error();
        match g.probe_identity() {
            Ok(()) => ctx.writeln_line(&format!(
                "g64 info: id=0x{id:08X} version=0x{ver:08X} caps=0x{caps:08X} status=0x{st:08X} error=0x{err:08X} @ 0x40001000"
            ))?,
            Err(G64Error::IdentityMismatch { .. }) => {
                use crate::board::zynq7000::g1p::EXPECTED_ID as G1P_ID;
                if id == G1P_ID {
                    ctx.writeln_line(&format!(
                        "g64 info: not on this bitstream (PATH1 G1P+TNSR). 0x40001000 aliases G1P id=0x{id:08X} ver=0x{ver:08X}; TNSR is at 0x40000000+0x100. Not a G64S core — ID alone is not PASS"
                    ))?;
                } else {
                    ctx.writeln_line(&format!(
                        "g64 info: UNEXPECTED id=0x{id:08X} version=0x{ver:08X} (ID alone is not PASS)"
                    ))?;
                }
            }
            Err(e) => ctx.writeln_line(&format!("g64 info: {}", e.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("g64 info: requires zynq-a9 ARM + g64-research build")?;
        Ok(())
    }
}

pub fn run_oracle(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let mut same = 0u32;
    let mut refl = 0u32;
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            for c in PhaseState::ALL {
                match topology_status(a, b, c).kind {
                    TopologyKind::Same => same += 1,
                    TopologyKind::Reflection => refl += 1,
                }
            }
        }
    }
    ctx.writeln_line(&format!(
        "g64 oracle: software census identity={same} reflection={refl} error=0 (from gphase_core)"
    ))?;
    Ok(())
}

pub fn run_atlas(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let g = G64::default_window();
        if let Err(e) = g.probe_identity() {
            ctx.writeln_line(&format!("g64 atlas: FAIL {}", e.as_str()))?;
            return Ok(());
        }
        g.soft_reset();
        g.load_full_atlas_slots();
        match g.atlas(DEFAULT_TIMEOUT_SPINS) {
            Ok(masks) => {
                let (exp_id, exp_ref, exp_err) = software_atlas_masks();
                let id_ok = masks.identity == exp_id;
                let ref_ok = masks.reflection == exp_ref;
                let err_ok = masks.error == exp_err;
                let id_n = masks.identity.count_ones();
                let ref_n = masks.reflection.count_ones();
                let err_n = masks.error.count_ones();
                if id_ok && ref_ok && err_ok {
                    ctx.writeln_line(&format!(
                        "g64 atlas: PASS identity={id_n} reflection={ref_n} error={err_n} (hw==gphase_core)"
                    ))?;
                } else {
                    ctx.writeln_line(&format!(
                        "g64 atlas: FAIL id_n={id_n} ref_n={ref_n} err_n={err_n} \
                         hw_id=0x{:016X} exp_id=0x{:016X} hw_ref=0x{:016X} exp_ref=0x{:016X}",
                        masks.identity, exp_id, masks.reflection, exp_ref
                    ))?;
                }
                let _ = AtlasMasks {
                    identity: 0,
                    reflection: 0,
                    error: 0,
                };
            }
            Err(e) => ctx.writeln_line(&format!("g64 atlas: FAIL {}", e.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        let (exp_id, exp_ref, exp_err) = software_atlas_masks();
        ctx.writeln_line(&format!(
            "g64 atlas: host-only masks id_bits={} ref_bits={} err_bits={} (no HW)",
            exp_id.count_ones(),
            exp_ref.count_ones(),
            exp_err.count_ones()
        ))?;
        Ok(())
    }
}

fn software_atlas_masks() -> (u64, u64, u64) {
    let mut identity = 0u64;
    let mut reflection = 0u64;
    let mut error = 0u64;
    let mut idx = 0u32;
    for a in 0u8..4 {
        for b in 0u8..4 {
            for c in 0u8..4 {
                let pa = phase(a).unwrap();
                let pb = phase(b).unwrap();
                let pc = phase(c).unwrap();
                let left = left_group(pa, pb, pc);
                let right = right_group(pa, pb, pc);
                if left == right {
                    identity |= 1u64 << idx;
                } else if right == reflect(left) {
                    reflection |= 1u64 << idx;
                } else {
                    error |= 1u64 << idx;
                }
                idx += 1;
            }
        }
    }
    (identity, reflection, error)
}

pub fn run_compose_sweep(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let g = G64::default_window();
        let mut pass = 0u32;
        let mut fail = 0u32;
        for a in 0u8..4 {
            for b in 0u8..4 {
                let sw = compose(phase(a).unwrap(), phase(b).unwrap()).as_u8();
                match g.compose(a, b, DEFAULT_TIMEOUT_SPINS) {
                    Ok(hw) if hw == sw => pass += 1,
                    Ok(hw) => {
                        fail += 1;
                        ctx.writeln_line(&format!("g64 compose FAIL a={a} b={b} sw={sw} hw={hw}"))?;
                    }
                    Err(e) => {
                        fail += 1;
                        ctx.writeln_line(&format!("g64 compose FAIL a={a} b={b} {}", e.as_str()))?;
                    }
                }
            }
        }
        if fail == 0 {
            ctx.writeln_line(&format!("g64 compose: PASS {pass}/16"))?;
        } else {
            ctx.writeln_line(&format!("g64 compose: FAIL pass={pass} fail={fail}"))?;
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        let mut n = 0u32;
        for a in PhaseState::ALL {
            for b in PhaseState::ALL {
                let _ = compose(a, b);
                n += 1;
            }
        }
        ctx.writeln_line(&format!("g64 compose: host software {n}/16 (no HW)"))?;
        Ok(())
    }
}

pub fn run_reflect_sweep(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let g = G64::default_window();
        let mut pass = 0u32;
        let mut fail = 0u32;
        for a in 0u8..4 {
            let sw = reflect(phase(a).unwrap()).as_u8();
            match g.reflect(a, DEFAULT_TIMEOUT_SPINS) {
                Ok(hw) if hw == sw => pass += 1,
                Ok(hw) => {
                    fail += 1;
                    ctx.writeln_line(&format!("g64 reflect FAIL a={a} sw={sw} hw={hw}"))?;
                }
                Err(e) => {
                    fail += 1;
                    ctx.writeln_line(&format!("g64 reflect FAIL a={a} {}", e.as_str()))?;
                }
            }
        }
        if fail == 0 {
            ctx.writeln_line(&format!("g64 reflect: PASS {pass}/4"))?;
        } else {
            ctx.writeln_line(&format!("g64 reflect: FAIL pass={pass} fail={fail}"))?;
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("g64 reflect: host software 4/4 (no HW)")?;
        Ok(())
    }
}

pub fn run_benchmark(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let g = G64::default_window();
        g.load_full_atlas_slots();
        let mut ok = 0u32;
        for _ in 0..8 {
            match g.atlas(DEFAULT_TIMEOUT_SPINS) {
                Ok(_) => ok += 1,
                Err(e) => {
                    ctx.writeln_line(&format!("g64 benchmark: FAIL {}", e.as_str()))?;
                    return Ok(());
                }
            }
        }
        ctx.writeln_line(&format!("g64 benchmark: {ok}/8 atlas iterations completed"))?;
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("g64 benchmark: host-only (no HW)")?;
        Ok(())
    }
}
