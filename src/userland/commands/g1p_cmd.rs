//! G1P shell commands — polled hardware path on PZ7020 (+ IRQ test when GIC armed).

use crate::core::gphase_core::{self, PhaseState};
use crate::kernel::phase_consensus::{evaluate_lockstep, ConsensusMode};
#[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
use crate::kernel::phase_consensus::EmulatedAccelerator;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

#[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
use crate::board::zynq7000::g1p::{G1p, G1pError, DEFAULT_TIMEOUT_SPINS};

#[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
use crate::board::zynq7000::gic;

#[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
use crate::platform::String;

pub fn run_info(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let g = G1p::default_window();
        let id = g.id();
        let ver = g.version();
        let caps = g.capabilities();
        let st = g.status();
        let pend = g.irq_pending();
        let en = g.irq_enable();
        match g.probe_identity() {
            Ok(()) => ctx.writeln_line(&format!(
                "g1p info: id=0x{id:08X} version=0x{ver:08X} caps=0x{caps:08X} status=0x{st:08X} irq_en=0x{en:X} irq_pend=0x{pend:X} irq_count={} @ 0x40000000",
                gic::irq_count()
            ))?,
            Err(G1pError::IdentityMismatch { .. }) => ctx.writeln_line(&format!(
                "g1p info: UNEXPECTED id=0x{id:08X} version=0x{ver:08X} caps=0x{caps:08X} status=0x{st:08X}"
            ))?,
            Err(err) => ctx.writeln_line(&format!("g1p info: {}", err.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("g1p info: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

pub fn run_exec(ctx: &mut ShellContext<'_>, a: u8, b: u8) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let Some(pa) = PhaseState::from_u8(a) else {
            ctx.writeln_line("g1p exec: a must be 0..3")?;
            return Ok(());
        };
        let Some(pb) = PhaseState::from_u8(b) else {
            ctx.writeln_line("g1p exec: b must be 0..3")?;
            return Ok(());
        };
        let sw = gphase_core::compose(pa, pb).as_u8();
        let g = G1p::default_window();
        match g.exec(a as u32, b as u32, 0, DEFAULT_TIMEOUT_SPINS) {
            Ok(r) => {
                let ok = r.compose_ab == sw as u32;
                ctx.writeln_line(&format!(
                    "g1p exec: a={a} b={b} sw_compose={sw} hw_compose={} status=0x{:08X} {}",
                    r.compose_ab,
                    r.status,
                    if ok { "MATCH" } else { "MISMATCH" }
                ))?;
            }
            Err(err) => ctx.writeln_line(&format!(
                "g1p exec: FAIL {} status=0x{:08X}",
                err.as_str(),
                g.status()
            ))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        let _ = (a, b);
        ctx.writeln_line("g1p exec: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

pub fn run_reflect(ctx: &mut ShellContext<'_>, a: u8) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let Some(pa) = PhaseState::from_u8(a) else {
            ctx.writeln_line("g1p reflect: a must be 0..3")?;
            return Ok(());
        };
        let sw = gphase_core::reflect(pa).as_u8();
        let g = G1p::default_window();
        match g.reflect(a as u32) {
            Ok(hw) => {
                let ok = hw == sw as u32;
                ctx.writeln_line(&format!(
                    "g1p reflect: a={a} sw_reflect={sw} hw_reflect={hw} {}",
                    if ok { "MATCH" } else { "MISMATCH" }
                ))?;
            }
            Err(err) => ctx.writeln_line(&format!(
                "g1p reflect: FAIL {} status=0x{:08X}",
                err.as_str(),
                g.status()
            ))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        let _ = a;
        ctx.writeln_line("g1p reflect: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

/// Software (gphase_core) vs hardware (G1P MMIO) equivalence. No IRQ claim.
pub fn run_selftest(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let g = G1p::default_window();
        if let Err(err) = g.probe_identity() {
            ctx.writeln_line(&format!("g1p selftest: FAIL identity ({})", err.as_str()))?;
            return Ok(());
        }

        let mut fails = 0u32;
        let mut checks = 0u32;

        for a in PhaseState::ALL {
            for b in PhaseState::ALL {
                checks += 1;
                let sw = gphase_core::compose(a, b).as_u8() as u32;
                match g.exec(a.as_u8() as u32, b.as_u8() as u32, 0, DEFAULT_TIMEOUT_SPINS) {
                    Ok(r) => {
                        ctx.writeln_line(&format!(
                            "compose a={} b={}: sw={} hw={}",
                            a.as_u8(),
                            b.as_u8(),
                            sw,
                            r.compose_ab
                        ))?;
                        if r.compose_ab != sw {
                            fails += 1;
                        }
                    }
                    Err(err) => {
                        ctx.writeln_line(&format!(
                            "compose a={} b={}: hw FAIL {}",
                            a.as_u8(),
                            b.as_u8(),
                            err.as_str()
                        ))?;
                        fails += 1;
                    }
                }
            }
        }

        for a in PhaseState::ALL {
            checks += 1;
            let sw = gphase_core::reflect(a).as_u8() as u32;
            match g.reflect(a.as_u8() as u32) {
                Ok(hw) => {
                    ctx.writeln_line(&format!("reflect a={}: sw={} hw={}", a.as_u8(), sw, hw))?;
                    if hw != sw {
                        fails += 1;
                    }
                }
                Err(err) => {
                    ctx.writeln_line(&format!(
                        "reflect a={}: hw FAIL {}",
                        a.as_u8(),
                        err.as_str()
                    ))?;
                    fails += 1;
                }
            }
        }

        checks += 1;
        match g.exercise_invalid_input() {
            Ok(()) => ctx.writeln_line("invalid-input: hw latched input_error (ok)")?,
            Err(_) => {
                ctx.writeln_line("invalid-input: FAIL expected input_error")?;
                fails += 1;
            }
        }

        checks += 1;
        match g.exercise_invalid_opcode() {
            Ok(()) => ctx.writeln_line("invalid-opcode: hw latched opcode_error (ok)")?,
            Err(_) => {
                ctx.writeln_line("invalid-opcode: FAIL expected opcode_error")?;
                fails += 1;
            }
        }

        if fails == 0 {
            ctx.writeln_line(&format!(
                "g1p selftest: PASS checks={checks} (software vs hardware; polled; no IRQ claim)"
            ))?;
        } else {
            ctx.writeln_line(&format!(
                "g1p selftest: FAIL mismatches={fails}/{checks} (polled; no IRQ claim)"
            ))?;
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        let mut out = String::from("g1p selftest (host software only):\n");
        for a in PhaseState::ALL {
            for b in PhaseState::ALL {
                let c = gphase_core::compose(a, b).as_u8();
                out.push_str(&format!(
                    "  compose({},{}) -> {}\n",
                    a.as_u8(),
                    b.as_u8(),
                    c
                ));
            }
        }
        for a in PhaseState::ALL {
            out.push_str(&format!(
                "  reflect({}) -> {}\n",
                a.as_u8(),
                gphase_core::reflect(a).as_u8()
            ));
        }
        out.push_str("g1p selftest: HOST-OK (hardware MMIO still required on board)\n");
        ctx.writeln(&out)?;
        Ok(())
    }
}

/// 64-triple lock-step. Host is Emulated (never printed as hardware PASS).
pub fn run_consensus(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let g = G1p::default_window();
        return run_consensus_with(ctx, &g);
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        let accel = EmulatedAccelerator::new();
        run_consensus_with(ctx, &accel)
    }
}

fn run_consensus_with<A: crate::kernel::phase_consensus::PhaseAccelerator>(
    ctx: &mut ShellContext<'_>,
    accel: &A,
) -> Result<(), ShellError> {
    let mut pass = 0u32;
    let mut fail = 0u32;
    let mut same = 0u32;
    let mut reflection = 0u32;
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            for c in PhaseState::ALL {
                match evaluate_lockstep(accel, a, b, c) {
                    Ok(ok) => {
                        pass += 1;
                        match crate::core::gphase_core::topology_status(a, b, c).kind {
                            crate::core::gphase_core::TopologyKind::Same => same += 1,
                            crate::core::gphase_core::TopologyKind::Reflection => reflection += 1,
                        }
                        let _ = ok;
                    }
                    Err(err) => {
                        fail += 1;
                        ctx.writeln_line(&format!(
                            "g1p consensus: FAIL ({},{},{}) {}",
                            a.as_u8(),
                            b.as_u8(),
                            c.as_u8(),
                            err.as_str()
                        ))?;
                    }
                }
            }
        }
    }
    let mode = accel.mode();
    let label = match mode {
        ConsensusMode::Emulated => "EMULATED",
        ConsensusMode::Accelerated => "ACCELERATED",
    };
    if fail == 0 && same == 52 && reflection == 12 {
        if mode == ConsensusMode::Emulated {
            ctx.writeln_line(&format!(
                "g1p consensus: {label} 64/64 same=52 reflection=12 (not hardware)"
            ))?;
        } else {
            ctx.writeln_line(&format!(
                "g1p consensus: {label} 64/64 same=52 reflection=12"
            ))?;
        }
    } else {
        ctx.writeln_line(&format!(
            "g1p consensus: {label} FAIL pass={pass} fail={fail} same={same} reflection={reflection}"
        ))?;
    }
    Ok(())
}

/// Fire `count` G1P EXEC operations with IRQ enabled; compare irqreg slot 61.
pub fn run_irq_test(ctx: &mut ShellContext<'_>, count: u32) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let count = count.max(1);
        let g = G1p::default_window();

        g.set_irq_enable(false);
        g.clear_errors();
        g.set_irq_enable(true);

        let start = gic::irq_count();
        let expected = count;
        let mut exec_fails = 0u32;
        let mut missed = 0u32;
        for i in 0..count {
            let before = gic::irq_count();
            let a = (i % 4) as u32;
            let b = ((i + 1) % 4) as u32;
            if g.exec(a, b, 0, DEFAULT_TIMEOUT_SPINS).is_err() {
                exec_fails += 1;
            }
            let mut spins = 0u32;
            while gic::irq_count() <= before && spins < DEFAULT_TIMEOUT_SPINS {
                core::hint::spin_loop();
                spins += 1;
            }
            if gic::irq_count() <= before {
                missed += 1;
            }
        }

        let after = gic::irq_count();
        let observed = after.saturating_sub(start);
        g.set_irq_enable(false);
        g.clear_errors();

        let verdict = if exec_fails == 0 && missed == 0 && observed >= expected {
            "PASS"
        } else {
            "FAIL"
        };
        ctx.writeln_line(&format!(
            "g1p irq-test: expected={expected} observed={observed} slot61_before={start} slot61_after={after} exec_fails={exec_fails} missed={missed} {verdict}"
        ))?;
        // 7) return to gpos> via normal shell path
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        let _ = count;
        ctx.writeln_line("g1p irq-test: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}
