//! T1/T12-style seal: `tensor` is default-OFF and must not appear in
//! default features. Layer-0 52/12 census must still hold without the
//! feature (the algebra is not behind `tensor`).
#![cfg(not(feature = "tensor"))]

use kairos::core::gphase_core::{topology_status, PhaseState, TopologyKind};

#[test]
fn default_features_do_not_enable_tensor() {
    let cargo = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
    let default_line = cargo
        .lines()
        .find(|l| l.starts_with("default ="))
        .expect("Cargo.toml default features line");
    assert!(
        !default_line.contains("tensor"),
        "tensor leaked into default features: {default_line}"
    );
    assert!(
        cargo.contains("tensor = []"),
        "tensor feature must remain an empty default-OFF flag"
    );
    assert!(
        default_line.contains("std"),
        "T14 must not wipe default features; sealed host default stays std/ssh: {default_line}"
    );
}

#[test]
fn g1p_tensor_mmio_is_feature_gated() {
    let g1p = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/board/zynq7000/g1p.rs"
    ));
    assert!(
        g1p.contains("#[cfg(feature = \"tensor\")]"),
        "tensor register bank must not compile into sealed default"
    );
    assert!(
        g1p.contains("0x100"),
        "Path-1 tensor offset is 0x100 inside the G1P window, not a new AXI base"
    );
    assert!(
        !g1p.contains("0x1FC") && !g1p.contains("0x0000_01FC"),
        "do not invent a 0x1FC tensor register; Path-1 last offset is 0x118"
    );
    assert!(
        g1p.contains("0x544E_5352") || g1p.contains("0x544E5352"),
        "TNSR magic 0x544E5352 must be the Path-1 ID"
    );
    let mmap = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/board/zynq7000/memory_map.rs"
    ));
    assert!(
        mmap.contains("G1P_TENSOR_OFFSET") && mmap.contains("0x100"),
        "memory_map must name Path-1 offset 0x100"
    );
    assert!(
        mmap.contains("G1P_TENSOR_LAST") && mmap.contains("0x118"),
        "Path-1 last occupied offset is 0x118, not 0x1FC"
    );
}

#[test]
fn layer0_census_holds_with_tensor_feature_off() {
    let mut same = 0;
    let mut reflection = 0;
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            for c in PhaseState::ALL {
                match topology_status(a, b, c).kind {
                    TopologyKind::Same => same += 1,
                    TopologyKind::Reflection => reflection += 1,
                }
            }
        }
    }
    assert_eq!(same, 52);
    assert_eq!(reflection, 12);
}
