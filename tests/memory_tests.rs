use kairos::core::gphase_core::{compose, PhaseState};
use kairos::Kernel;

fn page_at(k: &mut Kernel, tag: PhaseState) -> u32 {
    let pid = k.alloc_page();
    match tag {
        PhaseState::Phi0 => {}
        PhaseState::Phi1 => k.map_user(pid).unwrap(),
        PhaseState::Phi2 => k.map_kernel(pid).unwrap(),
        PhaseState::Phi3 => {
            k.map_kernel(pid).unwrap();
            k.lock_page(pid).unwrap();
        }
    }
    pid
}

#[test]
fn alloc_starts_phi0() {
    let mut k = Kernel::new();
    let page = k.alloc_page();
    assert_eq!(k.tag_report()[&page], PhaseState::Phi0);
}

#[test]
fn full_page_lifecycle() {
    let mut k = Kernel::new();
    let page = k.alloc_page();
    k.map_user(page).unwrap();
    k.escalate(page).unwrap();
    k.lock_page(page).unwrap();
    k.unlock_page(page).unwrap();
    k.unmap_kernel(page).unwrap();
    k.free_page(page).unwrap();
    assert!(!k.tag_report().contains_key(&page));
}

#[test]
fn cannot_free_mapped_or_locked() {
    for tag in [PhaseState::Phi1, PhaseState::Phi2, PhaseState::Phi3] {
        let mut k = Kernel::new();
        let page = page_at(&mut k, tag);
        assert!(k.free_page(page).is_err());
    }
}

#[test]
fn memory_contention_escalates_via_compose() {
    let mut k = Kernel::new();
    let user = page_at(&mut k, PhaseState::Phi1);
    let kernel = page_at(&mut k, PhaseState::Phi2);
    assert_eq!(k.memory_arbitrate(user, kernel).unwrap(), PhaseState::Phi3);
    assert_eq!(
        k.memory_arbitrate(user, user).unwrap(),
        compose(PhaseState::Phi1, PhaseState::Phi1)
    );
    let locked_a = page_at(&mut k, PhaseState::Phi3);
    let locked_b = page_at(&mut k, PhaseState::Phi3);
    assert_eq!(
        k.memory_arbitrate(locked_a, locked_b).unwrap(),
        PhaseState::Phi0
    );
}
