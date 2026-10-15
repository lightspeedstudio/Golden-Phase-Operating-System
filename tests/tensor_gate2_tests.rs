//! Gate T2: exhaustive 256-encoding tests through the public
//! `kernel::tensor_state` API (not only the in-module tests).
#![cfg(feature = "tensor")]

use kairos::core::gphase_core::{compose, reflect, PhaseState};
use kairos::kernel::tensor_state::{
    CapabilityClass, SchedulerPhase, TensorState, TensorStateError,
};

#[test]
fn public_api_256_round_trip_and_pi_s() {
    for s in PhaseState::ALL {
        for m in SchedulerPhase::ALL {
            for v in 0u8..16 {
                let x = TensorState::new(s, m, CapabilityClass::from_u8(v).unwrap());
                assert_eq!(TensorState::from_raw(x.raw()).unwrap(), x);
                assert_eq!(x.project_root(), s);
                for op in PhaseState::ALL {
                    assert_eq!(x.t_s_compose(op).project_root(), compose(s, op));
                }
                assert_eq!(x.t_s_reflect().project_root(), reflect(s));
                assert_eq!(x.t_m().project_root(), s);
                assert_eq!(x.t_v(0xFF).project_root(), s);
                assert_eq!(x.t_v(0xFF).capability_class().as_u8(), 0x0F);
            }
        }
    }
    assert_eq!(
        TensorState::from_raw(256),
        Err(TensorStateError::ReservedBitsSet)
    );
}
