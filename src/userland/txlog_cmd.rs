//! `txlog` — observe `kernel.txlog`'s live counters, and demonstrate real
//! multi-op atomic transactions / fault injection / recovery (subsystem 7)
//! for on-board evidence capture.

use crate::kernel::Kernel;
use crate::platform::String;

pub fn render(kernel: &Kernel) -> String {
    #[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
    {
        let stats = kernel.txlog.stats();
        format!(
            "txlog\n\
             records: {}\n\
             committed: {}\n\
             rolled_back: {}\n\
             isolated: {}\n\
             recoveries: {}\n\
             faults_injected: {}",
            stats.records,
            stats.committed,
            stats.rolled_back,
            stats.isolated,
            stats.recoveries,
            stats.faults_injected,
        )
    }
    #[cfg(not(all(feature = "txlog", any(feature = "std", feature = "baremetal"))))]
    {
        let _ = kernel;
        "txlog unavailable (txlog feature not enabled)".into()
    }
}

/// Run one real 3-op atomic transaction (`write_file` + `mkdir` + `rm`) and
/// report Prepared/Writing/Verified/Committed evidence — required-proof
/// bullet "multi-block/multi-op files round-trip exactly."
#[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
pub fn demo(kernel: &mut Kernel) -> String {
    use crate::kernel::txlog::TxOp;
    let tick = kernel.clock.kairos_tick;
    let sector = kernel.clock.sector_id;
    let _ = kernel.fs.write_file("/txlog_demo_stale.txt", b"stale");
    let ops = vec![
        TxOp::WriteFile {
            path: "/txlog_demo.txt".into(),
            data: b"txlog atomic demo".to_vec(),
        },
        TxOp::Mkdir {
            path: "/txlog_demo_dir".into(),
        },
        TxOp::Rm {
            path: "/txlog_demo_stale.txt".into(),
        },
    ];
    let crate::kernel::Kernel {
        fs,
        statechain,
        txlog,
        ..
    } = kernel;
    match txlog.run_transaction(fs, statechain, tick, sector, ops) {
        Ok((id, outcome)) => {
            let record = txlog.record(id).expect("just-run record exists");
            format!(
                "txlog demo: id={} outcome={:?} state={:?}\n\
                 root_old={:02x?}\n\
                 root_candidate={:02x?}\n\
                 verify_record={} verify_intent={}",
                id,
                outcome,
                record.state,
                &record.root_old[..4],
                &record.root_candidate[..4],
                txlog.verify_record(fs, id).unwrap_or(false),
                txlog.verify_intent(id).unwrap_or(false),
            )
        }
        Err(e) => format!("txlog demo: FAILED err={e}"),
    }
}

#[cfg(not(all(feature = "txlog", any(feature = "std", feature = "baremetal"))))]
pub fn demo(_kernel: &mut Kernel) -> String {
    "txlog unavailable (txlog feature not enabled)".into()
}

/// Run a real 2-op transaction with an injected fault after op 0 completes
/// (op0's write really executes, then op1 is never attempted) and report
/// the real rollback — required-proof bullet "inject failure after every
/// transaction state ... reopen/recover to exactly old ... state."
#[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
pub fn fault(kernel: &mut Kernel) -> String {
    use crate::kernel::txlog::{InjectedFault, TxOp};
    let tick = kernel.clock.kairos_tick;
    let sector = kernel.clock.sector_id;
    let _ = kernel.fs.write_file("/txlog_fault_target.txt", b"original");
    let ops = vec![
        TxOp::WriteFile {
            path: "/txlog_fault_target.txt".into(),
            data: b"overwritten".to_vec(),
        },
        TxOp::Mkdir {
            path: "/txlog_fault_never_created".into(),
        },
    ];
    let crate::kernel::Kernel {
        fs,
        statechain,
        txlog,
        ..
    } = kernel;
    match txlog.run_transaction_with_fault(
        fs,
        statechain,
        tick,
        sector,
        ops,
        Some(InjectedFault::StopAfterOp(1)),
    ) {
        Ok((id, outcome)) => {
            let restored = fs.read_file("/txlog_fault_target.txt").unwrap_or_default();
            format!(
                "txlog fault: id={} outcome={:?} restored_content={:?}",
                id,
                outcome,
                crate::platform::String::from_utf8_lossy(&restored),
            )
        }
        Err(e) => format!("txlog fault: FAILED err={e}"),
    }
}

#[cfg(not(all(feature = "txlog", any(feature = "std", feature = "baremetal"))))]
pub fn fault(_kernel: &mut Kernel) -> String {
    "txlog unavailable (txlog feature not enabled)".into()
}

/// Simulate a software-reset-interrupted transaction left in `Verified`
/// (all writes real and already matching `root_candidate`, but the
/// COMMITTED ledger append was lost), then call `recover_one` twice to
/// demonstrate forward completion and idempotence — required-proof bullets
/// "reopen/recover to exactly ... new state" and "replay twice ...
/// idempotence."
///
/// Relies on [`crate::kernel::txlog::Txlog::simulate_interrupted_transaction_for_test`],
/// which only exists under `#[cfg(any(test, debug_assertions))]` — a real
/// mid-transaction reset cannot be manufactured from a shell command without
/// that explicit test-only hook (same precedent as `PhaseFs::
/// set_pending_journal_for_test`). This means `txlog recover` is unavailable
/// in a release (`--release`, `debug_assertions` off) board build; explicit
/// non-claim, not exercised on real hardware. `recover_one`/`recover_all`
/// themselves are proven host-side (`HOST_UNIT_TEST`,
/// `recover_from_prepared_rolls_back_and_is_idempotent`/
/// `recover_from_writing_partial_rolls_back_both_ops`/
/// `recover_from_verified_commits_forward_and_is_idempotent`).
#[cfg(all(
    feature = "txlog",
    any(feature = "std", feature = "baremetal"),
    any(test, debug_assertions)
))]
pub fn recover(kernel: &mut Kernel) -> String {
    use crate::kernel::txlog::TxOp;
    let tick = kernel.clock.kairos_tick;
    let sector = kernel.clock.sector_id;
    let ops = vec![TxOp::WriteFile {
        path: "/txlog_recover_demo.txt".into(),
        data: b"recovered forward".to_vec(),
    }];
    let crate::kernel::Kernel {
        fs,
        statechain,
        txlog,
        ..
    } = kernel;
    let id = match txlog.simulate_interrupted_transaction_for_test(fs, tick, sector, ops, 1, true) {
        Ok(id) => id,
        Err(e) => return format!("txlog recover: FAILED to simulate reset err={e}"),
    };
    let generation = txlog.record(id).expect("record exists").generation;
    let first = txlog.recover_one(fs, statechain, id, generation);
    let second = txlog.recover_one(fs, statechain, id, generation);
    format!(
        "txlog recover: id={} first={:?} second={:?} content={:?}",
        id,
        first,
        second,
        crate::platform::String::from_utf8_lossy(
            &fs.read_file("/txlog_recover_demo.txt").unwrap_or_default()
        ),
    )
}

#[cfg(not(all(
    feature = "txlog",
    any(feature = "std", feature = "baremetal"),
    any(test, debug_assertions)
)))]
pub fn recover(_kernel: &mut Kernel) -> String {
    #[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
    {
        "txlog recover: unavailable in a release build (debug_assertions off) — \
         requires the test-only interrupted-transaction hook, see kernel::txlog::txlog_cmd docs"
            .into()
    }
    #[cfg(not(all(feature = "txlog", any(feature = "std", feature = "baremetal"))))]
    {
        "txlog unavailable (txlog feature not enabled)".into()
    }
}
