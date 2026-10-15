//! `topo` — recent topology / reflection events.

use crate::core::gphase_core::TopologyKind;
use crate::kernel::Kernel;
use crate::platform::{String, Vec};

pub fn render(kernel: &Kernel) -> String {
    let events = kernel.topology_report();
    if events.is_empty() {
        return "no topology events".into();
    }
    let lines: Vec<String> = events
        .iter()
        .map(|e| {
            let kind = match e.kind {
                TopologyKind::Same => "same",
                TopologyKind::Reflection => "reflection",
            };
            format!(
                "pid={} triple=({},{},{}) {kind} L={:?} R={:?}",
                e.pid,
                e.triple.0.as_u8(),
                e.triple.1.as_u8(),
                e.triple.2.as_u8(),
                e.left,
                e.right
            )
        })
        .collect();
    lines.join("\n")
}
