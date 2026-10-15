//! Transport replay / tamper tests.

use kairos::kernel::phase_transport::error::TransportError;
use kairos::kernel::phase_transport::packet::PacketOpcode;
use kairos::kernel::phase_transport::TransportController;

#[test]
fn replay_matches_live_digest() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (0, 0), PacketOpcode::Data, b"rep", 0)
        .unwrap();
    ctl.tick(2).unwrap();
    let live = ctl
        .traces
        .iter()
        .find(|t| t.packet_id == id)
        .unwrap()
        .digest;
    assert_eq!(ctl.replay_trace(id).unwrap(), live);
}

#[test]
fn live_vs_replay_mismatch_after_tamper() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (0, 0), PacketOpcode::Data, b"tam", 0)
        .unwrap();
    ctl.tick(2).unwrap();
    ctl.inject_fault(id, "tamper_digest").unwrap();
    assert!(matches!(
        ctl.replay_trace(id),
        Err(TransportError::TraceMismatch)
    ));
}
