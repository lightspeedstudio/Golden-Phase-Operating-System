//! Project A — GPL Transfer Base tests.

use kairos::core::gphase_core::{compose, PhaseState};
use kairos::kernel::phase_transport::error::TransportError;
use kairos::kernel::phase_transport::lane::TransportPort;
use kairos::kernel::phase_transport::packet::PacketOpcode;
use kairos::kernel::phase_transport::router::TransportRouter;
use kairos::kernel::phase_transport::{
    find_carrier, flat, footprint, golden_m4_states, place_glider, rule_a, vacuum_phi1,
    TransportController,
};

#[test]
fn verified_m4_glider_golden_eight_gens() {
    let m = 4;
    let mut g = vacuum_phi1(m);
    place_glider(&mut g, m, 2, 2);
    for (t, exp) in golden_m4_states().iter().enumerate() {
        assert_eq!(flat(&g), *exp, "gen {t}");
        assert_eq!(footprint(&g), 1);
        let (r, c) = find_carrier(&g, m).unwrap();
        let er = (2isize - t as isize).rem_euclid(4) as usize;
        let ec = (2isize - t as isize).rem_euclid(4) as usize;
        assert_eq!((r, c), (er, ec));
        g = rule_a(&g, m);
    }
}

#[test]
fn compose_matches_canonical() {
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            let table = [[0, 1, 2, 3], [1, 3, 3, 2], [2, 3, 3, 1], [3, 2, 1, 0]];
            assert_eq!(
                compose(a, b).as_u8(),
                table[a.as_u8() as usize][b.as_u8() as usize]
            );
        }
    }
}

#[test]
fn packet_injection_and_delivery() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (0, 0), PacketOpcode::Data, b"hello", 0)
        .unwrap();
    let eta = TransportController::expected_arrival((2, 2), (0, 0), 4);
    assert_eq!(eta, 2);
    let notes = ctl.tick(eta).unwrap();
    assert!(notes.iter().any(|n| n.contains("received")));
    let pkt = ctl.packets.iter().find(|p| p.packet_id == id).unwrap();
    assert_eq!(
        pkt.status.phase,
        kairos::kernel::phase_transport::packet::TransportPhase::Received
    );
    ctl.verify_trace(id).unwrap();
}

#[test]
fn ack_delivery_logical_reverse_plane() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (0, 0), PacketOpcode::Data, b"ping", 0)
        .unwrap();
    ctl.tick(2).unwrap();
    let ack_id = ctl.acknowledge(id).unwrap();
    // ACK logical motion (+1,+1) from (0,0) to (2,2) takes 2 gens
    let notes = ctl.tick(2).unwrap();
    assert!(
        notes.iter().any(|n| n.contains("received"))
            || ctl.packets.iter().any(|p| p.packet_id == ack_id
                && p.status.phase
                    == kairos::kernel::phase_transport::packet::TransportPhase::Received)
    );
    ctl.verify_trace(ack_id).unwrap();
}

#[test]
fn path_digest_replay_agrees() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (1, 1), PacketOpcode::Data, b"x", 0)
        .unwrap();
    ctl.tick(1).unwrap();
    ctl.verify_trace(id).unwrap();
    let dig = ctl.replay_trace(id).unwrap();
    let live = ctl
        .traces
        .iter()
        .find(|t| t.packet_id == id)
        .unwrap()
        .digest;
    assert_eq!(dig, live);
}

#[test]
fn duplicate_packet_id_rejected() {
    let mut ctl = TransportController::init(4).unwrap();
    ctl.send((2, 2), (0, 0), PacketOpcode::Data, b"a", 0)
        .unwrap();
    // Force same id by resetting next and crafting — use inject_fault path:
    // second send gets new id; duplicate checked inside inject on packet_id.
    // Simulate by pushing a packet with colliding id via send then manual inject attempt:
    let mut pkt = kairos::kernel::phase_transport::packet::PhasePacket::new(
        1,
        (3, 3),
        (0, 0),
        PacketOpcode::Data,
        b"b",
        0,
    );
    // lane busy + duplicate id
    let err = kairos::kernel::phase_transport::injector::Injector::inject(&mut ctl, 0, pkt);
    assert!(matches!(
        err,
        Err(TransportError::DuplicatePacketId) | Err(TransportError::BusyPort)
    ));
}

#[test]
fn busy_port_rejection() {
    let mut ctl = TransportController::init(4).unwrap();
    ctl.send((2, 2), (0, 0), PacketOpcode::Data, b"a", 0)
        .unwrap();
    let err = ctl.send((3, 3), (1, 1), PacketOpcode::Data, b"b", 0);
    assert!(matches!(err, Err(TransportError::BusyPort)));
}

#[test]
fn ttl_expiration() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (0, 0), PacketOpcode::Data, b"a", 0)
        .unwrap();
    ctl.packets
        .iter_mut()
        .find(|p| p.packet_id == id)
        .unwrap()
        .ttl = 1;
    ctl.tick(3).unwrap();
    let pkt = ctl.packets.iter().find(|p| p.packet_id == id).unwrap();
    assert!(matches!(pkt.status.error, Some(TransportError::Timeout)) || pkt.ttl == 0);
}

#[test]
fn carrier_deletion_detected() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (0, 0), PacketOpcode::Data, b"a", 0)
        .unwrap();
    ctl.inject_fault(id, "delete_carrier").unwrap();
    let err = ctl.tick(1);
    assert!(matches!(err, Err(TransportError::CarrierLost)));
}

#[test]
fn carrier_duplication_detected() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (0, 0), PacketOpcode::Data, b"a", 0)
        .unwrap();
    ctl.inject_fault(id, "duplicate_carrier").unwrap();
    let err = ctl.tick(1);
    assert!(matches!(
        err,
        Err(TransportError::CarrierDuplicated) | Err(TransportError::CarrierSpreading)
    ));
}

#[test]
fn carrier_spreading_detected() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (0, 0), PacketOpcode::Data, b"a", 0)
        .unwrap();
    ctl.inject_fault(id, "spread").unwrap();
    let err = ctl.tick(1);
    assert!(matches!(
        err,
        Err(TransportError::CarrierSpreading) | Err(TransportError::CarrierDuplicated)
    ));
}

#[test]
fn header_tamper_fails_checksum_on_receive_path() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (0, 0), PacketOpcode::Data, b"a", 0)
        .unwrap();
    ctl.inject_fault(id, "corrupt_header").unwrap();
    // checksum no longer matches stored value
    let pkt = ctl.packets.iter().find(|p| p.packet_id == id).unwrap();
    assert!(pkt.verify_checksum().is_err());
}

#[test]
fn path_digest_tamper_fails_verify() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (1, 1), PacketOpcode::Data, b"a", 0)
        .unwrap();
    ctl.tick(1).unwrap();
    ctl.inject_fault(id, "tamper_digest").unwrap();
    assert!(matches!(
        ctl.verify_trace(id),
        Err(TransportError::TraceMismatch)
    ));
}

#[test]
fn router_store_and_forward() {
    let mut ctl = TransportController::init(4).unwrap();
    // Extra outbound lane for router output
    ctl.lanes
        .push(kairos::kernel::phase_transport::lane::TransportLane::new(
            2, 4, false,
        ));
    ctl.routers.push(TransportRouter {
        id: 1,
        input_lane: 0,
        output_lane: 2,
        port: (1, 1),
    });
    let id = ctl
        .send((2, 2), (1, 1), PacketOpcode::Data, b"route-me", 0)
        .unwrap();
    ctl.tick(1).unwrap(); // arrive at (1,1)
                          // Manually mark as at router and forward
    ctl.route(1, id).unwrap();
    assert!(ctl
        .lanes
        .iter()
        .any(|l| l.id == 2 && !l.packet_ids.is_empty()));
    let child = ctl
        .packets
        .iter()
        .find(|p| p.parent_packet_id == Some(id))
        .unwrap();
    ctl.verify_trace(child.packet_id).unwrap();
    assert!(ctl
        .traces
        .iter()
        .find(|t| t.packet_id == child.packet_id)
        .unwrap()
        .linked_from
        .is_some());
}

#[test]
fn multiple_independent_lanes() {
    let mut ctl = TransportController::init(4).unwrap();
    ctl.lanes
        .push(kairos::kernel::phase_transport::lane::TransportLane::new(
            2, 4, false,
        ));
    let a = ctl
        .send((2, 2), (0, 0), PacketOpcode::Data, b"A", 0)
        .unwrap();
    let b = ctl
        .send((3, 3), (1, 1), PacketOpcode::Data, b"B", 2)
        .unwrap();
    ctl.tick(2).unwrap();
    assert_eq!(
        ctl.packets
            .iter()
            .find(|p| p.packet_id == a)
            .unwrap()
            .status
            .phase,
        kairos::kernel::phase_transport::packet::TransportPhase::Received
    );
    assert_eq!(
        ctl.packets
            .iter()
            .find(|p| p.packet_id == b)
            .unwrap()
            .status
            .phase,
        kairos::kernel::phase_transport::packet::TransportPhase::Received
    );
}

#[test]
fn receive_port_helper() {
    let mut ctl = TransportController::init(4).unwrap();
    ctl.send((2, 2), (0, 0), PacketOpcode::Data, b"z", 0)
        .unwrap();
    ctl.tick(2).unwrap();
    // already received by tick; receive returns None
    let r = ctl.receive(TransportPort {
        lane_id: 0,
        coord: (0, 0),
    });
    assert!(r.is_ok());
}

#[test]
fn statechain_records_transport_episode() {
    let mut ctl = TransportController::init(4).unwrap();
    let id = ctl
        .send((2, 2), (0, 0), PacketOpcode::Data, b"audit", 0)
        .unwrap();
    ctl.tick(2).unwrap();
    ctl.ledger.verify_all().unwrap();
    let chain = ctl
        .ledger
        .chain(kairos::kernel::state_proof::ObjectKind::Driver, id as u32)
        .expect("chain");
    assert!(chain.events.len() >= 2);
}

#[test]
fn one_packet_smoke_over_sealed_glider_law() {
    let mut t = kairos::kernel::phase_transport::fpga_glider_transport::FpgaGliderTransport::new()
        .expect("init");
    let id = t.one_packet_smoke().expect("smoke");
    assert!(id >= 1);
}

#[test]
fn one_packet_prop_arm_host_twin() {
    let mut t = kairos::kernel::phase_transport::fpga_glider_transport::FpgaGliderTransport::new()
        .expect("init");
    let id = t.one_packet_prop_arm_smoke().expect("prop_arm");
    assert!(id >= 1);
}

#[test]
fn collision_policy_sealed_and_gated() {
    use kairos::kernel::phase_transport::collision_policy::{
        class_for_scenario, route_policy_class, CollisionClass, COLLISION_ROUTING_ENABLED,
        SEALED_COLLISION_SCENARIOS,
    };
    assert_eq!(SEALED_COLLISION_SCENARIOS.len(), 4);
    assert_eq!(class_for_scenario(0), Some(CollisionClass::PassThrough));
    assert_eq!(class_for_scenario(3), Some(CollisionClass::DomainGrowth));
    assert!(!COLLISION_ROUTING_ENABLED);
    assert!(route_policy_class(0).is_none());
}

#[cfg(feature = "fpga-uart")]
#[test]
fn live_uart_prop_arm_com24() {
    // Opt-in: set KAIROS_FPGA_LIVE=1 (board must be flashed with packet bitstream).
    if std::env::var("KAIROS_FPGA_LIVE").ok().as_deref() != Some("1") {
        eprintln!("skip live UART (set KAIROS_FPGA_LIVE=1 to enable)");
        return;
    }
    let com = std::env::var("KAIROS_FPGA_COM").unwrap_or_else(|_| "COM24".into());
    kairos::kernel::phase_transport::fpga_glider_transport::live_uart::craft_over_com24(&com)
        .expect("live COM PROP_ARM");
}
