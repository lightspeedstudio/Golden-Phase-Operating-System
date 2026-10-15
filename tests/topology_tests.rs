use kairos::core::gphase_core::{
    left_group, reflect, right_group, topology_status, PhaseState, TopologyKind,
};

#[test]
fn all_64_triples_same_or_reflection() {
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            for c in PhaseState::ALL {
                let result = topology_status(a, b, c);
                match result.kind {
                    TopologyKind::Same => assert_eq!(result.left, result.right),
                    TopologyKind::Reflection => {
                        assert_eq!(result.right, reflect(result.left));
                    }
                }
            }
        }
    }
}

#[test]
fn counts_52_same_12_reflection() {
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

#[test]
fn left_and_right_group_match_topology() {
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            for c in PhaseState::ALL {
                let result = topology_status(a, b, c);
                assert_eq!(left_group(a, b, c), result.left);
                assert_eq!(right_group(a, b, c), result.right);
            }
        }
    }
}

#[test]
fn kernel_integration_topology_survives_facade() {
    let mut k = kairos::Kernel::new();
    let pid = k.spawn();
    assert!(k.wake(pid).unwrap());
    assert!(k.schedule(pid).unwrap());
    assert!(k.syscall(pid).unwrap());

    let events = k.topology_report();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, TopologyKind::Reflection);
    assert_eq!(
        events[0].triple,
        (PhaseState::Phi1, PhaseState::Phi2, PhaseState::Phi3)
    );
}
