//! `netsess` — observe `kernel.netsess`'s live counters, and demonstrate real
//! session open/close (wrapped in a real `captable::ObjectKind::Network`
//! capability, per this subsystem's own declared `netsess = ["captable",
//! "sched2"]` dependency — see `kernel::netsess`'s module docs), replay
//! acceptance/rejection, descriptor ring push/drain/complete/fail, and RTT
//! estimation (subsystem 8) for on-board evidence capture. No new syscall is
//! introduced — see `kernel::netsess`'s module docs, "Non-goals"; the
//! `captable` mint/resolve/destroy round trip below is driven directly from
//! this shell command, exactly the layer the module docs describe.

use crate::kernel::event_log::ProcessId;
use crate::kernel::Kernel;
use crate::platform::String;

pub fn render(kernel: &Kernel) -> String {
    #[cfg(all(feature = "netsess", any(feature = "std", feature = "baremetal")))]
    {
        let stats = kernel.netsess.stats();
        format!(
            "netsess\n\
             sessions_live: {}\n\
             sessions_opened: {}\n\
             sessions_closed: {}\n\
             generation_retired: {}\n\
             replay_accepted: {}\n\
             replay_rejected: {}\n\
             descriptors_pushed: {}\n\
             descriptors_rejected_full: {}\n\
             descriptors_completed: {}\n\
             descriptors_failed: {}\n\
             gem_rx_recorded: {}\n\
             gem_rx_last_len: {}\n\
             gem_rx_rejected: {}\n\
             ring_len: {}",
            kernel.netsess.session_count(),
            stats.sessions_opened,
            stats.sessions_closed,
            stats.generation_retired,
            stats.replay_accepted,
            stats.replay_rejected,
            stats.descriptors_pushed,
            stats.descriptors_rejected_full,
            stats.descriptors_completed,
            stats.descriptors_failed,
            stats.gem_rx_recorded,
            kernel.netsess.gem_rx_last_len(),
            stats.gem_rx_rejected,
            kernel.netsess.ring_len(),
        )
    }
    #[cfg(not(all(feature = "netsess", any(feature = "std", feature = "baremetal"))))]
    {
        let _ = kernel;
        "netsess unavailable (netsess feature not enabled)".into()
    }
}

/// Open a real session, accept a strictly-increasing then a reordered
/// sequence, push/drain/complete one descriptor, and record one RTT sample —
/// required-proof bullets "session identity is deterministic", "in-order and
/// reordered-in-window packets are both accepted", "a pushed descriptor
/// round-trips through drain/complete", and "RTT estimator produces a real
/// `(srtt, rttvar, rto)` triple".
#[cfg(all(feature = "netsess", any(feature = "std", feature = "baremetal")))]
pub fn demo(kernel: &mut Kernel, caller: ProcessId) -> String {
    use crate::kernel::captable::{RIGHT_DELETE, RIGHT_READ, RIGHT_WRITE};
    use crate::kernel::netsess::SessionHandle;

    let boot_session_root = kernel.authority.boot_session_root;
    let handle = match kernel.netsess.open_session(
        1,
        [10, 0, 0, 1],
        [10, 0, 0, 2],
        0x4249_4747,
        boot_session_root,
    ) {
        Ok(h) => h,
        Err(e) => return format!("netsess demo: FAILED open_session err={e}"),
    };
    let session_id = kernel.netsess.session_id(handle).expect("just opened");

    // Wrap the real session in a real `captable::ObjectKind::Network`
    // capability (this subsystem's own declared dependency — see module
    // docs) and prove the mint -> resolve -> destroy round trip names the
    // exact same session, exactly `ipc`'s own `Queue`/`IpcHandle` relation
    // to `captable`.
    let cap = kernel.captable.open_network(
        handle.pack(),
        caller,
        RIGHT_READ | RIGHT_WRITE | RIGHT_DELETE,
        None,
    );
    let now_chronos = kernel.clock.chronos_tick;
    let resolved = kernel
        .captable
        .resolve_network(cap, caller, RIGHT_READ, now_chronos)
        .map(SessionHandle::unpack);

    let accept_10 = kernel.netsess.accept_sequence(handle, 10);
    let accept_9 = kernel.netsess.accept_sequence(handle, 9); // reordered, inside window

    let token = match kernel.netsess.push_descriptor(handle, 42) {
        Ok(t) => t,
        Err(e) => return format!("netsess demo: FAILED push_descriptor err={e}"),
    };
    let drained = kernel.netsess.drain_descriptor();
    let completed = kernel.netsess.complete_descriptor(token);

    let _ = kernel.netsess.record_rtt_sample(handle, 40);
    let (srtt, rttvar, rto) = kernel.netsess.record_rtt_sample(handle, 60).unwrap();

    let now_chronos = kernel.clock.chronos_tick;
    let destroyed = kernel
        .captable
        .destroy_network_via_cap(cap, caller, now_chronos)
        .map(SessionHandle::unpack);
    if let Ok(closed_handle) = destroyed {
        let _ = kernel.netsess.close_session(closed_handle);
    }

    format!(
        "netsess demo: handle=(slot={} gen={}) session_id={:02x?}\n\
         cap=(slot={} gen={}) resolved_matches_handle={} destroyed_matches_handle={}\n\
         accept_10={:?} accept_9_reordered={:?}\n\
         push_token={} drained={:?} complete={:?}\n\
         rtt: srtt={} rttvar={} rto={}",
        handle.slot,
        handle.generation,
        &session_id[..4],
        cap.slot,
        cap.generation,
        resolved == Ok(handle),
        destroyed == Ok(handle),
        accept_10,
        accept_9,
        token,
        drained,
        completed,
        srtt,
        rttvar,
        rto,
    )
}

#[cfg(not(all(feature = "netsess", any(feature = "std", feature = "baremetal"))))]
pub fn demo(_kernel: &mut Kernel, _caller: ProcessId) -> String {
    "netsess unavailable (netsess feature not enabled)".into()
}

/// Demonstrate the three real rejection/failure paths in one run: a replay
/// duplicate is rejected, a full ring backpressures the caller, and a failed
/// descriptor's ring slot is quarantined (never reused) — required-proof
/// bullets "duplicate sequence numbers are rejected, not silently accepted",
/// "a full descriptor ring rejects rather than overwrites", and "a failed
/// descriptor's slot never becomes writable again".
#[cfg(all(feature = "netsess", any(feature = "std", feature = "baremetal")))]
pub fn fault(kernel: &mut Kernel, caller: ProcessId) -> String {
    use crate::kernel::captable::{RIGHT_DELETE, RIGHT_READ, RIGHT_WRITE};
    use crate::kernel::netsess::{SessionHandle, RING_CAPACITY};

    let boot_session_root = kernel.authority.boot_session_root;
    let handle = match kernel.netsess.open_session(
        1,
        [10, 0, 0, 1],
        [10, 0, 0, 3],
        0x4641_554c,
        boot_session_root,
    ) {
        Ok(h) => h,
        Err(e) => return format!("netsess fault: FAILED open_session err={e}"),
    };
    let cap = kernel.captable.open_network(
        handle.pack(),
        caller,
        RIGHT_READ | RIGHT_WRITE | RIGHT_DELETE,
        None,
    );

    let _ = kernel.netsess.accept_sequence(handle, 5);
    let duplicate = kernel.netsess.accept_sequence(handle, 5);

    let fail_token = match kernel.netsess.push_descriptor(handle, 1) {
        Ok(t) => t,
        Err(e) => return format!("netsess fault: FAILED push_descriptor err={e}"),
    };
    let _ = kernel.netsess.drain_descriptor();
    let failed = kernel.netsess.fail_descriptor(fail_token);
    let refail_quarantined = kernel.netsess.fail_descriptor(fail_token);

    for i in 0..(RING_CAPACITY as u64 - 1) {
        let _ = kernel.netsess.push_descriptor(handle, i + 100);
    }
    let ring_full = kernel.netsess.push_descriptor(handle, 9999);

    let now_chronos = kernel.clock.chronos_tick;
    let destroyed = kernel
        .captable
        .destroy_network_via_cap(cap, caller, now_chronos)
        .map(SessionHandle::unpack);
    if let Ok(closed_handle) = destroyed {
        let _ = kernel.netsess.close_session(closed_handle);
    }

    format!(
        "netsess fault: duplicate_seq_5={:?}\n\
         fail_descriptor={:?} refail_on_quarantined_slot={:?}\n\
         ring_full_after_{}_pushes={:?}\n\
         cap_destroyed_matches_handle={}",
        duplicate,
        failed,
        refail_quarantined,
        RING_CAPACITY,
        ring_full,
        destroyed == Ok(handle),
    )
}

#[cfg(not(all(feature = "netsess", any(feature = "std", feature = "baremetal"))))]
pub fn fault(_kernel: &mut Kernel, _caller: ProcessId) -> String {
    "netsess unavailable (netsess feature not enabled)".into()
}
