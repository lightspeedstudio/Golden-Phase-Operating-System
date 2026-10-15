use kairos::core::gphase_core::PhaseState;
use kairos::kernel::anom::protocol::enter_anom_mode;
use kairos::kernel::phaselink::decoder::PhaseLinkDecoder;
use kairos::kernel::phaselink::encoder::PhaseLinkEncoder;
use kairos::kernel::phaselink::error::PhaseLinkError;
use kairos::kernel::phaselink::frame::{compute_frame_hash, serialize_frame_without_hash};
use kairos::kernel::phaselink::phase_stream::{bytes_to_phase_stream, phase_stream_to_bytes};
use kairos::kernel::phaselink::types::{PayloadEncoding, PhaseSymbol};
use kairos::kernel::phaselink::validation::FrameValidationResult;
use kairos::kernel::phaselink::{
    accept_or_quarantine, build_process_commitment_frame, send_loopback_text, sim_init,
    sim_send_text, sim_set_tamper,
};
use kairos::userland::boot;
use kairos::userland::Hosted;
use kairos::Runtime;

fn booted() -> Hosted {
    let _ = std::fs::remove_dir_all(".kairos/anom/objects");
    let mut hosted = Hosted::new();
    boot(&mut hosted);
    hosted
}

#[test]
fn all_byte_values_round_trip() {
    for byte in 0u8..=255 {
        let stream = bytes_to_phase_stream(&[byte]);
        let out = phase_stream_to_bytes(&stream).unwrap();
        assert_eq!(out, [byte]);
    }
}

#[test]
fn empty_round_trip() {
    let stream = bytes_to_phase_stream(&[]);
    assert!(stream.symbols.is_empty());
    assert_eq!(phase_stream_to_bytes(&stream).unwrap(), b"");
}

#[test]
fn byte_mapping_for_ascii_a() {
    let stream = bytes_to_phase_stream(&[0x41]);
    assert_eq!(
        stream.symbols,
        vec![
            PhaseSymbol::Phi1,
            PhaseSymbol::Phi0,
            PhaseSymbol::Phi0,
            PhaseSymbol::Phi1
        ]
    );
}

#[test]
fn invalid_phase_length_rejected() {
    let stream = kairos::kernel::phaselink::types::PhaseStream {
        symbols: vec![PhaseSymbol::Phi0, PhaseSymbol::Phi1, PhaseSymbol::Phi2],
    };
    assert_eq!(
        phase_stream_to_bytes(&stream).unwrap_err(),
        PhaseLinkError::InvalidPhaseStreamLength
    );
}

#[test]
fn frame_round_trip_and_hash_match() {
    let mut hosted = booted();
    let frame = send_loopback_text(&mut hosted.kernel, "HELLO").unwrap().0;
    let bytes = PhaseLinkEncoder::encode_frame(&frame).unwrap();
    let decoded = PhaseLinkDecoder::decode_frame(&bytes).unwrap();
    assert_eq!(decoded.payload, b"HELLO");
    assert_eq!(decoded.frame_hash, frame.frame_hash);
}

#[test]
fn wrong_magic_rejected() {
    let mut hosted = booted();
    let frame = send_loopback_text(&mut hosted.kernel, "HELLO").unwrap().0;
    let mut bytes = PhaseLinkEncoder::encode_frame(&frame).unwrap();
    bytes[0] ^= 1;
    assert_eq!(
        PhaseLinkDecoder::decode_frame(&bytes).unwrap_err(),
        PhaseLinkError::InvalidMagic
    );
}

#[test]
fn changed_frame_hash_detected() {
    let mut hosted = booted();
    let frame = send_loopback_text(&mut hosted.kernel, "HELLO").unwrap().0;
    let mut bytes = PhaseLinkEncoder::encode_frame(&frame).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    assert_eq!(
        PhaseLinkDecoder::decode_frame(&bytes).unwrap_err(),
        PhaseLinkError::FrameHashMismatch
    );
}

#[test]
fn sequence_gap_quarantined() {
    let mut hosted = booted();
    let first = send_loopback_text(&mut hosted.kernel, "HELLO").unwrap().0;
    assert!(matches!(
        accept_or_quarantine(&mut hosted.kernel, &first),
        FrameValidationResult::Accepted
    ));
    let mut frame = send_loopback_text(&mut hosted.kernel, "WORLD").unwrap().0;
    frame.previous_frame_hash = first.frame_hash;
    frame.transition_proof.previous_event_hash = first.frame_hash;
    frame.sequence_number = 5;
    frame.transition_proof.sequence_number = 5;
    let bytes_wo = serialize_frame_without_hash(&frame).unwrap();
    frame.frame_hash = compute_frame_hash(&bytes_wo);
    let result = accept_or_quarantine(&mut hosted.kernel, &frame);
    assert!(matches!(result, FrameValidationResult::Quarantined(_)));
}

#[test]
fn replay_detected() {
    let mut hosted = booted();
    let frame = send_loopback_text(&mut hosted.kernel, "HELLO").unwrap().0;
    assert!(matches!(
        accept_or_quarantine(&mut hosted.kernel, &frame),
        FrameValidationResult::Accepted
    ));
    assert!(matches!(
        accept_or_quarantine(&mut hosted.kernel, &frame),
        FrameValidationResult::Quarantined(_)
    ));
}

#[test]
fn loopback_updates_chain_and_root() {
    let mut hosted = booted();
    let initial_root = hosted.kernel.phaselink.root;
    let _ = send_loopback_text(&mut hosted.kernel, "HELLO").unwrap();
    let (_frame, _bytes, result) =
        kairos::kernel::phaselink::recv_loopback(&mut hosted.kernel).unwrap();
    assert!(matches!(result, FrameValidationResult::Accepted));
    assert_eq!(hosted.kernel.phaselink.accepted_chain.events.len(), 1);
    assert_ne!(hosted.kernel.phaselink.root, initial_root);
}

#[test]
fn simulation_send_accepts() {
    let mut hosted = booted();
    sim_init(&mut hosted.kernel);
    let (_frame, result) =
        sim_send_text(&mut hosted.kernel, "node-a", "node-b", "SENSOR_OK").unwrap();
    assert!(matches!(result, FrameValidationResult::Accepted));
}

#[test]
fn simulation_tamper_payload_quarantines() {
    let mut hosted = booted();
    sim_init(&mut hosted.kernel);
    assert!(sim_set_tamper(&mut hosted.kernel, "payload"));
    let result = sim_send_text(&mut hosted.kernel, "node-a", "node-b", "SENSOR_OK");
    match result {
        Err(PhaseLinkError::FrameHashMismatch) => {}
        Ok((_frame, FrameValidationResult::Rejected(_))) => {}
        Ok((_frame, FrameValidationResult::Quarantined(_))) => {}
        other => panic!("unexpected tamper result: {other:?}"),
    }
}

#[test]
fn anom_process_frame_uses_public_commitment() {
    let mut hosted = booted();
    enter_anom_mode(&mut hosted.kernel, b"phaselink-pass").unwrap();
    let pid = hosted.init_pid.unwrap();
    let destination = hosted.kernel.phaselink.local_node;
    let frame = build_process_commitment_frame(
        &mut hosted.kernel,
        pid,
        destination,
        "public_process_state",
        PhaseState::Phi1,
        PhaseState::Phi2,
    )
    .unwrap();
    let bytes = PhaseLinkEncoder::encode_frame(&frame).unwrap();
    assert_eq!(frame.payload_encoding, PayloadEncoding::PublicCommitment);
    assert!(frame.flags.anom_public);
    assert!(!bytes
        .windows(b"phaselink-pass".len())
        .any(|w| w == b"phaselink-pass"));
}

#[test]
fn gateway_defaults_disabled() {
    let hosted = booted();
    assert_eq!(hosted.kernel.phaselink.gateway.state as u8, 0);
    assert!(!hosted.kernel.phaselink.gateway.allow_anom_decryption);
}

#[test]
fn gateway_requires_correct_passphrase() {
    let mut hosted = booted();
    enter_anom_mode(&mut hosted.kernel, b"phase-gateway-pass").unwrap();
    assert!(kairos::kernel::phaselink::gateway_enable(&mut hosted.kernel, b"wrong").is_err());
    assert!(
        kairos::kernel::phaselink::gateway_enable(&mut hosted.kernel, b"phase-gateway-pass")
            .is_ok()
    );
}

#[test]
fn malformed_inputs_do_not_panic() {
    for len in 0..128usize {
        let bytes = vec![0xA5; len];
        let _ = PhaseLinkDecoder::decode_frame(&bytes);
    }
}

#[test]
fn shell_integration_transcript_contains_phaselink() {
    let mut rt = Runtime::with_input([
        "phaselink encode A",
        "phaselink decode Φ1 Φ0 Φ0 Φ1",
        "phaselink loopback HELLO",
        "phaselink status",
        "exit",
    ]);
    rt.boot().unwrap();
    let pid = rt.hosted.init_pid.unwrap();
    rt.hosted.kernel.syscall(pid).unwrap();
    rt.hosted.kernel.kreturn(pid).unwrap();
    rt.hosted.kernel.schedule(pid).unwrap();
    rt.run().unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("phase stream"));
    assert!(out.contains("[ RESULT ] HELLO"));
    assert!(out.contains("PhaseLinkRoot"));
}
