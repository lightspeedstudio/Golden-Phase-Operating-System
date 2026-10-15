//! Avenue 10 craft shell parser / scenario wiring.

use kairos::userland::parser::{parse, Command};

#[test]
fn parse_craft_commands() {
    assert_eq!(parse("craft init"), Command::CraftInit);
    assert_eq!(parse("craft status"), Command::CraftStatus);
    assert_eq!(parse("craft tick 3"), Command::CraftTick { n: 3 });
    assert_eq!(
        parse("craft command prop arm"),
        Command::CraftCommand {
            subsystem: "prop".into(),
            operation: "arm".into(),
        }
    );
    assert_eq!(
        parse("craft fault inject prop-stuck --reason stuck"),
        Command::CraftFaultInject {
            reason: "stuck".into(),
        }
    );
    assert_eq!(
        parse("craft quarantine status"),
        Command::CraftQuarantineStatus
    );
    assert_eq!(
        parse("scenario run prop-stuck"),
        Command::ScenarioRun {
            name: "prop-stuck".into(),
        }
    );
    assert_eq!(
        parse("scenario run prop-stuck-glider"),
        Command::ScenarioRun {
            name: "prop-stuck-glider".into(),
        }
    );
}

#[test]
fn scenario_api_pass() {
    let mut c = kairos::kernel::craft::CraftController::new();
    let report = kairos::kernel::craft::prop_stuck_scenario(&mut c).unwrap();
    assert!(report.contains("CRAFT FDIR PASS"));
}

#[test]
fn scenario_glider_api_pass() {
    let mut c = kairos::kernel::craft::CraftController::new_over_glider()
        .expect("controller over glider carrier");
    let report = kairos::kernel::craft::prop_stuck_over_glider_scenario(&mut c).unwrap();
    assert!(report.contains("CRAFT FDIR PASS"));
    assert!(report.contains("CRAFT GLIDER SCENARIO PASS"));
}
