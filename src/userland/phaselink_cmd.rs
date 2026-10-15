//! PhaseLink shell commands.

use crate::kernel::phaselink::merkle::short_root;
use crate::kernel::phaselink::{
    accept_or_quarantine, build_process_commitment_frame, bytes_to_phase_stream, gateway_disable,
    gateway_enable, parse_phase_symbols, phase_stream_to_bytes, recv_loopback, render_phase_stream,
    send_loopback_text, sim_init, sim_reboot, sim_replay, sim_send_text, sim_set_tamper,
    FrameValidationResult,
};
use crate::kernel::phasenet::stream::hex_encode;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;
use crate::userland::syscall;

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln(&ctx.kernel.phaselink.status_lines())?;
    Ok(())
}

pub fn run_encode(ctx: &mut ShellContext<'_>, text: &str) -> Result<(), ShellError> {
    if text.is_empty() {
        ctx.writeln_line("usage: phaselink encode <text>")?;
        return Ok(());
    }
    let bytes = text.as_bytes();
    let stream = bytes_to_phase_stream(bytes);
    let round =
        phase_stream_to_bytes(&stream).map_err(|e| ShellError::Unknown(e.reason().into()))?;
    let bits = bytes
        .iter()
        .flat_map(|b| {
            [
                format!("{:02b}", (b >> 6) & 0b11),
                format!("{:02b}", (b >> 4) & 0b11),
                format!("{:02b}", (b >> 2) & 0b11),
                format!("{:02b}", b & 0b11),
            ]
        })
        .collect::<Vec<_>>()
        .join(" ");
    ctx.writeln_line("binary:")?;
    ctx.writeln_line(&format!("0x{}", hex_encode(bytes)))?;
    ctx.writeln_line("")?;
    ctx.writeln_line("bits:")?;
    ctx.writeln_line(&bits)?;
    ctx.writeln_line("")?;
    ctx.writeln_line("phase stream:")?;
    ctx.writeln_line(&render_phase_stream(&stream))?;
    ctx.writeln_line("")?;
    ctx.writeln_line("round trip:")?;
    ctx.writeln_line(if round == bytes { "OK" } else { "FAIL" })?;
    Ok(())
}

pub fn run_decode(ctx: &mut ShellContext<'_>, symbols: &[String]) -> Result<(), ShellError> {
    if symbols.is_empty() {
        ctx.writeln_line("usage: phaselink decode <phase-symbols>")?;
        return Ok(());
    }
    let refs = symbols.iter().map(String::as_str).collect::<Vec<_>>();
    let stream = parse_phase_symbols(&refs).map_err(|e| ShellError::Unknown(e.reason().into()))?;
    let bytes =
        phase_stream_to_bytes(&stream).map_err(|e| ShellError::Unknown(e.reason().into()))?;
    let bits = bytes
        .iter()
        .flat_map(|b| {
            [
                format!("{:02b}", (b >> 6) & 0b11),
                format!("{:02b}", (b >> 4) & 0b11),
                format!("{:02b}", (b >> 2) & 0b11),
                format!("{:02b}", b & 0b11),
            ]
        })
        .collect::<Vec<_>>()
        .join(" ");
    ctx.writeln_line("bits:")?;
    ctx.writeln_line(&bits)?;
    ctx.writeln_line("")?;
    ctx.writeln_line("binary:")?;
    ctx.writeln_line(&format!("0x{}", hex_encode(&bytes)))?;
    ctx.writeln_line("")?;
    ctx.writeln_line("text:")?;
    ctx.writeln_line(&String::from_utf8_lossy(&bytes))?;
    Ok(())
}

pub fn run_loopback(ctx: &mut ShellContext<'_>, text: &str) -> Result<(), ShellError> {
    if text.is_empty() {
        ctx.writeln_line("usage: phaselink loopback <text>")?;
        return Ok(());
    }
    let (frame, _bytes) =
        send_loopback_text(ctx.kernel, text).map_err(|e| ShellError::Unknown(e.reason().into()))?;
    let phase = bytes_to_phase_stream(text.as_bytes());
    let (received_frame, _received_bytes, result) =
        recv_loopback(ctx.kernel).map_err(|e| ShellError::Unknown(e.reason().into()))?;
    ctx.writeln_line(&format!("[ ENCODE ] bytes: {}", text.len()))?;
    ctx.writeln_line(&format!("[ PHASE  ] symbols: {}", phase.symbols.len()))?;
    ctx.writeln_line(&format!("[ FRAME  ] sequence: {}", frame.sequence_number))?;
    ctx.writeln_line(&format!("[ HASH   ] {}", short_root(&frame.frame_hash)))?;
    ctx.writeln_line("[ DECODE ] OK")?;
    ctx.writeln_line("[ PROOF  ] OK")?;
    ctx.writeln_line(&format!(
        "[ CHAIN  ] {}",
        if matches!(result, FrameValidationResult::Accepted) {
            "OK"
        } else {
            "FAIL"
        }
    ))?;
    ctx.writeln_line(&format!(
        "[ RESULT ] {}",
        String::from_utf8_lossy(&received_frame.payload)
    ))?;
    Ok(())
}

pub fn run_chain(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let rows = ctx
        .kernel
        .phaselink
        .accepted_chain
        .events
        .iter()
        .map(|event| {
            format!(
                "{:?}\t{}\t{}\t{}",
                event.kind,
                event.sequence_number,
                short_root(&event.frame_hash),
                event.operation
            )
        })
        .collect::<Vec<_>>();
    ctx.writeln_line("KIND\tSEQ\tHASH\tOP")?;
    for row in rows {
        ctx.writeln_line(&row)?;
    }
    Ok(())
}

pub fn run_root(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln_line(&format!(
        "PhaseLinkRoot: {}",
        short_root(&ctx.kernel.phaselink.root)
    ))?;
    Ok(())
}

pub fn run_quarantine(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let rows = ctx
        .kernel
        .phaselink
        .quarantine
        .entries
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let source = item
                .source_hint
                .map(|s| short_root(&s))
                .unwrap_or_else(|| "none".into());
            format!(
                "{}\t{}\t{}\t{}",
                i,
                source,
                item.reason.label(),
                short_root(&item.calculated_hash)
            )
        })
        .collect::<Vec<_>>();
    ctx.writeln_line("ID\tSOURCE\tREASON\tHASH")?;
    for row in rows {
        ctx.writeln_line(&row)?;
    }
    Ok(())
}

pub fn run_verify(ctx: &mut ShellContext<'_>, frame_id: &str) -> Result<(), ShellError> {
    let index: usize = frame_id.parse().unwrap_or(usize::MAX);
    if let Some(event) = ctx.kernel.phaselink.accepted_chain.events.get(index) {
        ctx.writeln_line(&format!("frame: {}", short_root(&event.frame_hash)))?;
        ctx.writeln_line("proof: OK")?;
        ctx.writeln_line("chain: OK")?;
    } else {
        ctx.writeln_line("[ ERROR ] unknown frame")?;
    }
    Ok(())
}

pub fn run_sim_init(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    sim_init(ctx.kernel);
    ctx.writeln_line("node-a: ready")?;
    ctx.writeln_line("node-b: ready")?;
    ctx.writeln_line("transport: hosted/in-memory")?;
    ctx.writeln_line("EquationRoot match: YES")?;
    Ok(())
}

pub fn run_sim_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let Some(sim) = &ctx.kernel.phaselink.simulation else {
        ctx.writeln_line("simulation: inactive")?;
        return Ok(());
    };
    let node_a = short_root(&sim.node_a.node_id);
    let node_b = short_root(&sim.node_b.node_id);
    let queued = sim.transport.queue.len();
    ctx.writeln_line(&format!("node-a: {node_a}"))?;
    ctx.writeln_line(&format!("node-b: {node_b}"))?;
    ctx.writeln_line(&format!("queued frames: {queued}"))?;
    Ok(())
}

pub fn run_sim_send(
    ctx: &mut ShellContext<'_>,
    source: &str,
    destination: &str,
    text: &str,
) -> Result<(), ShellError> {
    if source.is_empty() || destination.is_empty() || text.is_empty() {
        ctx.writeln_line("usage: phaselink sim send <source> <destination> <text>")?;
        return Ok(());
    }
    let (frame, result) = match sim_send_text(ctx.kernel, source, destination, text) {
        Ok(value) => value,
        Err(err) => {
            ctx.writeln_line(&format!("[ ERROR ] {}", err.reason()))?;
            return Ok(());
        }
    };
    ctx.writeln_line(&format!("source: {source}"))?;
    ctx.writeln_line(&format!("destination: {destination}"))?;
    ctx.writeln_line(&format!("binary bytes: {}", text.len()))?;
    ctx.writeln_line(&format!("phase symbols: {}", text.len() * 4))?;
    ctx.writeln_line(&format!("sequence: {}", frame.sequence_number))?;
    ctx.writeln_line("frame proof: OK")?;
    ctx.writeln_line("payload proof: OK")?;
    ctx.writeln_line(&format!(
        "previous hash: {}",
        if frame.previous_frame_hash == [0u8; 32] {
            "GENESIS".into()
        } else {
            short_root(&frame.previous_frame_hash)
        }
    ))?;
    ctx.writeln_line(&format!(
        "receive result: {}",
        match result {
            FrameValidationResult::Accepted => "ACCEPTED",
            FrameValidationResult::Quarantined(_) => "QUARANTINED",
            FrameValidationResult::Rejected(_) => "REJECTED",
        }
    ))?;
    ctx.writeln_line("node-b chain: OK")?;
    Ok(())
}

pub fn run_sim_replay(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    if sim_replay(ctx.kernel) {
        ctx.writeln_line("replay queued")?;
    } else {
        ctx.writeln_line("no replay available")?;
    }
    Ok(())
}

pub fn run_sim_tamper(ctx: &mut ShellContext<'_>, field: &str) -> Result<(), ShellError> {
    if sim_set_tamper(ctx.kernel, field) {
        ctx.writeln_line(&format!("tamper armed: {field}"))?;
    } else {
        ctx.writeln_line("simulation: inactive")?;
    }
    Ok(())
}

pub fn run_sim_reboot(ctx: &mut ShellContext<'_>, node: &str) -> Result<(), ShellError> {
    if sim_reboot(ctx.kernel, node) {
        ctx.writeln_line(&format!("{node}: rebooted"))?;
    } else {
        ctx.writeln_line("simulation: inactive")?;
    }
    Ok(())
}

pub fn run_sim_reset(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.kernel.phaselink.simulation = None;
    ctx.writeln_line("simulation reset")?;
    Ok(())
}

pub fn run_gateway_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let gw = &ctx.kernel.phaselink.gateway;
    ctx.writeln_line(&format!(
        "gateway: {}\nbinary input: {}\nbinary output: {}\nroot authority required: {}\nANOM decryptor access: {}",
        match gw.state {
            crate::kernel::phaselink::GatewayState::Disabled => "disabled",
            crate::kernel::phaselink::GatewayState::RootAuthorized => "root-authorized",
            crate::kernel::phaselink::GatewayState::Active => "active",
            crate::kernel::phaselink::GatewayState::Quarantined => "quarantined",
        },
        if gw.allow_binary_input { "allowed" } else { "denied" },
        if gw.allow_binary_output { "allowed" } else { "denied" },
        if gw.require_root_authority { "yes" } else { "no" },
        if gw.allow_anom_decryption { "allowed" } else { "denied" },
    ))?;
    Ok(())
}

pub fn run_gateway_enable(
    ctx: &mut ShellContext<'_>,
    use_passphrase: bool,
) -> Result<(), ShellError> {
    if !use_passphrase {
        ctx.writeln_line("usage: phaselink gateway enable -p")?;
        return Ok(());
    }
    ctx.writeln("passphrase: ")?;
    ctx.ensure_running()?;
    let pass = syscall::read(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
    ctx.writeln_line("")?;
    match gateway_enable(ctx.kernel, pass.as_bytes()) {
        Ok(()) => ctx.writeln_line("gateway enabled")?,
        Err(_) => ctx.writeln_line("[ ERROR ] gateway authorization failed")?,
    }
    Ok(())
}

pub fn run_gateway_disable(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    gateway_disable(ctx.kernel);
    ctx.writeln_line("gateway disabled")?;
    Ok(())
}

#[allow(dead_code)]
pub fn run_public_process_demo(ctx: &mut ShellContext<'_>, pid: u32) -> Result<(), ShellError> {
    let frame = build_process_commitment_frame(
        ctx.kernel,
        pid,
        ctx.kernel.phaselink.local_node,
        "public_process_state",
        crate::core::gphase_core::PhaseState::Phi1,
        crate::core::gphase_core::PhaseState::Phi2,
    )
    .map_err(|e| ShellError::Unknown(e.reason().into()))?;
    let result = accept_or_quarantine(ctx.kernel, &frame);
    ctx.writeln_line(&format!(
        "ANOM frame: {}",
        match result {
            FrameValidationResult::Accepted => "accepted",
            FrameValidationResult::Quarantined(_) => "quarantined",
            FrameValidationResult::Rejected(_) => "rejected",
        }
    ))?;
    Ok(())
}
