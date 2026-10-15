//! PhaseNet shell commands.

use crate::core::gphase_core::PhaseState;
use crate::kernel::phasenet::addr::{addr_for_uri, compute_phase_addr};
use crate::kernel::phasenet::visualize::render_packet_view;
use crate::kernel::phasenet::{
    decode_bytes, encode_bytes, hex_decode, hex_encode, parse_phase_uri, render_recv_line,
    render_status, render_stream, render_uri_addr,
};
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln(&render_status(&ctx.kernel.phasenet))?;
    Ok(())
}

pub fn run_stream_encode(ctx: &mut ShellContext<'_>, text: &str) -> Result<(), ShellError> {
    if text.is_empty() {
        ctx.writeln_line("usage: phasenet stream encode <text>")?;
        return Ok(());
    }
    let stream = encode_bytes(text.as_bytes());
    ctx.writeln_line(&format!("bytes: {}", hex_encode(text.as_bytes())))?;
    ctx.writeln_line(&format!("stream: {}", render_stream(&stream)))?;
    Ok(())
}

pub fn run_stream_decode(ctx: &mut ShellContext<'_>, hex: &str) -> Result<(), ShellError> {
    if hex.is_empty() {
        ctx.writeln_line("usage: phasenet stream decode <hex>")?;
        return Ok(());
    }
    let bytes = hex_decode(hex).map_err(|e| ShellError::Unknown(e.reason().into()))?;
    let stream = encode_bytes(&bytes);
    let recovered = decode_bytes(&stream).map_err(|e| ShellError::Unknown(format!("{e}")))?;
    ctx.writeln_line(&format!("stream: {}", render_stream(&stream)))?;
    ctx.writeln_line(&format!("bytes: {}", hex_encode(&recovered)))?;
    Ok(())
}

pub fn run_addr(ctx: &mut ShellContext<'_>, uri_text: &str) -> Result<(), ShellError> {
    if uri_text.is_empty() {
        ctx.writeln_line("usage: phasenet addr <phase://domain/path>")?;
        return Ok(());
    }
    let Some(uri) = parse_phase_uri(uri_text) else {
        ctx.writeln_line("invalid phase uri")?;
        return Ok(());
    };
    let addr = addr_for_uri(
        &ctx.kernel.phasenet.equation_root,
        &ctx.kernel.phasenet.node_root,
        &uri,
        0,
        0,
    );
    ctx.writeln(&render_uri_addr(&uri, &addr))?;
    Ok(())
}

pub fn run_loopback_send(
    ctx: &mut ShellContext<'_>,
    uri: &str,
    operation: &str,
    old: &str,
    new: &str,
) -> Result<(), ShellError> {
    if uri.is_empty() || operation.is_empty() {
        ctx.writeln_line("usage: phasenet loopback send <uri> <operation> <old_phi> <new_phi>")?;
        return Ok(());
    }
    let Some(old_phase) = parse_phi(old) else {
        ctx.writeln_line("invalid old Kairos State (Kairos0..Kairos3)")?;
        return Ok(());
    };
    let Some(new_phase) = parse_phi(new) else {
        ctx.writeln_line("invalid new Kairos State (Kairos0..Kairos3)")?;
        return Ok(());
    };
    let snapshot = ctx.kernel.clock.snapshot();
    {
        match ctx.kernel.phasenet.loopback_send_state_update(
            uri,
            operation,
            old_phase,
            new_phase,
            snapshot,
            &mut ctx.kernel.statechain,
        ) {
            Ok(hash) => {
                #[cfg(feature = "phasesentinel")]
                {
                    use crate::kernel::phasenet::PhaseNetVerdict;
                    let verdict = ctx
                        .kernel
                        .phasenet
                        .rx_queue
                        .last()
                        .map(|rx| match rx.verdict {
                            PhaseNetVerdict::Accept => 0,
                            PhaseNetVerdict::Quarantine => 1,
                            PhaseNetVerdict::Drop => 2,
                        })
                        .unwrap_or(2);
                    ctx.kernel.sentinel_observe_phasenet(ctx.caller, verdict);
                }
                ctx.writeln_line(&format!(
                    "loopback sent frame={:02x}{:02x}{:02x}{:02x}",
                    hash[0], hash[1], hash[2], hash[3],
                ))?;
                ctx.writeln_line("chain: OK")?;
            }
            Err(err) => ctx.writeln_line(&format!("[ ERROR ] {}", err.reason()))?,
        }
    }
    Ok(())
}

pub fn run_loopback_recv(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    {
        let anom = ctx.kernel.authority.is_anom_protected();
        if let Some(rx) = ctx.kernel.phasenet.loopback_recv() {
            let packet = rx.frame.packet().ok();
            ctx.writeln(&render_recv_line(
                rx.verdict,
                &rx.failures,
                packet.as_ref(),
                anom,
            ))?;
        } else {
            ctx.writeln_line("no pending loopback frames")?;
        }
    }
    Ok(())
}

pub fn run_sim_init(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    {
        let boot_b = compute_phase_addr(
            &ctx.kernel.phasenet.equation_root,
            &ctx.kernel.phasenet.boot_session_root,
            "sim",
            0,
            2,
            "node-b-boot",
        );
        ctx.kernel.phasenet.sim_init(boot_b);
        ctx.writeln_line("phasenet sim: node-a node-b ready")?;
    }
    Ok(())
}

pub fn run_sim_send(
    ctx: &mut ShellContext<'_>,
    from: &str,
    to: &str,
    uri: &str,
    operation: &str,
    old: &str,
    new: &str,
) -> Result<(), ShellError> {
    if from.is_empty() || to.is_empty() || uri.is_empty() {
        ctx.writeln_line(
            "usage: phasenet sim send <from> <to> <uri> <operation> <old_phi> <new_phi>",
        )?;
        return Ok(());
    }
    let Some(old_phase) = parse_phi(old) else {
        ctx.writeln_line("invalid old phase")?;
        return Ok(());
    };
    let Some(new_phase) = parse_phi(new) else {
        ctx.writeln_line("invalid new phase")?;
        return Ok(());
    };
    let snapshot = ctx.kernel.clock.snapshot();
    match ctx
        .kernel
        .phasenet
        .sim_send(from, to, uri, operation, old_phase, new_phase, snapshot)
    {
        Ok(hash) => ctx.writeln_line(&format!(
            "sim sent frame={:02x}{:02x}{:02x}{:02x}",
            hash[0], hash[1], hash[2], hash[3],
        ))?,
        Err(err) => ctx.writeln_line(&format!("[ ERROR ] {}", err.reason()))?,
    }
    Ok(())
}

pub fn run_sim_recv(ctx: &mut ShellContext<'_>, node: &str) -> Result<(), ShellError> {
    if node.is_empty() {
        ctx.writeln_line("usage: phasenet sim recv <node-a|node-b>")?;
        return Ok(());
    }
    {
        let anom = ctx.kernel.authority.is_anom_protected();
        if let Some(rx) = ctx.kernel.phasenet.sim_recv(node) {
            let packet = rx.frame.packet().ok();
            ctx.writeln(&format!("from: {}\n", rx.from))?;
            ctx.writeln(&render_recv_line(rx.verdict, &[], packet.as_ref(), anom))?;
        } else {
            ctx.writeln_line("no pending sim frames")?;
        }
    }
    Ok(())
}

pub fn run_view(ctx: &mut ShellContext<'_>, uri_text: &str) -> Result<(), ShellError> {
    if uri_text.is_empty() {
        ctx.writeln_line("usage: phasenet view <phase://domain/path>")?;
        return Ok(());
    }
    let Some(uri) = parse_phase_uri(uri_text) else {
        ctx.writeln_line("invalid phase uri")?;
        return Ok(());
    };
    let addr = addr_for_uri(
        &ctx.kernel.phasenet.equation_root,
        &ctx.kernel.phasenet.node_root,
        &uri,
        0,
        0,
    );
    ctx.writeln(&render_uri_addr(&uri, &addr))?;
    ctx.writeln_line(&format!("mode: {:?}", ctx.kernel.phasenet.mode))?;
    let snapshot = ctx.kernel.clock.snapshot();
    {
        let packet = crate::kernel::phasenet::packet::PhasePacket::state_update(
            ctx.kernel.phasenet.equation_root,
            &ctx.kernel.phasenet.node_name,
            &uri.domain,
            &uri.domain,
            "view",
            PhaseState::Phi1,
            PhaseState::Phi2,
            snapshot,
            [0u8; 32],
        );
        let anom = ctx.kernel.authority.is_anom_protected();
        ctx.writeln(&render_packet_view(&packet, anom))?;
    }
    Ok(())
}

fn parse_phi(token: &str) -> Option<PhaseState> {
    match token {
        "Phi0" | "phi0" | "Kairos0" | "kairos0" | "0" => Some(PhaseState::Phi0),
        "Phi1" | "phi1" | "Kairos1" | "kairos1" | "1" => Some(PhaseState::Phi1),
        "Phi2" | "phi2" | "Kairos2" | "kairos2" | "2" => Some(PhaseState::Phi2),
        "Phi3" | "phi3" | "Kairos3" | "kairos3" | "3" => Some(PhaseState::Phi3),
        _ => None,
    }
}
