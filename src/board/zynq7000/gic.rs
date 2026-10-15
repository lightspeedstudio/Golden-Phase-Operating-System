//! Zynq-7000 GIC (Distributor + CPU interface) for G1P IRQ delivery.
//!
//! Interrupt configuration confirmed from:
//! - `fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/g1p_irq_id.h` → SPI 29 / ID 61
//! - SDT `interrupts = <0 29 4>` → IRQ_TYPE_LEVEL_HIGH (4)
//! - REGISTER_MAP.md: fabric IRQ is level-high until ACK
//! - bring-up `main.c`: `XScuGic_SetPriorityTriggerType(..., 0xA0, 0x01)`

use super::g1p::{G1p, G1P_IRQ_ID, IRQ_PEND_DONE, IRQ_PEND_ERR};

/// Periphbase + offsets (Zynq-7000 UG585).
const GIC_DIST_BASE: u32 = 0xF8F0_1000;
const GIC_CPU_BASE: u32 = 0xF8F0_0100;

const ICDDCR: u32 = 0x000;
const ICDISER: u32 = 0x100;
const ICDICER: u32 = 0x180;
const ICDIPR: u32 = 0x400;
const ICDIPTR: u32 = 0x800;
const ICDICFR: u32 = 0xC00;

const ICCICR: u32 = 0x00;
const ICCPMR: u32 = 0x04;
const ICCIAR: u32 = 0x0C;
const ICCEOIR: u32 = 0x10;

const G1P_PRIORITY: u8 = 0xA0;
const SPURIOUS_ID: u32 = 1023;

/// GEM0 receive interrupt: shared peripheral interrupt (SPI) ID 54 on Zynq-7000.
pub const GEM0_IRQ_ID: u32 = 54;
const GEM_PRIORITY: u8 = 0xA0;
/// USB0 (ChipIdea/EHCI @ 0xE0002000): SPI ID 53. `XPS_USB0_INT_ID` in
/// vendor `xparameters_ps.h`.
pub const USB0_IRQ_ID: u32 = 53;
const USB_PRIORITY: u8 = 0xA0;
/// Cortex-A9 private timer: private-peripheral interrupt (PPI) ID 29.
pub const PRIV_TIMER_IRQ_ID: u32 = 29;
const TICK_PRIORITY: u8 = 0xB0;

// Cortex-A9 private timer (UG585 §B.29), periphbase + 0x600.
const PTIMER_LOAD: u32 = 0xF8F0_0600;
const PTIMER_CTRL: u32 = 0xF8F0_0608; // bit0 en, bit1 auto-reload, bit2 irq-en, [15:8] prescaler
const PTIMER_ISR: u32 = 0xF8F0_060C; // bit0 event flag (write 1 to clear)

#[cfg(not(feature = "irqreg"))]
static mut G1P_IRQ_COUNT: u32 = 0;
#[cfg(not(feature = "irqreg"))]
static mut GEM_RX_IRQ_COUNT: u32 = 0;
#[cfg(not(feature = "irqreg"))]
static mut TICK_COUNT: u32 = 0;

#[inline]
unsafe fn dist_read(off: u32) -> u32 {
    core::ptr::read_volatile((GIC_DIST_BASE + off) as *const u32)
}

#[inline]
unsafe fn dist_write(off: u32, val: u32) {
    core::ptr::write_volatile((GIC_DIST_BASE + off) as *mut u32, val);
}

#[inline]
unsafe fn cpu_read(off: u32) -> u32 {
    core::ptr::read_volatile((GIC_CPU_BASE + off) as *const u32)
}

#[inline]
unsafe fn cpu_write(off: u32, val: u32) {
    core::ptr::write_volatile((GIC_CPU_BASE + off) as *mut u32, val);
}

/// Configure one interrupt (SPI or PPI): priority, target CPU0, level-high,
/// and enable it in the distributor. Assumes the distributor + CPU interface
/// are (or will be) brought up by the caller.
unsafe fn configure_int(id: u32, priority: u8) {
    let bank = id / 32;
    let bit = id % 32;

    // Disable while programming.
    dist_write(ICDICER + bank * 4, 1 << bit);

    // Priority (byte per interrupt).
    let pri_off = ICDIPR + id;
    let pri_aligned = pri_off & !3;
    let pri_shift = (pri_off & 3) * 8;
    let mut pri_word = dist_read(pri_aligned);
    pri_word = (pri_word & !(0xFFu32 << pri_shift)) | ((priority as u32) << pri_shift);
    dist_write(pri_aligned, pri_word);

    // Target CPU0 (byte per interrupt; read-only/ignored for PPIs — harmless).
    let tgt_off = ICDIPTR + id;
    let tgt_aligned = tgt_off & !3;
    let tgt_shift = (tgt_off & 3) * 8;
    let mut tgt_word = dist_read(tgt_aligned);
    tgt_word = (tgt_word & !(0xFFu32 << tgt_shift)) | (0x01u32 << tgt_shift);
    dist_write(tgt_aligned, tgt_word);

    // Config: 2 bits per IRQ. Level-sensitive active-high = 0b01.
    let cfg_reg = ICDICFR + (id / 16) * 4;
    let cfg_shift = (id % 16) * 2;
    let mut cfg = dist_read(cfg_reg);
    cfg = (cfg & !(0b11 << cfg_shift)) | (0b01 << cfg_shift);
    dist_write(cfg_reg, cfg);

    // Enable this interrupt.
    dist_write(ICDISER + bank * 4, 1 << bit);
}

/// Enable CPSR IRQs (clear I bit). Call only after [`init_g1p_spi`].
pub fn enable_cpsr_irqs() {
    unsafe {
        core::arch::asm!("cpsie i", options(nomem, nostack));
    }
}

pub fn disable_cpsr_irqs() {
    unsafe {
        core::arch::asm!("cpsid i", options(nomem, nostack));
    }
}

// ---------------------------------------------------------------------
// feature = "irqreg" OFF: original hardcoded if/else dispatch (unchanged),
// preserved byte-for-byte so the default sealed boot never depends on the
// new registry. See fpga/zynq/evidence/IRQREG_STATUS_2026-08-12.md.
// ---------------------------------------------------------------------

/// Initialize distributor + CPU interface and configure G1P SPI (level-high).
/// Does **not** unmask CPSR I — caller enables IRQs after this returns.
#[cfg(not(feature = "irqreg"))]
pub fn init_g1p_spi() {
    unsafe {
        // Disable distributor while programming.
        dist_write(ICDDCR, 0);

        configure_int(G1P_IRQ_ID, G1P_PRIORITY);

        // CPU interface: mask none below 0xF0, enable signaling.
        cpu_write(ICCPMR, 0xF0);
        cpu_write(ICCICR, 0x01);

        // Enable distributor.
        dist_write(ICDDCR, 0x01);

        G1P_IRQ_COUNT = 0;
    }
}

/// Enable the GEM0 receive interrupt at the GIC (SPI 54) and at the GEM.
/// The GIC distributor/CPU interface must already be up ([`init_g1p_spi`]).
#[cfg(not(feature = "irqreg"))]
pub fn enable_gem0_rx_irq() {
    unsafe {
        configure_int(GEM0_IRQ_ID, GEM_PRIORITY);
        GEM_RX_IRQ_COUNT = 0;
    }
    super::gem::enable_rx_irq();
}

/// Start the Cortex-A9 private timer as a periodic tick (~`load` timer ticks per
/// period, auto-reload, IRQ enabled) and enable its GIC PPI. Bounds `wfi` sleep
/// so the serve loops stay live for smoltcp timers + the deadline check.
#[cfg(not(feature = "irqreg"))]
pub fn enable_tick(load: u32) {
    unsafe {
        configure_int(PRIV_TIMER_IRQ_ID, TICK_PRIORITY);
        TICK_COUNT = 0;
        core::ptr::write_volatile(PTIMER_LOAD as *mut u32, load);
        // enable | auto-reload | irq-enable, prescaler 0.
        core::ptr::write_volatile(PTIMER_CTRL as *mut u32, 0b111);
    }
}

/// Stop the private-timer tick and mask its PPI (leave GEM/G1P untouched).
#[cfg(not(feature = "irqreg"))]
pub fn disable_tick() {
    unsafe {
        core::ptr::write_volatile(PTIMER_CTRL as *mut u32, 0);
        dist_write(ICDICER, 1 << PRIV_TIMER_IRQ_ID);
    }
}

#[cfg(not(feature = "irqreg"))]
pub fn gem_rx_irq_count() -> u32 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(GEM_RX_IRQ_COUNT)) }
}

#[cfg(not(feature = "irqreg"))]
pub fn tick_count() -> u32 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(TICK_COUNT)) }
}

/// Clear the software IRQ counter (IRQ-safe on single-core A9).
#[cfg(not(feature = "irqreg"))]
pub fn clear_irq_count() {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(G1P_IRQ_COUNT), 0);
    }
}

#[cfg(not(feature = "irqreg"))]
pub fn irq_count() -> u32 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(G1P_IRQ_COUNT)) }
}

/// Rust IRQ dispatch entered from `startup.S`. Acknowledges GIC + G1P.
#[cfg(not(feature = "irqreg"))]
#[no_mangle]
pub extern "C" fn kairos_board_irq_dispatch() {
    let iar = unsafe { cpu_read(ICCIAR) };
    let id = iar & 0x3FF;
    if id == SPURIOUS_ID {
        return;
    }
    if id == G1P_IRQ_ID {
        let g = G1p::default_window();
        let pend = g.irq_pending();
        if (pend & (IRQ_PEND_DONE | IRQ_PEND_ERR)) != 0 {
            g.ack_irq(pend & (IRQ_PEND_DONE | IRQ_PEND_ERR));
        }
        unsafe {
            let c = core::ptr::read_volatile(core::ptr::addr_of!(G1P_IRQ_COUNT));
            core::ptr::write_volatile(core::ptr::addr_of_mut!(G1P_IRQ_COUNT), c.wrapping_add(1));
        }
    } else if id == GEM0_IRQ_ID {
        // One-shot: the GEM handler masks its RX interrupt and sets RX_READY;
        // the serve poll loop drains frames and re-arms before sleeping.
        super::gem::on_rx_irq();
        unsafe {
            let c = core::ptr::read_volatile(core::ptr::addr_of!(GEM_RX_IRQ_COUNT));
            core::ptr::write_volatile(core::ptr::addr_of_mut!(GEM_RX_IRQ_COUNT), c.wrapping_add(1));
        }
    } else if id == PRIV_TIMER_IRQ_ID {
        // Clear the private-timer event flag (write 1 to bit 0).
        unsafe { core::ptr::write_volatile(PTIMER_ISR as *mut u32, 1) };
        unsafe {
            let c = core::ptr::read_volatile(core::ptr::addr_of!(TICK_COUNT));
            core::ptr::write_volatile(core::ptr::addr_of_mut!(TICK_COUNT), c.wrapping_add(1));
        }
    }
    unsafe { cpu_write(ICCEOIR, iar) };
}

// ---------------------------------------------------------------------
// feature = "irqreg" ON: dispatch through kernel::irqreg::IrqRegistry.
// Same hardware configure_int() sequence and EOI discipline as the
// feature-off path. Wired IDs: G1P 61, GEM0 RX 54, USB0 53, private
// timer 29. See fpga/zynq/evidence/IRQREG_STATUS_2026-08-12.md.
// ---------------------------------------------------------------------

#[cfg(feature = "irqreg")]
use crate::kernel::irqreg::{IrqHandle, IrqRegistry};

#[cfg(feature = "irqreg")]
static mut REGISTRY: Option<IrqRegistry> = None;
#[cfg(feature = "irqreg")]
static mut G1P_HANDLE: Option<IrqHandle> = None;
#[cfg(feature = "irqreg")]
static mut GEM_HANDLE: Option<IrqHandle> = None;
#[cfg(feature = "irqreg")]
static mut TIMER_HANDLE: Option<IrqHandle> = None;
#[cfg(all(feature = "irqreg", feature = "usb"))]
static mut USB_HANDLE: Option<IrqHandle> = None;

/// The board's single IRQ registry instance. Lazily created on first use
/// (always from `init_g1p_spi`, which runs once at boot before CPSR IRQs are
/// unmasked) — safe under the same single-core, sequential-boot assumption
/// the rest of this file already relies on for its raw `static mut` counters.
#[cfg(feature = "irqreg")]
fn registry() -> &'static mut IrqRegistry {
    unsafe {
        let ptr = core::ptr::addr_of_mut!(REGISTRY);
        if (*ptr).is_none() {
            *ptr = Some(IrqRegistry::new());
        }
        (*ptr).as_mut().unwrap()
    }
}

#[cfg(feature = "irqreg")]
fn g1p_irq_handler() {
    let g = G1p::default_window();
    let pend = g.irq_pending();
    if (pend & (IRQ_PEND_DONE | IRQ_PEND_ERR)) != 0 {
        g.ack_irq(pend & (IRQ_PEND_DONE | IRQ_PEND_ERR));
    }
}

#[cfg(feature = "irqreg")]
fn gem_irq_handler() {
    // One-shot: the GEM handler masks its RX interrupt and sets RX_READY;
    // the serve poll loop drains frames and re-arms before sleeping.
    super::gem::on_rx_irq();
}

#[cfg(feature = "irqreg")]
fn timer_irq_handler() {
    // Clear the private-timer event flag (write 1 to bit 0).
    unsafe { core::ptr::write_volatile(PTIMER_ISR as *mut u32, 1) };
    // Storm-window sampling happens here (once per real timer period), never
    // inside the hot ISR dispatch path itself.
    let mut masked = [IrqHandle::default(); 4];
    let n = registry().check_storm_window(&mut masked);
    for h in &masked[..n] {
        unsafe { dist_write(ICDICER + (h.irq / 32) * 4, 1 << (h.irq % 32)) };
    }
}

#[cfg(all(feature = "irqreg", feature = "usb"))]
fn usb_irq_handler() {
    super::usb::on_irq();
}

#[cfg(feature = "irqreg")]
pub fn init_g1p_spi() {
    unsafe {
        dist_write(ICDDCR, 0);
        configure_int(G1P_IRQ_ID, G1P_PRIORITY);
        cpu_write(ICCPMR, 0xF0);
        cpu_write(ICCICR, 0x01);
        dist_write(ICDDCR, 0x01);
    }
    let handle = registry()
        .register(G1P_IRQ_ID, g1p_irq_handler)
        .expect("g1p irq: register once at boot");
    let _ = registry().enable(handle);
    unsafe { *core::ptr::addr_of_mut!(G1P_HANDLE) = Some(handle) };
    #[cfg(feature = "usb")]
    {
        let usb = registry()
            .register(USB0_IRQ_ID, usb_irq_handler)
            .expect("usb0 irq: register once at boot");
        let _ = registry().enable(usb);
        unsafe { *core::ptr::addr_of_mut!(USB_HANDLE) = Some(usb) };
    }
}

#[cfg(feature = "irqreg")]
pub fn enable_gem0_rx_irq() {
    unsafe {
        configure_int(GEM0_IRQ_ID, GEM_PRIORITY);
        let ptr = core::ptr::addr_of_mut!(GEM_HANDLE);
        if (*ptr).is_none() {
            let handle = registry()
                .register(GEM0_IRQ_ID, gem_irq_handler)
                .expect("gem irq: register once");
            *ptr = Some(handle);
        }
        let _ = registry().enable((*ptr).unwrap());
    }
    super::gem::enable_rx_irq();
}

#[cfg(feature = "irqreg")]
pub fn enable_tick(load: u32) {
    unsafe {
        configure_int(PRIV_TIMER_IRQ_ID, TICK_PRIORITY);
        let ptr = core::ptr::addr_of_mut!(TIMER_HANDLE);
        if (*ptr).is_none() {
            let handle = registry()
                .register(PRIV_TIMER_IRQ_ID, timer_irq_handler)
                .expect("timer irq: register once");
            *ptr = Some(handle);
        }
        let _ = registry().enable((*ptr).unwrap());
        core::ptr::write_volatile(PTIMER_LOAD as *mut u32, load);
        core::ptr::write_volatile(PTIMER_CTRL as *mut u32, 0b111);
    }
}

/// Enable USB0 SPI 53 at the GIC and unmask EHCI USBINTR. The irqreg slot is
/// registered at [`init_g1p_spi`]; this arms the hardware line after the
/// controller is running. HID report drain stays poll-driven (GEM RX pattern).
#[cfg(all(feature = "irqreg", feature = "usb"))]
pub fn enable_usb0_irq() {
    unsafe {
        configure_int(USB0_IRQ_ID, USB_PRIORITY);
    }
    super::usb::enable_controller_irq();
}

#[cfg(feature = "irqreg")]
pub fn disable_tick() {
    unsafe {
        core::ptr::write_volatile(PTIMER_CTRL as *mut u32, 0);
        if let Some(h) = *core::ptr::addr_of!(TIMER_HANDLE) {
            let _ = registry().disable(h);
        }
        dist_write(ICDICER, 1 << PRIV_TIMER_IRQ_ID);
    }
}

#[cfg(feature = "irqreg")]
pub fn gem_rx_irq_count() -> u32 {
    registry().slot_count(GEM0_IRQ_ID).unwrap_or(0)
}

#[cfg(feature = "irqreg")]
pub fn tick_count() -> u32 {
    registry().slot_count(PRIV_TIMER_IRQ_ID).unwrap_or(0)
}

#[cfg(feature = "irqreg")]
pub fn irq_count() -> u32 {
    registry().slot_count(G1P_IRQ_ID).unwrap_or(0)
}

#[cfg(all(feature = "irqreg", feature = "usb"))]
pub fn usb0_irq_count() -> u32 {
    registry().slot_count(USB0_IRQ_ID).unwrap_or(0)
}

/// The registry's per-IRQ counters (`C_q`) are cumulative and feed the
/// storm-window delta accounting in [`timer_irq_handler`]; resetting one
/// here would corrupt that accounting. No-op by design under `irqreg`
/// (unlike the feature-off path, which does reset the raw counter) — see
/// `fpga/zynq/evidence/IRQREG_STATUS_2026-08-12.md`. No in-tree caller
/// currently invokes this.
#[cfg(feature = "irqreg")]
pub fn clear_irq_count() {}

/// Rust IRQ dispatch entered from `startup.S`. Acknowledges GIC via the
/// registry's O(1) lookup (see `kernel::irqreg::IrqRegistry::dispatch`),
/// then EOIs exactly as before.
#[cfg(feature = "irqreg")]
#[no_mangle]
pub extern "C" fn kairos_board_irq_dispatch() {
    let iar = unsafe { cpu_read(ICCIAR) };
    let id = iar & 0x3FF;
    if id != SPURIOUS_ID {
        registry().dispatch(id);
    }
    unsafe { cpu_write(ICCEOIR, iar) };
}

/// Read-only view of the live registry, for the `irqreg` shell command.
#[cfg(feature = "irqreg")]
pub fn registry_stats() -> crate::kernel::irqreg::IrqRegistryStats {
    registry().stats()
}

#[cfg(feature = "irqreg")]
pub fn is_storm_masked(irq: u32) -> bool {
    registry().is_storm_masked(irq)
}
