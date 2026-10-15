//! PhaseNet status and view rendering.

use crate::core::gphase_core::PhaseState;
use crate::kernel::phasenet::addr::{render_addr_short, PhaseUri};
use crate::kernel::phasenet::firewall::{render_failures, render_verdict, PhaseNetVerdict};
use crate::kernel::phasenet::link::PhaseLinkFrame;
use crate::kernel::phasenet::packet::PhasePacket;
use crate::kernel::phasenet::{PhaseNetController, PhaseNetMode};
use crate::platform::String;

pub fn render_status(ctrl: &PhaseNetController) -> String {
    let mut out = String::new();
    out.push_str("GPOS PhaseNet\n\n");
    out.push_str(&format!("mode: {}\n", mode_label(ctrl.mode)));
    out.push_str(&format!("node: {}\n", ctrl.node_name));
    out.push_str(&format!(
        "node_root: {}\n",
        render_addr_short(&ctrl.node_root)
    ));
    out.push_str(&format!(
        "equation_root: {}\n",
        render_addr_short(&ctrl.equation_root)
    ));
    out.push_str(&format!(
        "boot_session_root: {}\n",
        render_addr_short(&ctrl.boot_session_root)
    ));
    out.push_str(&format!("sequence_out: {}\n", ctrl.sequence_out));
    out.push_str(&format!("sequence_in: {}\n", ctrl.sequence_in));
    #[cfg(feature = "std")]
    out.push_str(&format!("pending_rx: {}\n", ctrl.rx_queue.len()));
    out.push_str(&format!(
        "sim_bus: {}\n",
        if ctrl.sim_bus.is_some() {
            "active"
        } else {
            "off"
        }
    ));
    out.push_str("\nroot alphabet: Φ0 ─ Φ1 ─ Φ2 ─ Φ3\n");
    out
}

pub fn render_packet_view(packet: &PhasePacket, anom_protected: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!("domain: {}\n", packet.domain));
    out.push_str(&format!("operation: {}\n", packet.operation));
    out.push_str(&format!(
        "src: {} -> dst: {}\n",
        packet.src_node, packet.dst_node
    ));
    if anom_protected {
        out.push_str("public state: ANOM:commitment\n");
        out.push_str("true state: root-only\n");
    } else {
        out.push_str(&format!(
            "transition: {} -> {}\n",
            phase_short(packet.old_phase),
            phase_short(packet.new_phase)
        ));
    }
    out.push_str(&format!(
        "stateclock: sector {}  x={:.3}\n",
        packet.snapshot.sector_id, packet.snapshot.x
    ));
    out.push_str(&format!(
        "proof: {}\n",
        render_addr_short(&packet.proof_hash)
    ));
    out.push_str(&format!(
        "previous: {}\n",
        render_addr_short(&packet.previous_hash)
    ));
    out.push_str(&format!(
        "packet_hash: {}\n",
        render_addr_short(&packet.packet_hash)
    ));
    out
}

pub fn render_frame_summary(frame: &PhaseLinkFrame, verdict: PhaseNetVerdict) -> String {
    format!(
        "PhaseLink seq={} channel={} verdict={} frame={}\n",
        frame.sequence,
        frame.channel,
        render_verdict(verdict),
        render_addr_short(&frame.frame_hash),
    )
}

pub fn render_uri_addr(uri: &PhaseUri, addr: &crate::kernel::authority::StateHash) -> String {
    format!(
        "uri: {}\ndomain: {}\nobject: {}\nphase_addr: {}\n",
        uri.raw,
        uri.domain,
        uri.object_path,
        render_addr_short(addr),
    )
}

fn mode_label(mode: PhaseNetMode) -> &'static str {
    match mode {
        PhaseNetMode::Off => "off",
        PhaseNetMode::Loopback => "loopback",
        PhaseNetMode::Sim => "sim",
    }
}

fn phase_short(p: PhaseState) -> &'static str {
    match p {
        PhaseState::Phi0 => "Φ0",
        PhaseState::Phi1 => "Φ1",
        PhaseState::Phi2 => "Φ2",
        PhaseState::Phi3 => "Φ3",
    }
}

pub fn render_recv_line(
    verdict: PhaseNetVerdict,
    failures: &[crate::kernel::phasenet::firewall::FirewallFailure],
    packet: Option<&PhasePacket>,
    anom_protected: bool,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("verdict: {}\n", render_verdict(verdict)));
    if !failures.is_empty() {
        out.push_str(&format!("failures: {}\n", render_failures(failures)));
    }
    if let Some(pkt) = packet {
        out.push_str(&render_packet_view(pkt, anom_protected));
    }
    out
}
