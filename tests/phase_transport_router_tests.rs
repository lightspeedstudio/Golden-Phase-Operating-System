//! Router store-and-forward tests.

use kairos::kernel::phase_transport::lane::TransportLane;
use kairos::kernel::phase_transport::packet::PacketOpcode;
use kairos::kernel::phase_transport::router::TransportRouter;
use kairos::kernel::phase_transport::TransportController;

#[test]
fn router_links_digests() {
    let mut ctl = TransportController::init(4).unwrap();
    ctl.lanes.push(TransportLane::new(2, 4, false));
    ctl.routers.push(TransportRouter {
        id: 7,
        input_lane: 0,
        output_lane: 2,
        port: (1, 1),
    });
    let id = ctl
        .send((2, 2), (1, 1), PacketOpcode::Data, b"fwd", 0)
        .unwrap();
    ctl.tick(1).unwrap();
    let old = ctl
        .traces
        .iter()
        .find(|t| t.packet_id == id)
        .unwrap()
        .digest;
    ctl.route(7, id).unwrap();
    let child = ctl
        .packets
        .iter()
        .find(|p| p.parent_packet_id == Some(id))
        .unwrap();
    let linked = ctl
        .traces
        .iter()
        .find(|t| t.packet_id == child.packet_id)
        .unwrap()
        .linked_from
        .unwrap();
    assert_eq!(linked, old);
}
