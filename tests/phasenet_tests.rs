//! PhaseNet — native phase-state network tests.

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::phasenet::{
    addr_for_uri, compute_phase_addr, decode_bytes, decode_frame, encode_bytes, encode_frame,
    evaluate_frame, parse_phase_uri, FirewallContext, PhaseLinkFrame, PhaseNetSimBus,
    PhaseNetVerdict, PhasePacket,
};
use kairos::kernel::state_proof::ObjectKind;
use kairos::kernel::state_root::compute_equation_root;
use kairos::Runtime;

fn eq_root() -> [u8; 32] {
    compute_equation_root().hash
}

#[test]
fn phase_stream_round_trip() {
    let data = b"KairOS";
    let stream = encode_bytes(data);
    assert_eq!(stream.len(), data.len() * 4);
    let back = decode_bytes(&stream).unwrap();
    assert_eq!(back, data);
}

#[test]
fn phase_stream_byte_mapping() {
    let stream = encode_bytes(&[0b00_01_10_11]);
    assert_eq!(
        stream,
        vec![
            PhaseState::Phi0,
            PhaseState::Phi1,
            PhaseState::Phi2,
            PhaseState::Phi3,
        ]
    );
}

#[test]
fn phase_uri_parse_and_addr() {
    let uri = parse_phase_uri("phase://craft/nav/state").unwrap();
    assert_eq!(uri.domain, "craft.nav");
    assert_eq!(uri.object_path, "state");
    let eq = eq_root();
    let node = [1u8; 32];
    let addr = addr_for_uri(&eq, &node, &uri, 0, 0);
    assert_ne!(addr, [0u8; 32]);
    let addr2 = compute_phase_addr(&eq, &node, "craft.nav", 0, 0, "state");
    assert_eq!(addr, addr2);
}

#[test]
fn phase_packet_proof_and_hash() {
    let snap = kairos::StateClock::new().snapshot();
    let packet = PhasePacket::state_update(
        eq_root(),
        "node-a",
        "node-b",
        "craft.nav",
        "state_update",
        PhaseState::Phi1,
        PhaseState::Phi2,
        snap,
        [0u8; 32],
    );
    assert!(packet.verify());
}

#[test]
fn phase_link_frame_round_trip() {
    let snap = kairos::StateClock::new().snapshot();
    let packet = PhasePacket::state_update(
        eq_root(),
        "local",
        "craft.nav",
        "craft.nav",
        "test",
        PhaseState::Phi0,
        PhaseState::Phi1,
        snap,
        [0u8; 32],
    );
    let src = [2u8; 32];
    let dst = [3u8; 32];
    let frame = PhaseLinkFrame::from_packet(
        src,
        dst,
        [4u8; 32],
        eq_root(),
        1,
        1,
        snap.into(),
        [0u8; 32],
        &packet,
    );
    let bytes = encode_frame(&frame);
    let decoded = decode_frame(&bytes).unwrap();
    assert_eq!(decoded.frame_hash, frame.frame_hash);
    assert!(decoded.packet().unwrap().verify());
}

#[test]
fn firewall_accepts_valid_frame() {
    let snap = kairos::StateClock::new().snapshot();
    let eq = eq_root();
    let boot = [5u8; 32];
    let src = compute_phase_addr(&eq, &boot, "kairos", 0, 0, "node");
    let packet = PhasePacket::state_update(
        eq,
        "node-a",
        "node-b",
        "craft.power",
        "state_update",
        PhaseState::Phi2,
        PhaseState::Phi3,
        snap,
        [0u8; 32],
    );
    let frame =
        PhaseLinkFrame::from_packet(src, src, boot, eq, 0, 1, snap.into(), [0u8; 32], &packet);
    let mut peers = kairos::platform::HashSet::new();
    peers.insert(src);
    let ctx = FirewallContext {
        equation_root: eq,
        boot_session_root: boot,
        trusted_peer_addrs: peers,
        anom_protected: false,
    };
    let result = evaluate_frame(&frame, &ctx, 0, &[0u8; 32]);
    assert_eq!(result.verdict, PhaseNetVerdict::Accept);
}

#[test]
fn sim_bus_delivers_between_nodes() {
    let eq = eq_root();
    let boot_a = [7u8; 32];
    let boot_b = [8u8; 32];
    let mut bus = PhaseNetSimBus::default();
    bus.init_pair(eq, boot_a, boot_b);
    let snap = kairos::StateClock::new().snapshot();
    bus.send_state_update(
        "node-a",
        "node-b",
        "phase://craft/nav/state",
        "state_update",
        PhaseState::Phi1,
        PhaseState::Phi2,
        snap,
    )
    .unwrap();
    let rx = bus.recv("node-b").expect("frame");
    assert_eq!(rx.verdict, PhaseNetVerdict::Accept);
    let packet = rx.frame.packet().unwrap();
    assert_eq!(packet.domain, "craft.nav");
}

#[test]
fn loopback_shell_integration() {
    let mut rt = Runtime::with_input([
        "phasenet status",
        "phasenet loopback send phase://craft/power/root state_update Phi1 Phi2",
        "phasenet loopback recv",
        "exit",
    ]);
    rt.boot().unwrap();
    let pid = rt.hosted.init_pid.unwrap();
    rt.hosted.kernel.syscall(pid).unwrap();
    rt.hosted.kernel.kreturn(pid).unwrap();
    rt.hosted.kernel.schedule(pid).unwrap();
    rt.run().unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("PhaseNet"));
    assert!(out.contains("loopback"));
    assert!(out.contains("verdict: ACCEPT"));
}

#[test]
fn boots_with_phasenet_loopback() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    assert_eq!(
        rt.hosted.kernel.phasenet.mode,
        kairos::kernel::phasenet::PhaseNetMode::Loopback
    );
}

#[test]
fn statechain_records_phasenet_loopback() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let snap = rt.hosted.kernel.clock.snapshot();
    rt.hosted
        .kernel
        .phasenet
        .loopback_send_state_update(
            "phase://craft/nav/state",
            "test",
            PhaseState::Phi0,
            PhaseState::Phi1,
            snap,
            &mut rt.hosted.kernel.statechain,
        )
        .unwrap();
    let chain = rt
        .hosted
        .kernel
        .statechain
        .chain(
            ObjectKind::Driver,
            kairos::kernel::phasenet::PHASENET_STATECHAIN_OBJECT_ID,
        )
        .unwrap();
    assert!(chain
        .events
        .iter()
        .any(|e| e.proof.operation_name == "phasenet_loopback"));
}
