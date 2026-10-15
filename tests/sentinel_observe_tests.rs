use kairos::userland::init::boot;
use kairos::userland::Hosted;

#[test]
fn observe_counts_spawn_and_fs_not_per_instruction() {
    let mut hosted = Hosted::new();
    let caller = boot(&mut hosted);
    let before = hosted.kernel.sentinel.observe_count();
    hosted.execute_line(caller, "spawn worker").unwrap();
    hosted.execute_line(caller, "mkdir /snt").unwrap();
    let after = hosted.kernel.sentinel.observe_count();
    assert!(after > before);
    assert!(after < before + 10_000);
}

#[test]
fn o_sentinel_is_measured() {
    let mut hosted = Hosted::new();
    let caller = boot(&mut hosted);
    hosted.execute_line(caller, "mkdir /snt-obs").unwrap();
    let n = hosted.kernel.sentinel.observe_count();
    assert!(n > 0);
    assert!(n < u64::MAX);
}

#[test]
fn mmio_denied_for_programs() {
    let err = kairos::kernel::sentinel::authorize_mmio(1).unwrap_err();
    assert_eq!(err, kairos::kernel::sentinel::SentinelError::MmioDenied);
}
