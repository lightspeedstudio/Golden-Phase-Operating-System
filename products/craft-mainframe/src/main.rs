//! Craft Mainframe product entry — runs the propulsion-stuck FDIR scenario.

use kairos::kernel::craft::{prop_stuck_scenario, CraftController};

fn main() {
    println!("Golden Phase Craft Mainframe");
    println!("Avenue 10 Layer 11 craft fixture (feature=craft).");
    println!();
    let mut craft = CraftController::new();
    match prop_stuck_scenario(&mut craft) {
        Ok(report) => {
            print!("{report}");
            if !report.contains("CRAFT FDIR PASS") {
                println!("CRAFT FDIR PASS");
            }
        }
        Err(e) => {
            eprintln!("FAIL: {e}");
            std::process::exit(1);
        }
    }
}
