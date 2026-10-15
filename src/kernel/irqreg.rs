//! IRQ and device registry — Subsystem 3 of the Eight Kernel Subsystems
//! build program (`feature = "irqreg"`). See
//! `fpga/zynq/evidence/KERNEL_MATH_CONTRACT_2026-08-12.md` and
//! `fpga/zynq/evidence/IRQREG_STATUS_2026-08-12.md` for the full design,
//! equations, and proof.
//!
//! Replaces the hardcoded if/else IRQ dispatch chain in
//! `board::zynq7000::gic::kairos_board_irq_dispatch` (kept, unchanged, under
//! `#[cfg(not(feature = "irqreg"))]`) with a bounded, constant-time
//! registration table:
//!
//! ```text
//! dispatch(q) runs IrqTable[q].handler only when
//!   D(q) = [q < IRQ_MAX] AND [registered_q] AND [enabled_q] AND [!storm_masked_q]
//! ```
//!
//! plus a power-of-two deferred-event ring using the same per-slot sequence
//! protocol as the math contract's IPC section (Writable(j,n) = [seq_j = n],
//! Readable(j,n) = [seq_j = n+1], Reusable(j,n) = [seq_j = n+N]), so the ISR
//! stays O(1) and allocation-free: acknowledge, run the handler, bump the
//! counter, enqueue, and EOI — nothing else. All formatting, logging, and
//! storm-window sampling happen outside the ISR: `check_storm_window` is
//! driven once per real private-timer period (itself an ordinary
//! *registered* handler, not extra ISR work), and `drain_event` is called
//! from non-ISR context (e.g. the `irqreg` shell command).
//!
//! Architecture-agnostic and `no_std`-safe: no board dependency, no
//! allocation, only fixed-size arrays and `fn()` pointers.
//! `board::zynq7000::gic` supplies the concrete IRQ IDs (G1P 61, GEM0 RX 54,
//! USB0 53, private-timer 29), the ISR wiring, and the real hardware
//! mask/unmask primitives.

#![allow(dead_code)] // several accessors are surfaced only via board glue / the `irqreg` shell command

/// Bound on registrable IRQ IDs: the Zynq-7000 GIC-400 (UG585) implements
/// ICDISER/ICDICER/ICDIPR banks 0..=2 (IDs 0..=95: 32 SGI/PPI + up to 64 SPI
/// on this part). See `board::zynq7000::gic`'s register map for the concrete
/// IDs actually wired (G1P 61, GEM0 RX 54, USB0 53, private timer 29).
pub const IRQ_MAX: u32 = 96;
/// Deferred-event ring capacity (power of two, per the math contract's
/// `j = n & (N-1)` indexing).
pub const RING_CAPACITY: usize = 32;
const RING_MASK: u64 = (RING_CAPACITY - 1) as u64;

/// A registered IRQ's bottom-half work. Must not allocate, format, or block —
/// it still runs with real hardware IRQs masked (single Cortex-A9 core).
pub type IrqHandlerFn = fn();

/// Generational identity for a registered IRQ slot: the (slot, generation)
/// shape from `fpga/zynq/evidence/KERNEL_MATH_CONTRACT_2026-08-12.md` Part
/// III / `kernel::captable::GenerationalHandle`, implemented locally here
/// because an IRQ "slot" is a hardware-fixed index (the IRQ number itself),
/// not a dynamically allocated free-list slot — importing `captable`'s type
/// would wrongly couple two independently-toggleable Cargo features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IrqHandle {
    pub irq: u32,
    pub generation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrqRegError {
    OutOfRange,
    AlreadyRegistered,
    NotRegistered,
    StaleGeneration,
}

impl core::fmt::Display for IrqRegError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            IrqRegError::OutOfRange => "irq id out of range",
            IrqRegError::AlreadyRegistered => "irq already registered",
            IrqRegError::NotRegistered => "irq not registered",
            IrqRegError::StaleGeneration => "stale irq handle generation",
        };
        f.write_str(msg)
    }
}

#[derive(Clone, Copy)]
struct IrqSlot {
    handler: Option<IrqHandlerFn>,
    generation: u32,
    enabled: bool,
    /// C_q(t): cumulative accepted-dispatch count. Never reset (storm-window
    /// delta accounting depends on monotonicity).
    count: u32,
    /// C_q value at the start of the current storm-detection window.
    window_baseline: u32,
    /// theta_q, expressed as an integer max delta-count per window (avoids
    /// floating point; equivalent to r_q(t) > theta_q scaled by the window
    /// width W, where W = one call to `check_storm_window`).
    max_per_window: Option<u32>,
    storm_masked: bool,
}

impl IrqSlot {
    const fn empty() -> Self {
        Self {
            handler: None,
            generation: 0,
            enabled: false,
            count: 0,
            window_baseline: 0,
            max_per_window: None,
            storm_masked: false,
        }
    }
}

#[derive(Clone, Copy)]
struct RingSlot {
    sequence: u64,
    irq: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IrqRegistryStats {
    /// Deferred-event ring writes rejected because the ring was full
    /// (bounded, counted — never an unbounded allocation).
    pub ring_dropped: u32,
    /// Dispatches rejected by `D(q)` (out-of-range, unregistered, disabled,
    /// or storm-masked).
    pub dispatch_rejected: u32,
    /// Deferred events produced but not yet drained.
    pub ring_pending: u32,
}

/// The IRQ and device registry. One instance is the real dispatch table for
/// a board; `board::zynq7000::gic` owns the actual singleton (see its
/// `registry()` accessor) because only board code can reach the ISR entry
/// point and the real hardware mask/unmask registers.
pub struct IrqRegistry {
    slots: [IrqSlot; IRQ_MAX as usize],
    ring: [RingSlot; RING_CAPACITY],
    /// n: next sequence number this producer (the ISR) will write.
    ring_next: u64,
    /// Next sequence number the consumer (`drain_event`) expects to read.
    ring_read: u64,
    ring_dropped: u32,
    dispatch_rejected: u32,
}

impl Default for IrqRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl IrqRegistry {
    pub fn new() -> Self {
        Self {
            slots: core::array::from_fn(|_| IrqSlot::empty()),
            // Vyukov-style bounded-ring seeding: slot j's sequence starts at
            // j so the first producer pass (n=j) sees Writable(j,j)=true.
            ring: core::array::from_fn(|j| RingSlot {
                sequence: j as u64,
                irq: 0,
            }),
            ring_next: 0,
            ring_read: 0,
            ring_dropped: 0,
            dispatch_rejected: 0,
        }
    }

    /// Register a handler for `irq`. Fails on out-of-range or already-live
    /// registration (the caller must `unregister` first). Returns the new
    /// slot's generation; the IRQ starts disabled — the caller must `enable`
    /// it explicitly (mirrors the existing board bring-up sequence, where
    /// hardware configuration and IRQ unmasking are already separate steps).
    pub fn register(&mut self, irq: u32, handler: IrqHandlerFn) -> Result<IrqHandle, IrqRegError> {
        if irq >= IRQ_MAX {
            return Err(IrqRegError::OutOfRange);
        }
        let slot = &mut self.slots[irq as usize];
        if slot.handler.is_some() {
            return Err(IrqRegError::AlreadyRegistered);
        }
        slot.handler = Some(handler);
        slot.enabled = false;
        slot.storm_masked = false;
        slot.max_per_window = None;
        slot.window_baseline = slot.count;
        Ok(IrqHandle {
            irq,
            generation: slot.generation,
        })
    }

    /// Revoke a registration and advance its generation so any handle
    /// captured before this call becomes stale (`ValidHandle` from the math
    /// contract's §6 generation-stamped identity). The slot may then be
    /// registered to a different handler.
    pub fn unregister(&mut self, handle: IrqHandle) -> Result<(), IrqRegError> {
        let idx = self.validate(handle)?;
        let slot = &mut self.slots[idx];
        slot.handler = None;
        slot.enabled = false;
        slot.storm_masked = false;
        slot.max_per_window = None;
        // Never wrap silently into a valid old generation: on the u32
        // boundary this saturates rather than reusing generation 0, which a
        // very first `register()` call also legitimately produces.
        slot.generation = slot.generation.checked_add(1).unwrap_or(u32::MAX);
        Ok(())
    }

    pub fn enable(&mut self, handle: IrqHandle) -> Result<(), IrqRegError> {
        let idx = self.validate(handle)?;
        self.slots[idx].enabled = true;
        Ok(())
    }

    pub fn disable(&mut self, handle: IrqHandle) -> Result<(), IrqRegError> {
        let idx = self.validate(handle)?;
        self.slots[idx].enabled = false;
        Ok(())
    }

    /// Configure (or clear, with `None`) the storm policy for `irq`:
    /// `max_per_window` is the maximum accepted-dispatch delta allowed
    /// between two consecutive `check_storm_window` calls before the slot is
    /// disabled in software. Resets any existing storm mask and rebaselines
    /// the window.
    pub fn set_storm_policy(
        &mut self,
        handle: IrqHandle,
        max_per_window: Option<u32>,
    ) -> Result<(), IrqRegError> {
        let idx = self.validate(handle)?;
        let slot = &mut self.slots[idx];
        slot.max_per_window = max_per_window;
        slot.storm_masked = false;
        slot.window_baseline = slot.count;
        Ok(())
    }

    /// Explicitly lift a storm mask and re-enable the IRQ. Never automatic —
    /// a storm mask is a safety policy, not a transient backoff.
    pub fn clear_storm_mask(&mut self, handle: IrqHandle) -> Result<(), IrqRegError> {
        let idx = self.validate(handle)?;
        let slot = &mut self.slots[idx];
        slot.storm_masked = false;
        slot.enabled = true;
        slot.window_baseline = slot.count;
        Ok(())
    }

    fn validate(&self, handle: IrqHandle) -> Result<usize, IrqRegError> {
        if handle.irq >= IRQ_MAX {
            return Err(IrqRegError::OutOfRange);
        }
        let idx = handle.irq as usize;
        let slot = &self.slots[idx];
        if slot.handler.is_none() {
            return Err(IrqRegError::NotRegistered);
        }
        if slot.generation != handle.generation {
            return Err(IrqRegError::StaleGeneration);
        }
        Ok(idx)
    }

    /// Hot ISR path. `D(q) = [q<IRQ_MAX] AND [registered_q] AND [enabled_q]
    /// AND [!storm_masked_q]`. O(1): one bounds check, one slot read, one
    /// indirect call, one counter increment, one bounded ring push. No
    /// allocation, no formatting, no hashing. Returns whether the handler
    /// ran (accepted dispatches only — duplicate/stale/disabled/unknown/
    /// out-of-range IRQs never reach the handler).
    pub fn dispatch(&mut self, irq: u32) -> bool {
        if irq >= IRQ_MAX {
            self.dispatch_rejected = self.dispatch_rejected.wrapping_add(1);
            return false;
        }
        let idx = irq as usize;
        let handler = {
            let slot = &self.slots[idx];
            if slot.handler.is_none() || !slot.enabled || slot.storm_masked {
                None
            } else {
                slot.handler
            }
        };
        let Some(handler) = handler else {
            self.dispatch_rejected = self.dispatch_rejected.wrapping_add(1);
            return false;
        };
        handler();
        self.slots[idx].count = self.slots[idx].count.wrapping_add(1);
        self.push_event(irq);
        true
    }

    /// Producer side of the deferred-event ring: `Writable(j,n) = [seq_j =
    /// n]`. Bounded — a full ring counts a drop rather than allocating or
    /// overwriting an undrained slot.
    fn push_event(&mut self, irq: u32) -> bool {
        let n = self.ring_next;
        let j = (n & RING_MASK) as usize;
        if self.ring[j].sequence != n {
            self.ring_dropped = self.ring_dropped.wrapping_add(1);
            return false;
        }
        self.ring[j] = RingSlot {
            sequence: n + 1,
            irq,
        };
        self.ring_next = n + 1;
        true
    }

    /// Consumer side of the deferred-event ring: `Readable(j,n) = [seq_j =
    /// n+1]`. After reading, marks the slot `Reusable(j,n) = [seq_j = n+N]`
    /// for the producer's next lap. Called from non-ISR context only.
    /// Returns `(sequence, irq)` in strict FIFO order.
    pub fn drain_event(&mut self) -> Option<(u64, u32)> {
        let n = self.ring_read;
        let j = (n & RING_MASK) as usize;
        if self.ring[j].sequence != n + 1 {
            return None;
        }
        let irq = self.ring[j].irq;
        self.ring[j].sequence = n + RING_CAPACITY as u64;
        self.ring_read = n + 1;
        Some((n, irq))
    }

    /// Sample every registered slot's dispatch count once and compare its
    /// delta since the last call against `max_per_window`
    /// (`r_q(t) = (C_q(t)-C_q(t-W))/W`, evaluated as an integer delta so W is
    /// implicitly "one call to this function" — driven by the real
    /// private-timer period in `board::zynq7000::gic`, never from inside the
    /// hot ISR path). A slot whose delta exceeds its threshold is disabled
    /// in software and its handle is written into `out` so the caller can
    /// also mask it at the real hardware distributor. Returns the number of
    /// newly-masked slots (bounded by `out.len()`; extra masks in the same
    /// call are still applied, just not reported if `out` is too small).
    pub fn check_storm_window(&mut self, out: &mut [IrqHandle]) -> usize {
        let mut reported = 0usize;
        for i in 0..IRQ_MAX as usize {
            let slot = &mut self.slots[i];
            if let Some(threshold) = slot.max_per_window {
                let delta = slot.count.wrapping_sub(slot.window_baseline);
                slot.window_baseline = slot.count;
                if delta > threshold && !slot.storm_masked {
                    slot.storm_masked = true;
                    slot.enabled = false;
                    if reported < out.len() {
                        out[reported] = IrqHandle {
                            irq: i as u32,
                            generation: slot.generation,
                        };
                        reported += 1;
                    }
                }
            }
        }
        reported
    }

    pub fn is_registered(&self, irq: u32) -> bool {
        (irq as usize) < IRQ_MAX as usize && self.slots[irq as usize].handler.is_some()
    }

    pub fn is_enabled(&self, irq: u32) -> bool {
        (irq as usize) < IRQ_MAX as usize && self.slots[irq as usize].enabled
    }

    pub fn is_storm_masked(&self, irq: u32) -> bool {
        (irq as usize) < IRQ_MAX as usize && self.slots[irq as usize].storm_masked
    }

    pub fn slot_count(&self, irq: u32) -> Option<u32> {
        if irq >= IRQ_MAX {
            return None;
        }
        Some(self.slots[irq as usize].count)
    }

    pub fn stats(&self) -> IrqRegistryStats {
        IrqRegistryStats {
            ring_dropped: self.ring_dropped,
            dispatch_rejected: self.dispatch_rejected,
            ring_pending: (self.ring_next - self.ring_read) as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicU32, Ordering};

    static HANDLER_A_HITS: AtomicU32 = AtomicU32::new(0);
    fn handler_a() {
        HANDLER_A_HITS.fetch_add(1, Ordering::SeqCst);
    }

    static HANDLER_B_HITS: AtomicU32 = AtomicU32::new(0);
    fn handler_b() {
        HANDLER_B_HITS.fetch_add(1, Ordering::SeqCst);
    }

    fn reset_counters() {
        HANDLER_A_HITS.store(0, Ordering::SeqCst);
        HANDLER_B_HITS.store(0, Ordering::SeqCst);
    }

    #[test]
    fn out_of_range_register_and_dispatch_are_rejected() {
        let mut reg = IrqRegistry::new();
        assert_eq!(
            reg.register(IRQ_MAX, handler_a),
            Err(IrqRegError::OutOfRange)
        );
        let before = reg.stats();
        assert!(!reg.dispatch(IRQ_MAX));
        assert_eq!(reg.stats().dispatch_rejected, before.dispatch_rejected + 1);
    }

    #[test]
    fn duplicate_register_fails_without_disturbing_the_live_handler() {
        reset_counters();
        let mut reg = IrqRegistry::new();
        let h = reg.register(10, handler_a).expect("first register");
        assert_eq!(
            reg.register(10, handler_b),
            Err(IrqRegError::AlreadyRegistered)
        );
        assert!(reg.enable(h).is_ok());
        assert!(reg.dispatch(10));
        assert_eq!(HANDLER_A_HITS.load(Ordering::SeqCst), 1);
        assert_eq!(HANDLER_B_HITS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn usb0_irq_53_is_a_first_class_registry_slot() {
        let mut reg = IrqRegistry::new();
        for irq in [29u32, 53, 54, 61] {
            let h = reg.register(irq, handler_b).expect("register");
            reg.enable(h).expect("enable");
            assert!(reg.is_registered(irq));
            assert!(reg.is_enabled(irq));
        }
        assert!(reg.dispatch(53));
        assert_eq!(reg.slot_count(53), Some(1));
        assert_eq!(reg.slot_count(29), Some(0));
        assert_eq!(reg.slot_count(54), Some(0));
        assert_eq!(reg.slot_count(61), Some(0));
    }

    #[test]
    fn unknown_irq_never_dispatches() {
        reset_counters();
        let mut reg = IrqRegistry::new();
        assert!(!reg.dispatch(11));
        assert_eq!(HANDLER_A_HITS.load(Ordering::SeqCst), 0);
        assert_eq!(reg.stats().dispatch_rejected, 1);
    }

    #[test]
    fn disabled_irq_never_dispatches() {
        reset_counters();
        let mut reg = IrqRegistry::new();
        let _h = reg.register(12, handler_a).expect("register");
        // Not enabled yet.
        assert!(!reg.dispatch(12));
        assert_eq!(HANDLER_A_HITS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn real_dispatch_runs_handler_increments_count_and_enqueues_event() {
        reset_counters();
        let mut reg = IrqRegistry::new();
        let h = reg.register(13, handler_a).expect("register");
        reg.enable(h).expect("enable");
        assert!(reg.dispatch(13));
        assert_eq!(HANDLER_A_HITS.load(Ordering::SeqCst), 1);
        assert_eq!(reg.slot_count(13), Some(1));
        let (seq, irq) = reg.drain_event().expect("event present");
        assert_eq!(seq, 0);
        assert_eq!(irq, 13);
        assert!(reg.drain_event().is_none());
    }

    #[test]
    fn stale_generation_after_unregister_and_reregister_is_rejected() {
        let mut reg = IrqRegistry::new();
        let h_old = reg.register(14, handler_a).expect("register");
        reg.unregister(h_old).expect("unregister");
        // The bare NotRegistered case: nothing occupies slot 14 right now.
        assert_eq!(reg.enable(h_old), Err(IrqRegError::NotRegistered));
        let h_new = reg.register(14, handler_b).expect("re-register");
        assert_ne!(h_old.generation, h_new.generation);
        // Now the slot IS occupied again, but by a different generation —
        // the old handle must be rejected as stale, not merely "not found".
        assert_eq!(reg.enable(h_old), Err(IrqRegError::StaleGeneration));
        assert!(reg.enable(h_new).is_ok());
    }

    #[test]
    fn ring_full_drops_are_bounded_and_counted_then_fifo_drains_in_order() {
        reset_counters();
        let mut reg = IrqRegistry::new();
        let h = reg.register(15, handler_a).expect("register");
        reg.enable(h).expect("enable");
        for _ in 0..(RING_CAPACITY + 5) {
            reg.dispatch(15);
        }
        assert_eq!(
            HANDLER_A_HITS.load(Ordering::SeqCst),
            (RING_CAPACITY + 5) as u32
        );
        assert_eq!(reg.stats().ring_dropped, 5);
        let mut prev_seq: Option<u64> = None;
        let mut drained = 0;
        while let Some((seq, irq)) = reg.drain_event() {
            assert_eq!(irq, 15);
            if let Some(p) = prev_seq {
                assert_eq!(seq, p + 1);
            }
            prev_seq = Some(seq);
            drained += 1;
        }
        assert_eq!(drained, RING_CAPACITY);
    }

    #[test]
    fn storm_policy_masks_after_threshold_and_clear_mask_restores_dispatch() {
        reset_counters();
        let mut reg = IrqRegistry::new();
        let h = reg.register(16, handler_a).expect("register");
        reg.enable(h).expect("enable");
        reg.set_storm_policy(h, Some(3)).expect("set storm policy");

        for _ in 0..4 {
            assert!(reg.dispatch(16));
        }
        assert_eq!(HANDLER_A_HITS.load(Ordering::SeqCst), 4);
        assert!(!reg.is_storm_masked(16));

        let mut out = [IrqHandle::default(); 4];
        let n = reg.check_storm_window(&mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0].irq, 16);
        assert!(reg.is_storm_masked(16));
        assert!(!reg.is_enabled(16));

        // Real behavior change: dispatch genuinely stops running the handler
        // while storm-masked, not a manufactured PASS string.
        let before = reg.stats().dispatch_rejected;
        assert!(!reg.dispatch(16));
        assert_eq!(HANDLER_A_HITS.load(Ordering::SeqCst), 4);
        assert_eq!(reg.stats().dispatch_rejected, before + 1);

        reg.clear_storm_mask(out[0]).expect("clear storm mask");
        assert!(!reg.is_storm_masked(16));
        assert!(reg.dispatch(16));
        assert_eq!(HANDLER_A_HITS.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn quiet_irq_under_threshold_is_never_storm_masked() {
        reset_counters();
        let mut reg = IrqRegistry::new();
        let h = reg.register(17, handler_a).expect("register");
        reg.enable(h).expect("enable");
        reg.set_storm_policy(h, Some(100))
            .expect("set storm policy");
        for _ in 0..10 {
            reg.dispatch(17);
        }
        let mut out = [IrqHandle::default(); 4];
        assert_eq!(reg.check_storm_window(&mut out), 0);
        assert!(!reg.is_storm_masked(17));
        assert!(reg.dispatch(17));
    }
}
