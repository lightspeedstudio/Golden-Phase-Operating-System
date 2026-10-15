use kairos::core::gphase_core::{compose, reflect, threshold_energy, threshold_sign, PhaseState};

const TABLE: [[u8; 4]; 4] = [[0, 1, 2, 3], [1, 3, 3, 2], [2, 3, 3, 1], [3, 2, 1, 0]];

#[test]
fn compose_table_16_cells() {
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            assert_eq!(
                compose(a, b).as_u8(),
                TABLE[a.as_u8() as usize][b.as_u8() as usize]
            );
        }
    }
}

#[test]
fn phi0_is_identity() {
    for x in PhaseState::ALL {
        assert_eq!(compose(PhaseState::Phi0, x), x);
        assert_eq!(compose(x, PhaseState::Phi0), x);
    }
}

#[test]
fn phi3_phi3_returns_phi0() {
    assert_eq!(
        compose(PhaseState::Phi3, PhaseState::Phi3),
        PhaseState::Phi0
    );
}

#[test]
fn reflect_equals_compose_phi3() {
    for x in PhaseState::ALL {
        assert_eq!(reflect(x), compose(PhaseState::Phi3, x));
    }
}

#[test]
fn thresholds() {
    let sign: Vec<u8> = PhaseState::ALL.iter().map(|&s| threshold_sign(s)).collect();
    let energy: Vec<u8> = PhaseState::ALL
        .iter()
        .map(|&s| threshold_energy(s))
        .collect();
    assert_eq!(sign, vec![0, 0, 1, 1]);
    assert_eq!(energy, vec![1, 0, 0, 1]);
}
