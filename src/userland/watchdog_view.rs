//! `watchdog` — security and statechain operator view.

use crate::kernel::Kernel;
use crate::platform::String;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::userland::terminal_fmt::{short_hex, status_word};
use crate::userland::terminal_metrics::{locked_object_count, stuck_phi2, stuck_phi3};
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::userland::terminal_metrics::{reflection_count, violation_count};

#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::state_proof::ObjectKind;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::statechain_watchdog::StatechainWatchdog;

pub struct WatchdogView {
    pub alert: bool,
    pub alert_object: Option<String>,
    pub alert_reason: Option<String>,
    pub action: String,
}

pub fn render(kernel: &Kernel) -> String {
    let view = analyze(kernel);
    let mut out = String::new();

    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        let dog = StatechainWatchdog::new();
        let audit = dog.audit(&kernel.statechain);
        let cp_count = kernel.statechain.checkpoints().len();
        let cp_ok = audit.signed_checkpoints_ok || cp_count == 0;
        let broken = broken_chain_count(kernel);
        let invalid = violation_count(kernel);
        let reflections = reflection_count(kernel);

        out.push_str(&format!(
            "EquationRoot:       {}\n",
            status_word(audit.equation_ok)
        ));
        out.push_str(&format!(
            "KernelStateRoot:    {}\n",
            status_word(audit.kernel_root_ok)
        ));
        out.push_str(&format!(
            "Statechain audit:   {}\n",
            if audit.tamper_detected { "FAIL" } else { "OK" }
        ));
        out.push_str(&format!(
            "Checkpoints:        {cp_count:03} {}\n",
            status_word(cp_ok)
        ));
        out.push_str(&format!("Broken chains:      {broken}\n"));
        out.push_str(&format!("Invalid proofs:     {invalid}\n"));
        out.push_str(&format!("Reflection spikes:  {reflections}\n"));
        out.push_str(&format!("Stuck Phi2:         {}\n", stuck_phi2(kernel)));
        out.push_str(&format!("Stuck Phi3:         {}\n", stuck_phi3(kernel)));
        let clock = kernel.get_clock();
        out.push_str(&format!(
            "StateClock skew:    {} (proper {} state {} sector {})\n",
            clock.skew(),
            clock.proper_tick,
            clock.state_tick,
            clock.sector_id,
        ));
        out.push_str(&format!(
            "Locked objects:     {}\n",
            locked_object_count(kernel)
        ));
        out.push_str(&format!("Action:             {}\n", view.action));
        if kernel.authority.is_anom_protected() {
            out.push_str("watchdog: active\n");
            out.push_str("anom: active\n");
            out.push_str("public phase states: masked\n");
            out.push_str("kernel decryptor: armed\n");
            out.push_str("chain proof: OK\n");
            #[cfg(feature = "std")]
            if let Some(session) = kernel.anom.session() {
                out.push_str(&format!(
                    "last anom event: {}\n",
                    short_hex(&session.last_anom_event_hash)
                ));
            }
        }
    }

    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    {
        out.push_str("EquationRoot:       N/A (hosted build)\n");
        out.push_str("KernelStateRoot:    N/A (hosted build)\n");
        out.push_str("Statechain audit:   N/A\n");
        out.push_str("Checkpoints:        000 N/A\n");
        out.push_str(&format!("Stuck Phi2:         {}\n", stuck_phi2(kernel)));
        out.push_str(&format!("Stuck Phi3:         {}\n", stuck_phi3(kernel)));
        out.push_str(&format!(
            "Locked objects:     {}\n",
            locked_object_count(kernel)
        ));
        out.push_str(&format!("Action:             {}\n", view.action));
    }

    if view.alert {
        out.push_str("\nWATCHDOG ALERT\n");
        if let Some(obj) = &view.alert_object {
            out.push_str(&format!("object: {obj}\n"));
        }
        if let Some(reason) = &view.alert_reason {
            out.push_str(&format!("reason: {reason}\n"));
        }
        out.push_str(&format!("action: {}\n", view.action));
    }

    out
}

pub fn analyze(kernel: &Kernel) -> WatchdogView {
    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        let dog = StatechainWatchdog::new();
        let audit = dog.audit(&kernel.statechain);
        if audit.tamper_detected {
            return WatchdogView {
                alert: true,
                alert_object: Some(format!("kernel root {}", short_hex(&audit.stored_root))),
                alert_reason: Some(audit.message.clone()),
                action: "investigate".into(),
            };
        }
        if stuck_phi3(kernel) > 0 {
            for pid in kernel.process_ids() {
                if let Ok(p) = kernel.get_process(pid) {
                    if p.state == crate::core::gphase_core::PhaseState::Phi3 {
                        return WatchdogView {
                            alert: true,
                            alert_object: Some(format!("process {pid}")),
                            alert_reason: Some("stuck in kernel/locked too long".into()),
                            action: "quarantine".into(),
                        };
                    }
                }
            }
            return WatchdogView {
                alert: false,
                alert_object: None,
                alert_reason: None,
                action: "monitor".into(),
            };
        }
        WatchdogView {
            alert: false,
            alert_object: None,
            alert_reason: None,
            action: "ok".into(),
        }
    }

    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    {
        let action = if stuck_phi3(kernel) > 0 {
            String::from("monitor")
        } else {
            String::from("ok")
        };
        WatchdogView {
            alert: false,
            alert_object: None,
            alert_reason: None,
            action,
        }
    }
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn broken_chain_count(kernel: &Kernel) -> usize {
    let mut broken = 0usize;
    for pid in kernel.process_ids() {
        for kind in [ObjectKind::Process, ObjectKind::Syscall, ObjectKind::Vm] {
            if let Some(chain) = kernel.statechain.chain(kind, pid) {
                if chain.verify().is_err() {
                    broken += 1;
                }
            }
        }
    }
    for (page, _) in kernel.tag_report() {
        if let Some(chain) = kernel.statechain.chain(ObjectKind::MemoryPage, page) {
            if chain.verify().is_err() {
                broken += 1;
            }
        }
    }
    broken
}
