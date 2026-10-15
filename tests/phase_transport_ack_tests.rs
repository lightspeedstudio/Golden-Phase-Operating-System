//! ACK-plane focused tests.

use kairos::kernel::phase_transport::ack::AckPlane;
use kairos::kernel::phase_transport::packet::PacketOpcode;
use kairos::kernel::phase_transport::TransportController;

#[test]
fn logical_flip_maps_minus_one_physical_to_plus_one_logical() {
    let m = 4;
    // physical 2 ->1 (-1); logical from phys: 2->2, 1->3 (+1)
    let l0 = AckPlane::logical_from_physical(2, 2, m);
    let l1 = AckPlane::logical_from_physical(1, 1, m);
    assert_eq!(l0, (2, 2));
    assert_eq!(l1, (3, 3));
    let dr = (l1.0 as isize - l0.0 as isize).rem_euclid(m as isize);
    let dc = (l1.1 as isize - l0.1 as isize).rem_euclid(m as isize);
    assert_eq!((dr, dc), (1, 1));
}

#[test]
fn full_round_trip_data_then_ack() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (0, 0), PacketOpcode::Data, b"rt", 0)
        .unwrap();
    ctl.tick(2).unwrap();
    let ack = ctl.acknowledge(id).unwrap();
    ctl.tick(2).unwrap();
    let ap = ctl.packets.iter().find(|p| p.packet_id == ack).unwrap();
    assert_eq!(
        ap.status.phase,
        kairos::kernel::phase_transport::packet::TransportPhase::Received
    );
    assert_eq!(ap.parent_packet_id, Some(id));
}
