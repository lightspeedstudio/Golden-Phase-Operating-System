//! StateClock Phase 1 tests.

use kairos::kernel::state_proof::ObjectKind;
use kairos::stateclock::{
    geometry::{master_x, sector_from_phi, update_from_phi, DEFAULT_TICKS_PER_SECTOR},
    ClockEventKind, KairosEventKind, StateClock,
};
use kairos::userland::commands::state_cmd;
use kairos::userland::init::boot;
use kairos::userland::parser::{parse, Command};
use kairos::userland::shell::Shell;
use kairos::userland::Hosted;
use kairos::Kernel;

const TOL: f64 = 1e-9;

fn shell_ready(hosted: &mut Hosted) -> kairos::kernel::event_log::ProcessId {
    let pid = boot(hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();
    pid
}

#[test]
fn new_initializes_valid_fields() {
    let clock = StateClock::new();
    assert_eq!(clock.chronos_tick, 0);
    assert_eq!(clock.kairos_tick, 0);
    assert_eq!(clock.proper_tick, 0);
    assert_eq!(clock.state_tick, 0);
    assert_eq!(clock.dilation_factor, 1.0);
    assert_eq!(clock.skew(), 0);
    assert!(clock.sector_id < 10);
    assert!((clock.x - master_x(clock.phi)).abs() < TOL);
}

#[test]
fn advance_chronos_increments_chronos_tick() {
    let mut clock = StateClock::new();
    clock.advance_chronos();
    assert_eq!(clock.chronos_tick, 1);
}

#[test]
fn advance_chronos_increments_proper_tick() {
    let mut clock = StateClock::new();
    clock.advance_chronos();
    assert_eq!(clock.proper_tick, 1);
}

#[test]
fn advance_chronos_changes_phi() {
    let mut clock = StateClock::new();
    let phi0 = clock.phi;
    clock.advance_chronos();
    assert!((clock.phi - phi0).abs() > TOL);
}

#[test]
fn x_equals_two_cos_phi_over_five() {
    let mut clock = StateClock::new();
    for _ in 0..50 {
        clock.advance_chronos();
        let expected = master_x(clock.phi);
        assert!(
            (clock.x - expected).abs() < TOL,
            "x={} expected={}",
            clock.x,
            expected
        );
        assert!((clock.x - 2.0 * (clock.phi / 5.0).cos()).abs() < TOL);
    }
}

#[test]
fn sector_id_always_zero_to_nine() {
    let mut clock = StateClock::new();
    for _ in 0..200 {
        assert!(clock.sector_id < 10);
        clock.advance_chronos();
        assert!(clock.sector_id < 10);
        let from_phi = sector_from_phi(clock.phi);
        assert_eq!(clock.sector_id, from_phi);
    }
}

#[test]
fn epsilon_updates_from_phi_and_sector() {
    let mut clock = StateClock::new();
    for _ in 0..100 {
        let (_, epsilon, _) = update_from_phi(clock.phi);
        assert!((clock.epsilon - epsilon).abs() < TOL);
        clock.advance_chronos();
    }
}

#[test]
fn record_kairos_event_increments_kairos_tick() {
    let mut clock = StateClock::new();
    clock.record_kairos_event(KairosEventKind::Command, "test");
    assert_eq!(clock.kairos_tick, 1);
}

#[test]
fn kairos_tick_not_advanced_by_chronos_alone() {
    let mut clock = StateClock::new();
    clock.advance_chronos();
    clock.advance_chronos();
    assert_eq!(clock.kairos_tick, 0);
}

#[test]
fn set_dilation_factor_changes_value() {
    let mut clock = StateClock::new();
    clock.set_dilation_factor(0.75).unwrap();
    assert!((clock.dilation_factor - 0.75).abs() < TOL);
}

#[test]
fn invalid_dilation_factor_rejected() {
    let mut clock = StateClock::new();
    assert!(clock.set_dilation_factor(0.0).is_err());
    assert!(clock.set_dilation_factor(-1.0).is_err());
    assert_eq!(clock.dilation_factor, 1.0);
}

#[test]
fn skew_is_computed() {
    let mut clock = StateClock::new();
    clock.set_dilation_factor(0.5).unwrap();
    for _ in 0..10 {
        clock.advance_chronos();
    }
    assert_ne!(clock.skew(), 0);
    assert_eq!(
        clock.skew(),
        clock.state_tick as i64 - clock.proper_tick as i64
    );
}

#[test]
fn resync_reduces_skew() {
    let mut clock = StateClock::new();
    clock.set_dilation_factor(0.5).unwrap();
    for _ in 0..20 {
        clock.advance_chronos();
    }
    let before = clock.skew().abs();
    assert!(before > 0);
    clock.resync("test");
    assert_eq!(clock.skew(), 0);
}

#[test]
fn clock_event_records_tick_transitions() {
    let mut clock = StateClock::new();
    clock.advance_chronos();
    let events = clock.recent_events();
    let tick = events
        .iter()
        .find(|e| e.kind == ClockEventKind::ChronosTick)
        .expect("chronos event");
    assert_eq!(tick.previous_chronos_tick, 0);
    assert_eq!(tick.new_chronos_tick, 1);
    assert_eq!(tick.previous_kairos_tick, 0);
    assert_eq!(tick.new_kairos_tick, 0);
}

#[test]
fn state_command_output_contains_clock_fields() {
    let mut hosted = Hosted::new();
    let pid = shell_ready(&mut hosted);
    hosted.kernel.advance_chronos();

    hosted.execute_line(pid, "state").unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("GPOS StateClock"));
    assert!(out.contains("chronos:"));
    assert!(out.contains("kairos:"));
    assert!(out.contains("x:"));
    assert!(out.contains("sector_id:"));
    assert!(out.contains("dilation_factor:"));
    assert!(out.contains("skew:"));
}

#[test]
fn hosted_statechain_records_clock_event() {
    let mut kernel = Kernel::new();
    kernel.advance_chronos();
    let before = kernel
        .statechain
        .chain(
            ObjectKind::Driver,
            kairos::stateclock::CLOCK_STATECHAIN_OBJECT_ID,
        )
        .map(|c| c.events.len())
        .unwrap_or(0);
    kernel.resync_clock("test audit");
    let after = kernel
        .statechain
        .chain(
            ObjectKind::Driver,
            kairos::stateclock::CLOCK_STATECHAIN_OBJECT_ID,
        )
        .map(|c| c.events.len())
        .unwrap_or(0);
    assert!(after > before, "expected clock events on statechain");
}

#[test]
fn parser_recognizes_clock_commands() {
    assert_eq!(parse("state"), Command::State);
    assert_eq!(parse("clock"), Command::Clock);
    assert_eq!(parse("clock skew"), Command::ClockSkew);
    assert_eq!(
        parse("clock set-dilation 0.9"),
        Command::ClockSetDilation { value: 0.9 }
    );
    assert_eq!(parse("clock resync"), Command::ClockResync);
}

#[test]
fn render_clock_matches_snapshot() {
    let mut clock = StateClock::new();
    clock.advance_chronos();
    let text = state_cmd::render_clock(&clock);
    assert!(text.contains(&format!("tick: {}", clock.chronos_tick)));
    assert!(text.contains(&format!("sector_id: {}", clock.sector_id)));
}

#[test]
fn default_delta_phi_uses_r_ticks_per_sector() {
    let clock = StateClock::new();
    let expected = core::f64::consts::PI / (5.0 * DEFAULT_TICKS_PER_SECTOR as f64);
    assert!((clock.delta_phi - expected).abs() < TOL);
}
