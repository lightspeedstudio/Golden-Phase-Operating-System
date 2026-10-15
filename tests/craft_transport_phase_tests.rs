//! Avenue 10B WP-1 — craft fixture bridged to the real glider carrier.

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::craft::{
    CraftCommand, CraftController, CraftTransport, PhaseTransportCraftAdapter,
};

#[test]
fn command_round_trips_through_glider_carrier() {
    let mut adapter = PhaseTransportCraftAdapter::new().expect("adapter init");
    let id = adapter
        .send("kernel", "propulsion", "prop_arm", b"prop_arm")
        .expect("send delivered");

    let rec = adapter
        .records()
        .iter()
        .find(|r| r.packet_id == id)
        .expect("mirrored record for sent packet");
    assert_eq!(rec.source, "kernel");
    assert_eq!(rec.destination, "propulsion");
    assert_eq!(rec.opcode, "prop_arm");
    assert!(!rec.ack);

    let ack_id = adapter.acknowledge(id).expect("ack delivered");
    assert_ne!(ack_id, id);

    // original record flips to ack=true once the reply carrier lands.
    let orig = adapter
        .records()
        .iter()
        .find(|r| r.packet_id == id)
        .unwrap();
    assert!(orig.ack);

    let ack_rec = adapter
        .records()
        .iter()
        .find(|r| r.packet_id == ack_id)
        .expect("mirrored ACK record");
    assert_eq!(ack_rec.source, "propulsion");
    assert_eq!(ack_rec.destination, "kernel");
    assert_eq!(ack_rec.opcode, "ACK");
    assert!(ack_rec.ack);
}

#[test]
fn every_craft_endpoint_pair_round_trips() {
    let mut adapter = PhaseTransportCraftAdapter::new().expect("adapter init");
    for dest in ["navigation", "propulsion", "power"] {
        let id = adapter
            .send("kernel", dest, "heartbeat", b"hb")
            .unwrap_or_else(|e| panic!("send to {dest} failed: {e}"));
        adapter
            .acknowledge(id)
            .unwrap_or_else(|e| panic!("ack from {dest} failed: {e}"));
    }
    // 3 sends + 3 acks mirrored.
    assert_eq!(adapter.records().len(), 6);
}

#[test]
fn craft_controller_arm_fire_over_glider_carrier() {
    let mut c = CraftController::new_over_glider().expect("controller over glider carrier");
    c.init_craft().unwrap();

    c.execute(CraftCommand::PropArm, None).unwrap();
    assert_eq!(c.obj(2).unwrap().state, PhaseState::Phi1);
    c.execute(CraftCommand::PropFire, None).unwrap();
    assert_eq!(c.obj(2).unwrap().state, PhaseState::Phi2);

    let arm = c
        .transport
        .records()
        .iter()
        .find(|r| r.opcode == "prop_arm")
        .expect("prop_arm packet mirrored");
    assert_eq!(arm.source, "kernel");
    assert_eq!(arm.destination, "propulsion");
    assert!(arm.ack, "controller acknowledges every send in execute()");

    let fire = c
        .transport
        .records()
        .iter()
        .find(|r| r.opcode == "prop_fire")
        .expect("prop_fire packet mirrored");
    assert!(fire.ack);

    // Each send() got a mirrored ACK reply packet on top of the flipped flag.
    let ack_replies = c
        .transport
        .records()
        .iter()
        .filter(|r| r.opcode == "ACK")
        .count();
    assert_eq!(ack_replies, 2);
}
