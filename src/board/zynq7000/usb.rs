//! Bare-metal USB 2.0 **HOST** bring-up (feature `usb`) for the PZ7020.
//!
//! Hardware (grounded from the vendor packs, not guessed):
//!  - Device tree (`image.ub`): `usb@e0002000` `phy_type=ulpi` `dr_mode=host`
//!    => **USB0 @ 0xE0002000, ULPI PHY (USB3320), HOST mode**, USB Type-A `J10`.
//!  - Vendor `ps7_init.tcl` (1_hello_world, USB0 enabled): MIO 28-39 mux values
//!    `0x1204`/`0x1205` (L1_SEL=1 = USB0), `APER_CLK_CTRL` `USB0_CPU_1XCLKACT`.
//!  - `xusbps_hw.h`: controller register offsets / bit masks.
//!
//! U0 milestone = the **muscle**: MIO+clock init, host-mode controller bring-up,
//! and root-port connect/reset/speed detection. Enumeration (U1) and HID (U2)
//! build the EHCI transfer engine on top. The device *lifecycle* is modeled by
//! the `DomainId::Usb` phase state domain (the brain) — see `usb probe` dispatch.

use crate::platform::String;
use core::sync::atomic::{AtomicBool, Ordering};

// ── USB0 controller (ChipIdea/EHCI) — base + offsets from xusbps_hw.h ──────
const USB0_BASE: u32 = 0xE000_2000;
const OFF_CMD: u32 = 0x140; // USB command
const OFF_USBSTS: u32 = 0x144; // USB status (xusbps ISR)
const OFF_USBINTR: u32 = 0x148; // USB interrupt enable
const OFF_PORTSC1: u32 = 0x184; // port status/control 1
const OFF_OTGSC: u32 = 0x1A4; // OTG status/control
const OFF_MODE: u32 = 0x1A8; // USB mode
const OFF_ULPIVIEW: u32 = 0x170; // ULPI viewport (xusbps_hw.h)

const CMD_RS: u32 = 1 << 0; // run/stop
const CMD_RST: u32 = 1 << 1; // controller reset
const MODE_CM_HOST: u32 = 0x3; // controller mode = host (XUSBPS_MODE_CM_HOST_MASK)
const IXR_UI: u32 = 1 << 0; // USB transaction complete
const IXR_UE: u32 = 1 << 1; // USB error
const IXR_PC: u32 = 1 << 2; // port change
const IXR_W1C: u32 = IXR_UI | IXR_UE | IXR_PC;

const PORTSC_CCS: u32 = 1 << 0; // current connect status
const PORTSC_CSC: u32 = 1 << 1; // connect status change (write-1-clear)
const PORTSC_PE: u32 = 1 << 2; // port enabled
const PORTSC_PEC: u32 = 1 << 3; // port enable change (write-1-clear)
const PORTSC_OCC: u32 = 1 << 5; // over-current change (write-1-clear)
const PORTSC_PR: u32 = 1 << 8; // port reset
const PORTSC_PP: u32 = 1 << 12; // port power
const PORTSC_PSPD_SHIFT: u32 = 26; // port speed [27:26]
/// Write-1-to-clear status bits — mask these OFF when doing read-modify-write
/// so we don't inadvertently clear a pending change.
const PORTSC_RWC: u32 = PORTSC_CSC | PORTSC_PEC | PORTSC_OCC;

const OTGSC_VC: u32 = 1 << 1; // VBus charge
const ULPI_WU: u32 = 1 << 31;
const ULPI_RUN: u32 = 1 << 30;
const ULPI_RW: u32 = 1 << 29;
const ULPI_SS: u32 = 1 << 27;
const PORTSC_PTS_ULPI: u32 = 2 << 30;
const PORTSC_PTS_MASK: u32 = (0x7 << 29) | (1 << 25); // PTS[31:30]+STS/extra + PTW-ish clear mask

// ── SLCR (locked by default; unlock/lock exactly like gem.rs) ──────────────
const SLCR_LOCK: u32 = 0xF800_0004;
const SLCR_UNLOCK: u32 = 0xF800_0008;
const SLCR_LOCK_KEY: u32 = 0x0000_767B;
const SLCR_UNLOCK_KEY: u32 = 0x0000_DF0D;
const APER_CLK_CTRL: u32 = 0xF800_012C;
const APER_USB0_CLK: u32 = 1 << 2; // USB0_CPU_1XCLKACT
const MIO_PIN_28: u32 = 0xF800_0770; // 0xF8000700 + 28*4
const MIO_MASK: u32 = 0x0000_3FFF;

/// MIO 28-39 for USB0 ULPI — vendor `ps7_init` + UG585:
/// CLK=MIO36, DIR=29, NXT=31 (inputs → `0x1205`); data/STP → `0x1204`.
/// (v1–v9 wrongly put `0x1205` on MIO37/DATA5 instead of MIO36/CLK.)
const USB0_MIO: [u32; 12] = [
    0x1204, 0x1205, 0x1204, 0x1205, 0x1204, 0x1204, // MIO 28-33
    0x1204, 0x1204, 0x1205, 0x1204, 0x1204, 0x1204, // MIO 34-39
];

static USB_CTRL_LIVE: AtomicBool = AtomicBool::new(false);

// ── Cortex-A9 global timer for real millisecond delays ─────────────────────
const GTC_LO: u32 = 0xF8F0_0200;
const GTC_HI: u32 = 0xF8F0_0204;
const GTC_CTRL: u32 = 0xF8F0_0208;
const TICKS_PER_MS: u64 = 333_333;

/// Match Linux ARM `writel`/`readl` barriers (hyp H-barrier): bare `write_volatile`
/// alone can let WU and RUN coalesce on the same viewport address.
#[inline]
fn dsb() {
    unsafe { core::arch::asm!("dsb", options(nostack, preserves_flags)) }
}
#[inline]
fn dmb() {
    unsafe { core::arch::asm!("dmb", options(nostack, preserves_flags)) }
}

#[inline]
unsafe fn rd(a: u32) -> u32 {
    dmb();
    let v = core::ptr::read_volatile(a as *const u32);
    dmb();
    v
}
#[inline]
unsafe fn wr(a: u32, v: u32) {
    dsb();
    core::ptr::write_volatile(a as *mut u32, v);
    dsb();
}
#[inline]
unsafe fn r(off: u32) -> u32 {
    rd(USB0_BASE + off)
}
#[inline]
unsafe fn w(off: u32, v: u32) {
    wr(USB0_BASE + off, v);
}

fn read_gtc() -> u64 {
    unsafe {
        loop {
            let hi = rd(GTC_HI);
            let lo = rd(GTC_LO);
            let hi2 = rd(GTC_HI);
            if hi == hi2 {
                return ((hi as u64) << 32) | lo as u64;
            }
        }
    }
}

fn delay_ms(ms: u64) {
    unsafe { wr(GTC_CTRL, 1) }; // enable global timer (idempotent)
    let end = read_gtc() + ms * TICKS_PER_MS;
    while read_gtc() < end {
        core::hint::spin_loop();
    }
}

/// ~1 µs delay via A9 global timer (333333 ticks/ms → ~333 ticks/µs).
fn delay_us(us: u64) {
    unsafe { wr(GTC_CTRL, 1) };
    let end = read_gtc() + us * (TICKS_PER_MS / 1000);
    while read_gtc() < end {
        core::hint::spin_loop();
    }
}

/// Route MIO 28-39 to USB0 ULPI and enable the USB0 controller clock. Grounded
/// in the vendor ps7_init; SLCR unlock/lock mirrors `gem.rs`.
pub fn usb_mio_init() {
    unsafe {
        wr(SLCR_UNLOCK, SLCR_UNLOCK_KEY);
        for i in 0..12u32 {
            let reg = MIO_PIN_28 + i * 4;
            let v = (rd(reg) & !MIO_MASK) | (USB0_MIO[i as usize] & MIO_MASK);
            wr(reg, v);
        }
        let a = rd(APER_CLK_CTRL) | APER_USB0_CLK;
        wr(APER_CLK_CTRL, a);
        wr(SLCR_LOCK, SLCR_LOCK_KEY);
    }
}

// USBPHY_nRSET (MIO46) is pulsed by FSBL `ps7_init`; do not re-pulse at runtime.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortStatus {
    pub connected: bool,
    pub enabled: bool,
    pub reset_done: bool,
    pub speed: &'static str,
}

impl PortStatus {
    pub fn speed_from(portsc: u32) -> &'static str {
        match (portsc >> PORTSC_PSPD_SHIFT) & 0x3 {
            0 => "full-speed",
            1 => "low-speed",
            2 => "high-speed",
            _ => "undefined",
        }
    }
}

/// Linux order (seller boot log): PHY identify happens *before* "USB 2.0 started".
/// FSBL already pulsed USBPHY_nRSET (ps7_init); do **not** re-pulse here — v5–v8
/// re-reset never produced VID and may disturb an already-synced ULPI link.
/// Call [`host_run`] after ULPI VID/scratch checks.
pub fn host_init_stopped() -> Result<(), &'static str> {
    usb_mio_init();
    ensure_usb_dma_pools()?;
    unsafe {
        w(OFF_CMD, CMD_RST);
        let mut spins = 0u32;
        while (r(OFF_CMD) & CMD_RST) != 0 {
            delay_ms(1);
            spins += 1;
            if spins > 500 {
                return Err("usb: controller reset timeout");
            }
        }
        w(OFF_MODE, MODE_CM_HOST);
        delay_ms(2);
        w(OFF_OTGSC, r(OFF_OTGSC) | OTGSC_VC);
        // Linux `hw_phymode_configure`: force PTS=ULPI, then port power.
        let p = r(OFF_PORTSC1) & !PORTSC_RWC & !PORTSC_PTS_MASK;
        w(OFF_PORTSC1, p | PORTSC_PTS_ULPI | PORTSC_PP);
        delay_ms(2);
        // Wait for ULPI sync (Linux ci_ulpi_resume) before viewport access.
        let mut ss_wait = 0u32;
        while (r(OFF_ULPIVIEW) & ULPI_SS) == 0 && ss_wait < 100_000 {
            delay_us(1);
            ss_wait += 1;
        }
    }
    Ok(())
}

pub fn host_run() {
    unsafe {
        w(OFF_CMD, r(OFF_CMD) | CMD_RS);
        delay_ms(50);
    }
}

/// Unmask EHCI USBINTR. ISR-safe counterpart is [`on_irq`].
pub fn enable_controller_irq() {
    USB_CTRL_LIVE.store(true, Ordering::Release);
    unsafe {
        w(OFF_USBSTS, r(OFF_USBSTS) & IXR_W1C);
        w(OFF_USBINTR, IXR_W1C);
    }
}

/// IRQ context: one-shot mask USBINTR, W1C USBSTS. HID drain stays polled.
pub fn on_irq() {
    if !USB_CTRL_LIVE.load(Ordering::Acquire) {
        return;
    }
    unsafe {
        w(OFF_USBINTR, 0);
        let sts = r(OFF_USBSTS);
        w(OFF_USBSTS, sts & IXR_W1C);
    }
}

/// Poll context: re-arm USBINTR after draining HID/report qTDs.
pub fn rearm_controller_irq() {
    if !USB_CTRL_LIVE.load(Ordering::Acquire) {
        return;
    }
    enable_controller_irq();
}

fn arm_usb0_irq() {
    #[cfg(feature = "irqreg")]
    super::gic::enable_usb0_irq();
}

/// Full host bring-up including run (used by `usb probe`).
pub fn host_init() -> Result<(), &'static str> {
    host_init_stopped()?;
    host_run();
    arm_usb0_irq();
    Ok(())
}

/// Bring the root port to an enumerable state: wait for a connection, debounce,
/// (optionally) bus-reset, wait for port-enable, and settle. Returns status +
/// negotiated speed. This is the standard USB port sequence -- a fixed delay is
/// not enough after a controller reset re-powers the port.
pub fn poll_port(do_reset: bool) -> PortStatus {
    unsafe {
        // 1. Wait for a device to be detected (CCS) after port power-on.
        let mut connected = false;
        for _ in 0..1500 {
            if (r(OFF_PORTSC1) & PORTSC_CCS) != 0 {
                connected = true;
                break;
            }
            delay_ms(1);
        }

        let mut reset_done = false;
        if connected && do_reset {
            // 2. Connect debounce (device stable) before reset.
            delay_ms(100);
            // 3. Assert port reset, hold >=50 ms (clear RWC bits so we keep flags).
            w(OFF_PORTSC1, (r(OFF_PORTSC1) & !PORTSC_RWC) | PORTSC_PR);
            delay_ms(60);
            // 4. Deassert reset, wait for it to complete.
            w(OFF_PORTSC1, r(OFF_PORTSC1) & !PORTSC_RWC & !PORTSC_PR);
            for _ in 0..100 {
                if (r(OFF_PORTSC1) & PORTSC_PR) == 0 {
                    break;
                }
                delay_ms(1);
            }
            // 5. Wait for the port to enable (PE) after a successful reset.
            for _ in 0..100 {
                if (r(OFF_PORTSC1) & PORTSC_PE) != 0 {
                    break;
                }
                delay_ms(1);
            }
            // 6. Reset recovery before the first transaction.
            delay_ms(20);
            reset_done = true;
        }

        let p2 = r(OFF_PORTSC1);
        PortStatus {
            connected: (p2 & PORTSC_CCS) != 0,
            enabled: (p2 & PORTSC_PE) != 0,
            reset_done,
            speed: PortStatus::speed_from(p2),
        }
    }
}

// ── EHCI host: async schedule + endpoint-0 control transfers (U1) ──────────
//
// The board runs with MMU/D-cache ON (CPU0 identity WBWA; see cpu1.rs).
// QH/qTD/SETUP/DATA live in DDR .bss, so each DMA kick must clean then
// invalidate, same as gem.rs. Physical == virtual (identity).

const OFF_ASYNCLISTADDR: u32 = 0x158;
const CMD_ASE: u32 = 1 << 5; // async schedule enable
const A9_DCACHE_LINE: u32 = 32;

/// CPU0 D-cache is on (identity WBWA). USB DMA reads/writes DDR; same
/// clean/invalidate as `gem.rs`.
fn dma_clean(addr: u32, len: u32) {
    if len == 0 {
        return;
    }
    let start = addr & !(A9_DCACHE_LINE - 1);
    let end = addr.wrapping_add(len).wrapping_add(A9_DCACHE_LINE - 1) & !(A9_DCACHE_LINE - 1);
    let mut a = start;
    while a < end {
        unsafe {
            core::arch::asm!(
                "mcr p15, 0, {}, c7, c10, 1",
                in(reg) a,
                options(nostack, preserves_flags)
            );
        }
        a = a.wrapping_add(A9_DCACHE_LINE);
    }
    dsb();
}

fn dma_invalidate(addr: u32, len: u32) {
    if len == 0 {
        return;
    }
    let start = addr & !(A9_DCACHE_LINE - 1);
    let end = addr.wrapping_add(len).wrapping_add(A9_DCACHE_LINE - 1) & !(A9_DCACHE_LINE - 1);
    let mut a = start;
    while a < end {
        unsafe {
            core::arch::asm!(
                "mcr p15, 0, {}, c7, c6, 1",
                in(reg) a,
                options(nostack, preserves_flags)
            );
        }
        a = a.wrapping_add(A9_DCACHE_LINE);
    }
    dsb();
}


/// Poll viewport until `mask` bits clear. Linux `phy-ulpi-viewport.c`:
/// `readl_poll_timeout_atomic(..., 1 /*us*/, 2000 /*us*/)`.
fn ulpi_wait_clear(mask: u32) {
    let mut spins = 0u32;
    unsafe {
        while (r(OFF_ULPIVIEW) & mask) != 0 && spins < 2000 {
            delay_us(1);
            spins += 1;
        }
    }
}

/// Linux `drivers/usb/phy/phy-ulpi-viewport.c` read path.
fn ulpi_read(addr: u8) -> u8 {
    unsafe {
        w(OFF_ULPIVIEW, ULPI_WU | ULPI_RW);
        ulpi_wait_clear(ULPI_WU);
        w(OFF_ULPIVIEW, ULPI_RUN | ((addr as u32) << 16));
        ulpi_wait_clear(ULPI_RUN);
        ((r(OFF_ULPIVIEW) >> 8) & 0xFF) as u8
    }
}

/// Linux `phy-ulpi-viewport.c` write path.
fn ulpi_write(addr: u8, data: u8) {
    unsafe {
        w(OFF_ULPIVIEW, ULPI_WU | ULPI_RW);
        ulpi_wait_clear(ULPI_WU);
        w(
            OFF_ULPIVIEW,
            ULPI_RUN | ULPI_RW | ((addr as u32) << 16) | data as u32,
        );
        ulpi_wait_clear(ULPI_RUN);
    }
}

// qTD token bits.
const QTD_ACTIVE: u32 = 1 << 7;
const QTD_HALTED: u32 = 1 << 6;
const QTD_IOC: u32 = 1 << 15;
const QTD_CERR3: u32 = 3 << 10;
const PID_OUT: u32 = 0 << 8;
const PID_IN: u32 = 1 << 8;
const PID_SETUP: u32 = 2 << 8;
const TOGGLE1: u32 = 1 << 31;
const TERMINATE: u32 = 1; // T-bit on a link pointer

/// EHCI queue element transfer descriptor (32 bytes, 32-byte aligned).
#[repr(C, align(32))]
#[derive(Clone, Copy)]
struct Qtd {
    next: u32,
    alt_next: u32,
    token: u32,
    bufs: [u32; 5],
}

/// EHCI queue head (48 bytes, 32-byte aligned).
#[repr(C, align(32))]
struct Qh {
    horiz: u32,
    ep_char: u32,
    ep_cap: u32,
    cur_qtd: u32,
    ov_next: u32,
    ov_alt: u32,
    ov_token: u32,
    ov_bufs: [u32; 5],
}

static mut CTRL_QH: Qh = Qh {
    horiz: 0,
    ep_char: 0,
    ep_cap: 0,
    cur_qtd: 0,
    ov_next: 0,
    ov_alt: 0,
    ov_token: 0,
    ov_bufs: [0; 5],
};
static mut CTRL_QTD: [Qtd; 3] = [Qtd {
    next: 0,
    alt_next: 0,
    token: 0,
    bufs: [0; 5],
}; 3];
static mut SETUP_BUF: [u8; 8] = [0; 8];
// Data buffer, page-aligned so a small transfer never crosses a 4 KiB page.
#[repr(C, align(4096))]
struct DataBuf([u8; 512]);
static mut DATA_BUF: DataBuf = DataBuf([0; 512]);

/// Private USB DMA pool. Do not share GEM's 8-slot table.
static mut USB_DMA: crate::kernel::dma_contain::DmaContainment =
    crate::kernel::dma_contain::DmaContainment::new();
static USB_DMA_READY: AtomicBool = AtomicBool::new(false);

fn phys<T>(p: *const T) -> u32 {
    p as u32
}

fn usb_qh_phys() -> u32 {
    unsafe { phys(core::ptr::addr_of!(CTRL_QH) as *const Qh) }
}

fn usb_qtd_phys() -> u32 {
    unsafe { phys(core::ptr::addr_of!(CTRL_QTD) as *const Qtd) }
}

fn usb_setup_phys() -> u32 {
    unsafe { phys(core::ptr::addr_of!(SETUP_BUF) as *const u8) }
}

fn usb_data_phys() -> u32 {
    unsafe { phys(core::ptr::addr_of!(DATA_BUF) as *const u8) }
}

fn ensure_usb_dma_pools() -> Result<(), &'static str> {
    if USB_DMA_READY.load(Ordering::SeqCst) {
        return Ok(());
    }
    if crate::kernel::dma_contain::usb_quarantined() {
        return Err("usb: dma quarantined");
    }
    unsafe {
        USB_DMA = crate::kernel::dma_contain::DmaContainment::new();
        USB_DMA
            .register_pool(usb_qh_phys(), core::mem::size_of::<Qh>() as u32)
            .map_err(|_| "usb: dma QH pool")?;
        USB_DMA
            .register_pool(
                usb_qtd_phys(),
                core::mem::size_of::<[Qtd; 3]>() as u32,
            )
            .map_err(|_| "usb: dma qTD pool")?;
        USB_DMA
            .register_pool(usb_setup_phys(), 8)
            .map_err(|_| "usb: dma SETUP pool")?;
        USB_DMA
            .register_pool(usb_data_phys(), 512)
            .map_err(|_| "usb: dma DATA pool")?;
    }
    USB_DMA_READY.store(true, Ordering::SeqCst);
    Ok(())
}

/// Submit QH/qTD/SETUP/DATA before the controller is given those physical
/// addresses. Refuse and do not kick DMA on quarantine or out-of-pool.
fn usb_dma_submit_ctrl() -> Result<(), &'static str> {
    if crate::kernel::dma_contain::usb_quarantined() {
        return Err("usb: dma quarantined");
    }
    ensure_usb_dma_pools()?;
    let cap = crate::kernel::dma_contain::kernel_dma_allowed();
    unsafe {
        let _ = USB_DMA.complete(usb_qh_phys());
        let _ = USB_DMA.complete(usb_qtd_phys());
        let _ = USB_DMA.complete(usb_setup_phys());
        let _ = USB_DMA.complete(usb_data_phys());
        if USB_DMA
            .submit(usb_qh_phys(), core::mem::size_of::<Qh>() as u32, cap)
            .is_err()
            || USB_DMA
                .submit(
                    usb_qtd_phys(),
                    core::mem::size_of::<[Qtd; 3]>() as u32,
                    cap,
                )
                .is_err()
            || USB_DMA.submit(usb_setup_phys(), 8, cap).is_err()
            || USB_DMA.submit(usb_data_phys(), 512, cap).is_err()
        {
            return Err("usb: dma refuse (not an IOMMU)");
        }
    }
    Ok(())
}

fn usb_dma_complete_ctrl() {
    unsafe {
        let _ = USB_DMA.complete(usb_qh_phys());
        let _ = USB_DMA.complete(usb_qtd_phys());
        let _ = USB_DMA.complete(usb_setup_phys());
        let _ = USB_DMA.complete(usb_data_phys());
    }
}

/// UART for `dma` / `usb probe`. Not an IOMMU.
pub fn dma_status_lines() -> String {
    if !USB_DMA_READY.load(Ordering::SeqCst) {
        return String::from(
            "usb dma: pools=0 submits=0 refused=0 (not an IOMMU)\nIOMMU: HARDWARE_UNAVAILABLE\n",
        );
    }
    unsafe {
        alloc::format!(
            "usb dma: pools={} submits={} refused={} (not an IOMMU)\n{}",
            USB_DMA.pool_count(),
            USB_DMA.submit_count(),
            USB_DMA.refused_count(),
            USB_DMA.status_lines()
        )
    }
}

fn set_qtd_bufs(qtd: &mut Qtd, addr: u32, len: usize) {
    qtd.bufs = [0; 5];
    if len == 0 {
        return;
    }
    qtd.bufs[0] = addr;
    // page 1 covers the case where the buffer crosses one 4 KiB boundary.
    qtd.bufs[1] = (addr + 0x1000) & !0xFFF;
}

/// Run a control transfer on endpoint 0. `setup` is the 8-byte setup packet;
/// for an IN transfer `data` receives up to `data.len()` bytes. Returns the
/// number of data bytes. Single-buffer, polled — enough for enumeration.
pub fn control_transfer(
    dev_addr: u8,
    ep0_maxpkt: u16,
    speed_full_low_high: u32,
    setup: &[u8; 8],
    data: Option<&mut [u8]>,
    data_in: bool,
) -> Result<usize, &'static str> {
    let want = data.as_ref().map(|d| d.len()).unwrap_or(0);
    unsafe {
        // Copy the setup packet into the DMA buffer.
        SETUP_BUF.copy_from_slice(setup);

        let qtd_ptr = core::ptr::addr_of!(CTRL_QTD) as *const Qtd;
        let setup_p = phys(qtd_ptr);
        let data_p = phys(qtd_ptr.add(1));
        let status_p = phys(qtd_ptr.add(2));

        // qTD[0] SETUP (PID SETUP, toggle 0, 8 bytes).
        CTRL_QTD[0].next = data_p; // -> DATA (or STATUS if no data)
        CTRL_QTD[0].alt_next = TERMINATE;
        CTRL_QTD[0].token = QTD_ACTIVE | PID_SETUP | QTD_CERR3 | ((8u32) << 16);
        set_qtd_bufs(
            &mut CTRL_QTD[0],
            phys(core::ptr::addr_of!(SETUP_BUF) as *const u8),
            8,
        );

        // qTD[1] DATA (optional).
        if want > 0 {
            let pid = if data_in { PID_IN } else { PID_OUT };
            CTRL_QTD[1].next = status_p;
            CTRL_QTD[1].alt_next = TERMINATE;
            CTRL_QTD[1].token = QTD_ACTIVE | pid | QTD_CERR3 | ((want as u32) << 16) | TOGGLE1;
            set_qtd_bufs(
                &mut CTRL_QTD[1],
                phys(core::ptr::addr_of!(DATA_BUF) as *const u8),
                want,
            );
            CTRL_QTD[0].next = data_p;
        } else {
            CTRL_QTD[0].next = status_p;
        }

        // qTD[2] STATUS (opposite direction, zero length, IOC, toggle 1).
        let status_pid = if data_in { PID_OUT } else { PID_IN };
        CTRL_QTD[2].next = TERMINATE;
        CTRL_QTD[2].alt_next = TERMINATE;
        CTRL_QTD[2].token = QTD_ACTIVE | status_pid | QTD_CERR3 | QTD_IOC | TOGGLE1;
        set_qtd_bufs(&mut CTRL_QTD[2], 0, 0);

        // Queue head for endpoint 0.
        let eps = speed_full_low_high & 0x3; // 0 full, 1 low, 2 high (EHCI EPS)
        let control_flag = if eps == 2 { 0 } else { 1u32 << 27 }; // C=1 for FS/LS control
        CTRL_QH.horiz = phys(core::ptr::addr_of!(CTRL_QH) as *const Qh) | (1 << 1); // Typ=QH, T=0 (self)
        CTRL_QH.ep_char = (dev_addr as u32)
            | (0 << 8) // endpoint 0
            | (eps << 12)
            | (1 << 14) // DTC: use qTD data toggle
            | (1 << 15) // H: head of reclamation list
            | ((ep0_maxpkt as u32) << 16)
            | control_flag;
        CTRL_QH.ep_cap = qh_ep_cap();
        CTRL_QH.cur_qtd = 0;
        CTRL_QH.ov_next = setup_p;
        CTRL_QH.ov_alt = TERMINATE;
        CTRL_QH.ov_token = 0;

        dma_clean(phys(core::ptr::addr_of!(SETUP_BUF) as *const u8), 8);
        if want > 0 {
            dma_clean(
                phys(core::ptr::addr_of!(DATA_BUF) as *const u8),
                want as u32,
            );
        }
        dma_clean(
            phys(core::ptr::addr_of!(CTRL_QTD) as *const Qtd),
            core::mem::size_of::<[Qtd; 3]>() as u32,
        );
        dma_clean(
            phys(core::ptr::addr_of!(CTRL_QH) as *const Qh),
            core::mem::size_of::<Qh>() as u32,
        );
        dma_invalidate(
            phys(core::ptr::addr_of!(CTRL_QTD) as *const Qtd),
            core::mem::size_of::<[Qtd; 3]>() as u32,
        );

        usb_dma_submit_ctrl()?;

        // Install the async list and enable it.
        w(
            OFF_ASYNCLISTADDR,
            phys(core::ptr::addr_of!(CTRL_QH) as *const Qh),
        );
        w(OFF_CMD, r(OFF_CMD) | CMD_ASE);

        // Poll the STATUS qTD until it retires (or any qTD halts).
        let mut spins = 0u32;
        loop {
            dma_invalidate(
                phys(core::ptr::addr_of!(CTRL_QTD) as *const Qtd),
                core::mem::size_of::<[Qtd; 3]>() as u32,
            );
            let st = core::ptr::read_volatile(core::ptr::addr_of!(CTRL_QTD[2].token));
            let d = core::ptr::read_volatile(core::ptr::addr_of!(CTRL_QTD[1].token));
            let s0 = core::ptr::read_volatile(core::ptr::addr_of!(CTRL_QTD[0].token));
            if (s0 & QTD_HALTED) != 0 || (d & QTD_HALTED) != 0 || (st & QTD_HALTED) != 0 {
                w(OFF_CMD, r(OFF_CMD) & !CMD_ASE);
                usb_dma_complete_ctrl();
                return Err("usb: control transfer halted");
            }
            if (st & QTD_ACTIVE) == 0 {
                break;
            }
            delay_ms(1);
            spins += 1;
            if spins > 500 {
                w(OFF_CMD, r(OFF_CMD) & !CMD_ASE);
                usb_dma_complete_ctrl();
                return Err("usb: control transfer timeout");
            }
        }
        w(OFF_CMD, r(OFF_CMD) & !CMD_ASE);
        usb_dma_complete_ctrl();

        // Bytes actually transferred = requested - residual (token[30:16]).
        let mut got = want;
        if want > 0 {
            dma_invalidate(
                phys(core::ptr::addr_of!(DATA_BUF) as *const u8),
                want as u32,
            );
            let residual = ((core::ptr::read_volatile(core::ptr::addr_of!(CTRL_QTD[1].token))
                >> 16)
                & 0x7FFF) as usize;
            got = want.saturating_sub(residual);
            if let Some(dst) = data {
                let n = got.min(dst.len());
                dst[..n].copy_from_slice(&DATA_BUF.0[..n]);
            }
        }
        Ok(got)
    }
}

/// U1 spike: GET_DESCRIPTOR(device) on the freshly-reset port. Returns the
/// 18-byte device descriptor (VID/PID live at offsets 8/10).
pub fn get_device_descriptor(speed_full_low_high: u32) -> Result<[u8; 18], &'static str> {
    // First read 8 bytes with the safe 8-byte max-packet to learn bMaxPacketSize0.
    let setup8 = [0x80u8, 0x06, 0x00, 0x01, 0x00, 0x00, 0x08, 0x00];
    let mut d8 = [0u8; 8];
    control_transfer(0, 8, speed_full_low_high, &setup8, Some(&mut d8), true)?;
    let maxpkt = if d8[7] == 0 { 8 } else { d8[7] as u16 };

    // Full 18-byte descriptor with the real max packet size.
    let setup18 = [0x80u8, 0x06, 0x00, 0x01, 0x00, 0x00, 0x12, 0x00];
    let mut d18 = [0u8; 18];
    let n = control_transfer(
        0,
        maxpkt,
        speed_full_low_high,
        &setup18,
        Some(&mut d18),
        true,
    )?;
    if n < 18 {
        return Err("usb: short device descriptor");
    }
    Ok(d18)
}

/// One-shot probe: bring up the host controller and read the root port. Returns
/// a human-readable report plus the port status (the caller drives the
/// `DomainId::Usb` phase transitions from the status).
pub fn probe() -> (String, PortStatus) {
    if let Err(e) = host_init() {
        return (
            alloc::format!("usb: {e}"),
            PortStatus {
                connected: false,
                enabled: false,
                reset_done: false,
                speed: "n/a",
            },
        );
    }
    let st = poll_port(true);
    let report = if st.connected {
        alloc::format!(
            "usb0: host up @ 0xE0002000 (ULPI) -- device CONNECTED, {}{}\n{}",
            st.speed,
            if st.reset_done { ", reset done" } else { "" },
            dma_status_lines().trim_end()
        )
    } else {
        alloc::format!(
            "usb0: host up @ 0xE0002000 (ULPI) -- port powered, no device\n{}",
            dma_status_lines().trim_end()
        )
    };
    (report, st)
}

fn eps_of(speed: &str) -> u32 {
    match speed {
        "low-speed" => 1,
        "high-speed" => 2,
        _ => 0, // full-speed
    }
}

/// U1 spike: identify ULPI PHY before CMD.RS (seller Linux order), then enumerate.
pub fn enum_spike() -> (String, PortStatus) {
    if let Err(e) = host_init_stopped() {
        return (
            alloc::format!("usb: {e}"),
            PortStatus {
                connected: false,
                enabled: false,
                reset_done: false,
                speed: "n/a",
            },
        );
    }

    let portsc0 = unsafe { r(OFF_PORTSC1) };
    let vidl = ulpi_read(0x00);
    let vidh = ulpi_read(0x01);
    let fc = ulpi_read(0x04);
    ulpi_write(0x16, 0x55);
    let s55 = ulpi_read(0x16);
    ulpi_write(0x16, 0xAA);
    let saa = ulpi_read(0x16);
    let mut report = alloc::format!(
        "usb enum: PORTSC0=0x{:08x} (pre-RS) PHY VID={:02x}{:02x} FuncCtrl=0x{:02x} Scratch={{0x{:02x},0x{:02x}}}",
        portsc0, vidh, vidl, fc, s55, saa
    );

    host_run();
    arm_usb0_irq();
    let st = poll_port(true);
    let portsc = unsafe { r(OFF_PORTSC1) };
    report.push_str(&alloc::format!(
        "\n  after RS+reset: PORTSC=0x{:08x} CCS={} PE={} PSPD={} speed={}",
        portsc,
        portsc & 1,
        (portsc >> 2) & 1,
        (portsc >> 26) & 3,
        st.speed
    ));

    if !st.connected {
        return (report + " -- no device", st);
    }
    delay_ms(30);

    let setup8 = [0x80u8, 0x06, 0x00, 0x01, 0x00, 0x00, 0x08, 0x00];
    let mut d8 = [0u8; 8];
    let res = control_transfer(0, 8, eps_of(st.speed), &setup8, Some(&mut d8), true);

    let usbsts = unsafe { r(OFF_USBSTS) };
    let (t0, t1, t2) = unsafe {
        (
            core::ptr::read_volatile(core::ptr::addr_of!(CTRL_QTD[0].token)),
            core::ptr::read_volatile(core::ptr::addr_of!(CTRL_QTD[1].token)),
            core::ptr::read_volatile(core::ptr::addr_of!(CTRL_QTD[2].token)),
        )
    };
    report.push_str(&alloc::format!(
        "\n  USBSTS=0x{usbsts:08x} qtd_tokens=[0x{t0:08x} 0x{t1:08x} 0x{t2:08x}]"
    ));
    match res {
        Ok(n) => {
            report.push_str(&alloc::format!(
                "\n  got {n} bytes: [{:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}] bMaxPacket0={}",
                d8[0], d8[1], d8[2], d8[3], d8[4], d8[5], d8[6], d8[7], d8[7]
            ));
        }
        Err(e) => report.push_str(&alloc::format!("\n  -> {e}")),
    }
    (report, st)
}

// ── U1 enumeration: SET_ADDRESS / config descriptor / SET_CONFIGURATION ─────
//
// Built on the proven `control_transfer` (SETUP/DATA/STATUS). Standard-request
// codes and descriptor layout are USB 2.0 §9. No-data control transfers already
// use an IN status stage (see `control_transfer`), which is what SET_ADDRESS and
// SET_CONFIGURATION need.

const CLASS_HID: u8 = 0x03;
const CLASS_HUB: u8 = 0x09;
const DESC_CONFIG: u8 = 0x02;
const DESC_INTERFACE: u8 = 0x04;
const DESC_ENDPOINT: u8 = 0x05;
const XFER_INTERRUPT: u8 = 0x03; // bmAttributes[1:0]
const EP_DIR_IN: u8 = 0x80; // bEndpointAddress bit 7

/// A freshly enumerated USB device: endpoint-0 facts plus, for an HID device,
/// its interrupt-IN endpoint (what U2 will poll for keyboard/mouse reports).
#[derive(Clone, Copy, Default)]
pub struct UsbDevice {
    pub addr: u8,
    pub vid: u16,
    pub pid: u16,
    pub dev_class: u8,
    pub ep0_maxpkt: u16,
    pub eps: u32, // EHCI speed field, reused for later transfers
    pub config_value: u8,
    pub is_hid: bool,
    pub hid_protocol: u8, // 1 = keyboard, 2 = mouse (boot protocol) of the CHOSEN iface
    pub hid_iface: u8,    // bInterfaceNumber of the chosen HID interface (for SET_PROTOCOL)
    pub ep_in: u8,        // chosen interrupt-IN endpoint address (0 = none found)
    pub ep_in_maxpkt: u16,
    pub ep_in_interval: u8,
    // Discovery list: every HID interrupt-IN endpoint seen (for diagnostics).
    pub hub_addr: u8,
    pub hub_port: u8,
    pub is_hub: bool,
    pub n_int_in: u8,
    pub int_in_ep: [u8; 4],
    pub int_in_iface: [u8; 4],
    pub int_in_proto: [u8; 4],
}

#[inline]
fn setup_set_address(new_addr: u8) -> [u8; 8] {
    // bmRequestType=0x00 (host->dev, standard, device), bRequest=SET_ADDRESS(5).
    [0x00, 0x05, new_addr, 0, 0, 0, 0, 0]
}
#[inline]
fn setup_get_config(index: u8, len: u16) -> [u8; 8] {
    // bmRequestType=0x80 (dev->host), bRequest=GET_DESCRIPTOR(6), wValue=CONFIG:index.
    [
        0x80,
        0x06,
        index,
        DESC_CONFIG,
        0,
        0,
        (len & 0xff) as u8,
        (len >> 8) as u8,
    ]
}
#[inline]
fn setup_set_config(value: u8) -> [u8; 8] {
    // bmRequestType=0x00, bRequest=SET_CONFIGURATION(9), wValue=config value.
    [0x00, 0x09, value, 0, 0, 0, 0, 0]
}

/// Walk a config-descriptor blob; record the HID interface + its interrupt-IN
/// endpoint. Descriptors are TLV: [bLength, bDescriptorType, ...].
fn parse_config(blob: &[u8], dev: &mut UsbDevice) {
    if blob.len() >= 6 && blob[1] == DESC_CONFIG {
        dev.config_value = blob[5]; // bConfigurationValue
    }
    let mut i = 0usize;
    let mut cur_is_hid = false;
    let mut cur_iface = 0u8;
    let mut cur_proto = 0u8;
    while i + 2 <= blob.len() {
        let blen = blob[i] as usize;
        if blen < 2 || i + blen > blob.len() {
            break;
        }
        match blob[i + 1] {
            DESC_INTERFACE if blen >= 9 => {
                cur_is_hid = blob[i + 5] == CLASS_HID; // bInterfaceClass
                cur_iface = blob[i + 2]; // bInterfaceNumber
                cur_proto = blob[i + 7]; // bInterfaceProtocol (1=kbd, 2=mouse)
                if cur_is_hid {
                    dev.is_hid = true;
                }
                if blob[i + 5] == CLASS_HUB {
                    dev.is_hub = true;
                }
            }
            DESC_ENDPOINT if blen >= 7 => {
                let addr = blob[i + 2]; // bEndpointAddress
                let attr = blob[i + 3]; // bmAttributes
                if cur_is_hid && (attr & 0x03) == XFER_INTERRUPT && (addr & EP_DIR_IN) != 0 {
                    let maxpkt = (blob[i + 4] as u16) | ((blob[i + 5] as u16) << 8);
                    let interval = blob[i + 6]; // bInterval
                                                // Record in the discovery list.
                    let k = dev.n_int_in as usize;
                    if k < dev.int_in_ep.len() {
                        dev.int_in_ep[k] = addr;
                        dev.int_in_iface[k] = cur_iface;
                        dev.int_in_proto[k] = cur_proto;
                        dev.n_int_in += 1;
                    }
                    // Choose the poll target: prefer a boot keyboard (protocol 1),
                    // else the first interrupt-IN endpoint found. Composite devices
                    // expose several HID interfaces; keys travel on the keyboard one.
                    let choose = (cur_proto == 1 && dev.hid_protocol != 1) || dev.ep_in == 0;
                    if choose {
                        dev.ep_in = addr;
                        dev.ep_in_maxpkt = maxpkt;
                        dev.ep_in_interval = interval;
                        dev.hid_iface = cur_iface;
                        dev.hid_protocol = cur_proto;
                    }
                }
            }
            _ => {}
        }
        i += blen;
    }
}

/// Full U1 enumeration on the attached device. Brings up the port in the proven
/// order, then: GET_DESCRIPTOR(device) -> SET_ADDRESS(1) -> GET_DESCRIPTOR(config)
/// -> SET_CONFIGURATION -> parse the HID interface. Returns the device + port.
pub fn enumerate() -> Result<(UsbDevice, PortStatus), &'static str> {
    host_init_stopped()?;
    host_run();
    arm_usb0_irq();
    let st = poll_port(true);
    if !st.connected {
        return Err("no device connected");
    }
    delay_ms(30); // reset recovery before the first transaction
    let eps = eps_of(st.speed);

    // 1. Device descriptor (8 then 18 bytes) at the default address 0.
    let d18 = get_device_descriptor(eps)?;
    let ep0_maxpkt = if d18[7] == 0 { 8 } else { d18[7] as u16 };
    let mut dev = UsbDevice {
        addr: 0,
        vid: (d18[8] as u16) | ((d18[9] as u16) << 8),
        pid: (d18[10] as u16) | ((d18[11] as u16) << 8),
        dev_class: d18[4],
        ep0_maxpkt,
        eps,
        ..UsbDevice::default()
    };
    if d18[4] == CLASS_HUB {
        dev.is_hub = true;
    }

    // 2. SET_ADDRESS(1): no data stage, IN status. Then wait the address-recovery
    //    interval (USB 2.0 §9.2.6.3) before talking to the new address.
    control_transfer(0, ep0_maxpkt, eps, &setup_set_address(1), None, false)?;
    delay_ms(5);
    dev.addr = 1;

    // 3. Config descriptor: read the 9-byte header for wTotalLength, then the full
    //    blob (interfaces + endpoints), capped at our DMA buffer.
    let mut hdr = [0u8; 9];
    control_transfer(
        1,
        ep0_maxpkt,
        eps,
        &setup_get_config(0, 9),
        Some(&mut hdr),
        true,
    )?;
    let total = (((hdr[3] as usize) << 8) | hdr[2] as usize).min(256);
    let mut blob = [0u8; 256];
    let n = control_transfer(
        1,
        ep0_maxpkt,
        eps,
        &setup_get_config(0, total as u16),
        Some(&mut blob[..total]),
        true,
    )?;
    parse_config(&blob[..n], &mut dev);

    // 4. SET_CONFIGURATION: activate the (only) configuration.
    let cfg = if dev.config_value == 0 {
        1
    } else {
        dev.config_value
    };
    control_transfer(1, ep0_maxpkt, eps, &setup_set_config(cfg), None, false)?;
    dev.config_value = cfg;

    Ok((dev, st))
}

const MAX_TOPO: usize = 6;
static mut TOPO: [UsbDevice; MAX_TOPO] = [UsbDevice {
    addr: 0,
    vid: 0,
    pid: 0,
    dev_class: 0,
    ep0_maxpkt: 0,
    eps: 0,
    config_value: 0,
    is_hid: false,
    hid_protocol: 0,
    hid_iface: 0,
    ep_in: 0,
    ep_in_maxpkt: 0,
    ep_in_interval: 0,
    hub_addr: 0,
    hub_port: 0,
    is_hub: false,
    n_int_in: 0,
    int_in_ep: [0; 4],
    int_in_iface: [0; 4],
    int_in_proto: [0; 4],
}; MAX_TOPO];
static mut TOPO_N: u8 = 0;
static mut NEXT_ADDR: u8 = 2;

fn setup_hub_desc(len: u16) -> [u8; 8] {
    [0xA0, 0x06, 0, crate::kernel::usb_hub::DESC_HUB, 0, 0, len as u8, (len >> 8) as u8]
}
fn setup_port_set(feat: u16, port: u8) -> [u8; 8] {
    [0x23, 0x03, feat as u8, (feat >> 8) as u8, port, 0, 0, 0]
}
fn setup_port_clear(feat: u16, port: u8) -> [u8; 8] {
    [0x23, 0x01, feat as u8, (feat >> 8) as u8, port, 0, 0, 0]
}
fn setup_port_status(port: u8) -> [u8; 8] {
    [0xA3, 0x00, 0, 0, port, 0, 4, 0]
}

fn class_label(d: &UsbDevice) -> &'static str {
    if d.is_hub {
        "HUB"
    } else if d.is_hid {
        match d.hid_protocol {
            1 => "HID-keyboard",
            2 => "HID-mouse",
            _ => "HID",
        }
    } else {
        "non-HID"
    }
}

fn push_topo(d: UsbDevice) -> Result<(), &'static str> {
    unsafe {
        if TOPO_N as usize >= MAX_TOPO {
            return Err("usb: topology full");
        }
        TOPO[TOPO_N as usize] = d;
        TOPO_N += 1;
    }
    Ok(())
}

fn enumerate_child(hub_addr: u8, port: u8, eps: u32, addr: u8) -> Result<UsbDevice, &'static str> {
    set_split(hub_addr, port);
    delay_ms(10);
    let d18 = get_device_descriptor(eps)?;
    let ep0_maxpkt = if d18[7] == 0 { 8 } else { d18[7] as u16 };
    let mut dev = UsbDevice {
        addr: 0,
        vid: (d18[8] as u16) | ((d18[9] as u16) << 8),
        pid: (d18[10] as u16) | ((d18[11] as u16) << 8),
        dev_class: d18[4],
        ep0_maxpkt,
        eps,
        hub_addr,
        hub_port: port,
        is_hub: d18[4] == CLASS_HUB,
        ..UsbDevice::default()
    };
    control_transfer(0, ep0_maxpkt, eps, &setup_set_address(addr), None, false)?;
    delay_ms(5);
    dev.addr = addr;
    let mut hdr = [0u8; 9];
    control_transfer(addr, ep0_maxpkt, eps, &setup_get_config(0, 9), Some(&mut hdr), true)?;
    let total = (((hdr[3] as usize) << 8) | hdr[2] as usize).min(256);
    let mut blob = [0u8; 256];
    let n = control_transfer(
        addr,
        ep0_maxpkt,
        eps,
        &setup_get_config(0, total as u16),
        Some(&mut blob[..total]),
        true,
    )?;
    parse_config(&blob[..n], &mut dev);
    let cfg = if dev.config_value == 0 { 1 } else { dev.config_value };
    control_transfer(addr, ep0_maxpkt, eps, &setup_set_config(cfg), None, false)?;
    dev.config_value = cfg;
    set_split(0, 0);
    Ok(dev)
}

fn read_port(hub: &UsbDevice, port: u8) -> Result<(u16, u16), &'static str> {
    let mut st = [0u8; 4];
    control_transfer(
        hub.addr,
        hub.ep0_maxpkt,
        hub.eps,
        &setup_port_status(port),
        Some(&mut st),
        true,
    )?;
    Ok((
        u16::from_le_bytes([st[0], st[1]]),
        u16::from_le_bytes([st[2], st[3]]),
    ))
}

fn reset_enum_child(
    hub: &UsbDevice,
    port: u8,
    status: u16,
    change: u16,
    diag: &mut String,
) -> Result<Option<UsbDevice>, &'static str> {
    let _ = control_transfer(
        hub.addr,
        hub.ep0_maxpkt,
        hub.eps,
        &setup_port_clear(crate::kernel::usb_hub::FEAT_C_PORT_CONNECTION, port),
        None,
        false,
    );
    let _ = control_transfer(
        hub.addr,
        hub.ep0_maxpkt,
        hub.eps,
        &setup_port_set(crate::kernel::usb_hub::FEAT_PORT_RESET, port),
        None,
        false,
    );
    let mut status2 = status;
    let mut change2 = change;
    let mut reset_done = false;
    for _ in 0..25 {
        delay_ms(10);
        if let Ok((s, c)) = read_port(hub, port) {
            status2 = s;
            change2 = c;
            if (c & (1 << 4)) != 0 || (s & crate::kernel::usb_hub::PORT_ENABLE) != 0 {
                reset_done = true;
                break;
            }
        }
    }
    diag.push_str(&alloc::format!(
        "hub port {port}: after-reset status=0x{status2:04x} change=0x{change2:04x} done={reset_done}\n"
    ));
    let _ = control_transfer(
        hub.addr,
        hub.ep0_maxpkt,
        hub.eps,
        &setup_port_clear(crate::kernel::usb_hub::FEAT_C_PORT_RESET, port),
        None,
        false,
    );
    if !reset_done && !crate::kernel::usb_hub::port_connected(status2) {
        return Ok(None);
    }
    let child_eps = crate::kernel::usb_hub::port_status_speed(status2);
    let addr = unsafe {
        let a = NEXT_ADDR;
        if a == 0 || a > 8 {
            return Err("usb: address exhausted");
        }
        NEXT_ADDR = a + 1;
        a
    };
    match enumerate_child(hub.addr, port, child_eps, addr) {
        Ok(child) => {
            diag.push_str(&alloc::format!(
                "hub port {port}: child {:04x}:{:04x} class={} addr={}\n",
                child.vid,
                child.pid,
                class_label(&child),
                child.addr
            ));
            Ok(Some(child))
        }
        Err(e) => {
            diag.push_str(&alloc::format!("hub port {port}: enum {e}\n"));
            set_split(0, 0);
            Ok(None)
        }
    }
}

fn enum_hub_leaves(hub: &UsbDevice, diag: &mut String) {
    set_split(0, 0);
    delay_ms(20);
    let mut hdr = [0u8; 16];
    let mut n = match control_transfer(
        hub.addr,
        hub.ep0_maxpkt,
        hub.eps,
        &setup_hub_desc(9),
        Some(&mut hdr[..9]),
        true,
    ) {
        Ok(n) => n,
        Err(_) => match control_transfer(
            hub.addr,
            hub.ep0_maxpkt,
            hub.eps,
            &setup_hub_desc(15),
            Some(&mut hdr[..15]),
            true,
        ) {
            Ok(n) => n,
            Err(e) => {
                diag.push_str(&alloc::format!("hub leaves: desc {e}\n"));
                return;
            }
        },
    };
    if n > hdr.len() {
        n = hdr.len();
    }
    let desc = match crate::kernel::usb_hub::parse_hub_descriptor(&hdr[..n]) {
        Ok(d) => d,
        Err(e) => {
            diag.push_str(&alloc::format!("hub leaves: parse {}\n", e.as_str()));
            return;
        }
    };
    let wait_ms = (desc.pwr_on_2_pwr_good as u64)
        .saturating_mul(2)
        .max(200);
    for port in 1..=desc.ports {
        let _ = control_transfer(
            hub.addr,
            hub.ep0_maxpkt,
            hub.eps,
            &setup_port_set(crate::kernel::usb_hub::FEAT_PORT_POWER, port),
            None,
            false,
        );
    }
    delay_ms(wait_ms);
    for port in 1..=desc.ports {
        let (status, change) = match read_port(hub, port) {
            Ok(v) => v,
            Err(e) => {
                diag.push_str(&alloc::format!("hub port {port}: status {e}\n"));
                continue;
            }
        };
        diag.push_str(&alloc::format!(
            "hub port {port}: status=0x{status:04x} change=0x{change:04x}\n"
        ));
        if crate::kernel::usb_hub::port_overcurrent(status)
            || !crate::kernel::usb_hub::port_connected(status)
        {
            continue;
        }
        match reset_enum_child(hub, port, status, change, diag) {
            Ok(Some(child)) => {
                let nested = child.is_hub;
                let _ = push_topo(child);
                if nested {
                    diag.push_str(&alloc::format!(
                        "hub port {port}: nested hub not expanded\n"
                    ));
                }
            }
            Ok(None) => {}
            Err(e) => diag.push_str(&alloc::format!("hub port {port}: {e}\n")),
        }
    }
    set_split(0, 0);
}

fn expand_hub(hub: &UsbDevice, depth: u8, diag: &mut String) -> Result<(), &'static str> {
    if let Err(e) = crate::kernel::usb_hub::refuse_nested(depth) {
        enum_hub_leaves(hub, diag);
        return Err(e.as_str());
    }
    set_split(0, 0);
    delay_ms(20);
    let mut hdr = [0u8; 16];
    let mut n = match control_transfer(
        hub.addr,
        hub.ep0_maxpkt,
        hub.eps,
        &setup_hub_desc(9),
        Some(&mut hdr[..9]),
        true,
    ) {
        Ok(n) => n,
        Err(_) => control_transfer(
            hub.addr,
            hub.ep0_maxpkt,
            hub.eps,
            &setup_hub_desc(15),
            Some(&mut hdr[..15]),
            true,
        )?,
    };
    if n > hdr.len() {
        n = hdr.len();
    }
    let desc = crate::kernel::usb_hub::parse_hub_descriptor(&hdr[..n]).map_err(|e| e.as_str())?;
    diag.push_str(&alloc::format!(
        "hub: ports={} pwr2good={} char=0x{:04x}\n",
        desc.ports,
        desc.pwr_on_2_pwr_good,
        desc.characteristics
    ));
    let wait_ms = (desc.pwr_on_2_pwr_good as u64)
        .saturating_mul(2)
        .max(200);
    for port in 1..=desc.ports {
        match control_transfer(
            hub.addr,
            hub.ep0_maxpkt,
            hub.eps,
            &setup_port_set(crate::kernel::usb_hub::FEAT_PORT_POWER, port),
            None,
            false,
        ) {
            Ok(_) => {}
            Err(e) => diag.push_str(&alloc::format!("hub port {port}: power {e}\n")),
        }
    }
    delay_ms(wait_ms);
    for port in 1..=desc.ports {
        let (status, change) = match read_port(hub, port) {
            Ok(v) => v,
            Err(e) => {
                diag.push_str(&alloc::format!("hub port {port}: status {e}\n"));
                continue;
            }
        };
        diag.push_str(&alloc::format!(
            "hub port {port}: status=0x{status:04x} change=0x{change:04x}\n"
        ));
        if crate::kernel::usb_hub::port_overcurrent(status) {
            continue;
        }
        if !crate::kernel::usb_hub::port_connected(status) {
            continue;
        }
        match reset_enum_child(hub, port, status, change, diag) {
            Ok(Some(child)) => {
                if child.is_hub {
                    push_topo(child)?;
                    if let Err(e) = expand_hub(&child, depth + 1, diag) {
                        diag.push_str(&alloc::format!(
                            "hub port {port}: nested expand {e}\n"
                        ));
                    }
                } else {
                    push_topo(child)?;
                }
            }
            Ok(None) => {}
            Err(e) => return Err(e),
        }
    }
    set_split(0, 0);
    Ok(())
}

pub fn enumerate_tree() -> Result<(UsbDevice, PortStatus, u8), &'static str> {
    let mut diag = String::new();
    enumerate_tree_diag(&mut diag)
}

fn enumerate_tree_diag(diag: &mut String) -> Result<(UsbDevice, PortStatus, u8), &'static str> {
    unsafe {
        TOPO_N = 0;
        NEXT_ADDR = 2;
    }
    set_split(0, 0);
    let (root, st) = enumerate()?;
    push_topo(root)?;
    if root.is_hub {
        if let Err(e) = expand_hub(&root, 1, diag) {
            diag.push_str("hub expand: ");
            diag.push_str(e);
            diag.push('\n');
        }
    }
    Ok((root, st, unsafe { TOPO_N }))
}

fn format_slot(i: usize, d: &UsbDevice, speed: &str) -> String {
    let ep = if d.ep_in != 0 {
        alloc::format!(
            " int-in=0x{:02x} maxpkt={} interval={}",
            d.ep_in,
            d.ep_in_maxpkt,
            d.ep_in_interval
        )
    } else {
        String::new()
    };
    let hub = if d.hub_addr != 0 {
        alloc::format!(" hub={} port={}", d.hub_addr, d.hub_port)
    } else {
        String::new()
    };
    alloc::format!(
        "usb{}: {:04x}:{:04x} class={} addr={} cfg={} ep0={} {}{}{}",
        i + 1,
        d.vid,
        d.pid,
        class_label(d),
        d.addr,
        d.config_value,
        d.ep0_maxpkt,
        speed,
        ep,
        hub
    )
}

/// `usb list` — enumerate the attached device and describe it for the shell.
pub fn list() -> (String, PortStatus) {
    let mut diag = String::new();
    match enumerate_tree_diag(&mut diag) {
        Ok((d, st, n)) => {
            let mut report = String::new();
            unsafe {
                for i in 0..n as usize {
                    let speed = if i == 0 {
                        st.speed
                    } else {
                        match TOPO[i].eps {
                            2 => "high-speed",
                            1 => "low-speed",
                            _ => "full-speed",
                        }
                    };
                    if !report.is_empty() {
                        report.push('\n');
                    }
                    report.push_str(&format_slot(i, &TOPO[i], speed));
                }
            }
            if report.is_empty() {
                report = format_slot(0, &d, st.speed);
            }
            if !diag.is_empty() {
                report.push('\n');
                report.push_str(&diag);
            }
            (report, st)
        }
        Err(e) => (
            alloc::format!("usb: {e}"),
            PortStatus {
                connected: false,
                enabled: false,
                reset_done: false,
                speed: "n/a",
            },
        ),
    }
}

// ── U2: HID interrupt-IN polling + boot protocol (keyboard/mouse reports) ───
//
// Pragmatic async-schedule polling: issue a single IN qTD on the interrupt
// endpoint and wait (software timeout). A full EHCI periodic frame list is
// overkill for one low-rate HID device. Data toggle is tracked across polls.

static mut SPLIT_HUB: u8 = 0;
static mut SPLIT_PORT: u8 = 0;
static mut INT_TOGGLE: u32 = 0;

fn set_split(hub: u8, port: u8) {
    unsafe {
        SPLIT_HUB = hub;
        SPLIT_PORT = port;
    }
}

fn qh_ep_cap() -> u32 {
    let mut cap = 1u32 << 30;
    unsafe {
        if SPLIT_HUB != 0 && SPLIT_PORT != 0 {
            cap |= (SPLIT_HUB as u32) << 16;
            cap |= (SPLIT_PORT as u32) << 23;
        }
    }
    cap
}

#[inline]
fn setup_set_protocol(iface: u8, boot: bool) -> [u8; 8] {
    // bmRequestType=0x21 (host->dev, class, interface), bRequest=SET_PROTOCOL(0x0B),
    // wValue = 0 boot / 1 report, wIndex = interface.
    [0x21, 0x0B, if boot { 0 } else { 1 }, 0, iface, 0, 0, 0]
}

/// One interrupt-IN transfer on `ep` (e.g. 0x83). Returns Ok(n) with `n` bytes in
/// `buf` on a report, Ok(0) if none arrived within `timeout_ms` (NAK), Err on halt.
pub fn interrupt_in(
    dev_addr: u8,
    ep: u8,
    maxpkt: u16,
    eps: u32,
    buf: &mut [u8],
    timeout_ms: u32,
) -> Result<usize, &'static str> {
    unsafe {
        let mut toggle = INT_TOGGLE;
        let n = interrupt_in_toggle(dev_addr, ep, maxpkt, eps, buf, timeout_ms, &mut toggle)?;
        INT_TOGGLE = toggle;
        Ok(n)
    }
}

/// Interrupt-IN with caller-owned data toggle (one toggle per endpoint).
pub fn interrupt_in_toggle(
    dev_addr: u8,
    ep: u8,
    maxpkt: u16,
    eps: u32,
    buf: &mut [u8],
    timeout_ms: u32,
    toggle: &mut u32,
) -> Result<usize, &'static str> {
    let want = buf.len().min(maxpkt as usize).min(512);
    unsafe {
        let qtd_p = phys(core::ptr::addr_of!(CTRL_QTD) as *const Qtd);

        // Single IN qTD.
        let tok_toggle = if *toggle != 0 { TOGGLE1 } else { 0 };
        CTRL_QTD[0].next = TERMINATE;
        CTRL_QTD[0].alt_next = TERMINATE;
        CTRL_QTD[0].token =
            QTD_ACTIVE | PID_IN | QTD_CERR3 | QTD_IOC | ((want as u32) << 16) | tok_toggle;
        set_qtd_bufs(
            &mut CTRL_QTD[0],
            phys(core::ptr::addr_of!(DATA_BUF) as *const u8),
            want,
        );

        // QH for the interrupt endpoint, run on the async schedule.
        CTRL_QH.horiz = phys(core::ptr::addr_of!(CTRL_QH) as *const Qh) | (1 << 1);
        CTRL_QH.ep_char = (dev_addr as u32)
            | ((ep as u32 & 0x0F) << 8)
            | (eps << 12)
            | (1 << 14) // DTC: use qTD data toggle
            | (1 << 15) // H: head of reclamation list
            | ((maxpkt as u32) << 16);
        CTRL_QH.ep_cap = qh_ep_cap();
        CTRL_QH.cur_qtd = 0;
        CTRL_QH.ov_next = qtd_p;
        CTRL_QH.ov_alt = TERMINATE;
        CTRL_QH.ov_token = 0;

        dma_clean(
            phys(core::ptr::addr_of!(DATA_BUF) as *const u8),
            want as u32,
        );
        dma_clean(
            phys(core::ptr::addr_of!(CTRL_QTD) as *const Qtd),
            core::mem::size_of::<Qtd>() as u32,
        );
        dma_clean(
            phys(core::ptr::addr_of!(CTRL_QH) as *const Qh),
            core::mem::size_of::<Qh>() as u32,
        );
        dma_invalidate(
            phys(core::ptr::addr_of!(CTRL_QTD) as *const Qtd),
            core::mem::size_of::<Qtd>() as u32,
        );

        usb_dma_submit_ctrl()?;

        w(
            OFF_ASYNCLISTADDR,
            phys(core::ptr::addr_of!(CTRL_QH) as *const Qh),
        );
        w(OFF_CMD, r(OFF_CMD) | CMD_ASE);

        let mut ms = 0u32;
        loop {
            dma_invalidate(
                phys(core::ptr::addr_of!(CTRL_QTD) as *const Qtd),
                core::mem::size_of::<Qtd>() as u32,
            );
            let t = core::ptr::read_volatile(core::ptr::addr_of!(CTRL_QTD[0].token));
            if (t & QTD_HALTED) != 0 {
                w(OFF_CMD, r(OFF_CMD) & !CMD_ASE);
                usb_dma_complete_ctrl();
                return Err("usb: interrupt transfer halted");
            }
            if (t & QTD_ACTIVE) == 0 {
                break; // a report retired
            }
            delay_ms(1);
            ms += 1;
            if ms >= timeout_ms {
                w(OFF_CMD, r(OFF_CMD) & !CMD_ASE);
                usb_dma_complete_ctrl();
                return Ok(0); // no report this window (NAK)
            }
        }
        w(OFF_CMD, r(OFF_CMD) & !CMD_ASE);
        usb_dma_complete_ctrl();
        *toggle ^= 1;

        dma_invalidate(
            phys(core::ptr::addr_of!(CTRL_QTD) as *const Qtd),
            core::mem::size_of::<Qtd>() as u32,
        );
        dma_invalidate(
            phys(core::ptr::addr_of!(DATA_BUF) as *const u8),
            want as u32,
        );
        let residual = ((core::ptr::read_volatile(core::ptr::addr_of!(CTRL_QTD[0].token)) >> 16)
            & 0x7FFF) as usize;
        let got = want.saturating_sub(residual);
        let n = got.min(buf.len());
        buf[..n].copy_from_slice(&DATA_BUF.0[..n]);
        Ok(n)
    }
}

/// `usb keys` — enumerate, request the HID boot protocol, then poll the
/// interrupt-IN endpoint for ~5 s and print each raw 8-byte report as it changes.
/// Proves the input pipe before wiring decoded chars into the shell (P3).
pub fn keys() -> String {
    let mut diag = String::new();
    let (_root, _st, n) = match enumerate_tree_diag(&mut diag) {
        Ok(t) => t,
        Err(e) => return alloc::format!("usb keys: {e}"),
    };
    let mut out = String::new();
    let mut any_hid = false;
    let mut last = [[0xFFu8; 8]; MAX_TOPO];
    let mut seen = [0u32; MAX_TOPO];
    let mut toggle = [0u32; MAX_TOPO];
    unsafe {
        for i in 0..n as usize {
            let dev = TOPO[i];
            if !dev.is_hid || dev.ep_in == 0 {
                continue;
            }
            any_hid = true;
            set_split(dev.hub_addr, dev.hub_port);
            let sp = setup_set_protocol(dev.hid_iface, true);
            let _ = control_transfer(dev.addr, dev.ep0_maxpkt, dev.eps, &sp, None, false);
            out.push_str(&alloc::format!(
                "usb keys: {:04x}:{:04x} addr={} proto={} ep=0x{:02x}\n",
                dev.vid, dev.pid, dev.addr, dev.hid_protocol, dev.ep_in
            ));
        }
        for _ in 0..200 {
            for i in 0..n as usize {
                let dev = TOPO[i];
                if !dev.is_hid || dev.ep_in == 0 {
                    continue;
                }
                set_split(dev.hub_addr, dev.hub_port);
                let mut buf = [0u8; 8];
                match interrupt_in_toggle(
                    dev.addr,
                    dev.ep_in,
                    dev.ep_in_maxpkt,
                    dev.eps,
                    &mut buf,
                    8,
                    &mut toggle[i],
                ) {
                    Ok(0) => {}
                    Ok(_) if buf != last[i] => {
                        out.push_str(&alloc::format!(
                            "  {:04x}:{:04x} proto={} report: {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}\n",
                            dev.vid, dev.pid, dev.hid_protocol,
                            buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]
                        ));
                        last[i] = buf;
                        seen[i] += 1;
                    }
                    _ => {}
                }
            }
        }
        set_split(0, 0);
        for i in 0..n as usize {
            let dev = TOPO[i];
            if dev.is_hid && dev.ep_in != 0 {
                out.push_str(&alloc::format!(
                    "usb keys: {} report(s) captured from {:04x}:{:04x} proto={}\n",
                    seen[i], dev.vid, dev.pid, dev.hid_protocol
                ));
            }
        }
    }
    if !any_hid {
        if diag.is_empty() {
            return alloc::format!("usb keys: no interrupt-IN endpoint (not an HID input device)");
        }
        return alloc::format!(
            "usb keys: no interrupt-IN endpoint (not an HID input device)\n{diag}"
        );
    }
    out
}

// ── U2 shell bridge: a persistent keyboard the shell can poll for characters ─
//
// `attach_keyboard()` enumerates once (heavy — never call it in the tight shell
// loop) and caches the device. `poll_key()` is cheap: one short interrupt-IN and
// a HID-usage→ASCII decode. Simplification (honest): boot protocol, first keycode
// only (`buf[2]`), no n-key rollover — all a command line needs.

static mut KBD: Option<UsbDevice> = None;
// Proper HID boot-keyboard state: the previous report's 6-key array (for key-down
// edge detection) + a small ring of decoded chars pending delivery to the shell.
static mut PREV_KEYS: [u8; 6] = [0; 6];
const PEND_LEN: usize = 16;
static mut PEND: [u8; PEND_LEN] = [0; PEND_LEN];
static mut PEND_HEAD: usize = 0;
static mut PEND_TAIL: usize = 0;

/// Map a HID boot-keyboard usage code to ASCII (with shift). Returns `None` for
/// keys with no printable/line-editing meaning here.
fn hid_to_ascii(code: u8, shift: bool) -> Option<u8> {
    let c = match code {
        0x04..=0x1d => {
            let base = b'a' + (code - 0x04); // a..z
            if shift {
                base - 32
            } else {
                base
            }
        }
        0x1e..=0x26 => {
            if shift {
                match code {
                    0x1e => b'!',
                    0x1f => b'@',
                    0x20 => b'#',
                    0x21 => b'$',
                    0x22 => b'%',
                    0x23 => b'^',
                    0x24 => b'&',
                    0x25 => b'*',
                    0x26 => b'(',
                    _ => return None,
                }
            } else {
                b'1' + (code - 0x1e) // 1..9
            }
        }
        0x27 => {
            if shift {
                b')'
            } else {
                b'0'
            }
        }
        0x28 => b'\n', // Enter
        0x2a => 0x08,  // Backspace
        0x2b => b'\t', // Tab
        0x2c => b' ',  // Space
        0x2d => {
            if shift {
                b'_'
            } else {
                b'-'
            }
        }
        0x2e => {
            if shift {
                b'+'
            } else {
                b'='
            }
        }
        0x2f => {
            if shift {
                b'{'
            } else {
                b'['
            }
        }
        0x30 => {
            if shift {
                b'}'
            } else {
                b']'
            }
        }
        0x31 => {
            if shift {
                b'|'
            } else {
                b'\\'
            }
        }
        0x33 => {
            if shift {
                b':'
            } else {
                b';'
            }
        }
        0x34 => {
            if shift {
                b'"'
            } else {
                b'\''
            }
        }
        0x35 => {
            if shift {
                b'~'
            } else {
                b'`'
            }
        }
        0x36 => {
            if shift {
                b'<'
            } else {
                b','
            }
        }
        0x37 => {
            if shift {
                b'>'
            } else {
                b'.'
            }
        }
        0x38 => {
            if shift {
                b'?'
            } else {
                b'/'
            }
        }
        _ => return None,
    };
    Some(c)
}

/// Enumerate + boot-protocol a keyboard and cache it for the shell to poll.
/// Returns the device on success. Heavy (`enumerate` waits on the port) — call
/// once from the `usb kbd` command, never in the shell loop.
pub fn attach_keyboard() -> Result<UsbDevice, &'static str> {
    let (dev, _st) = enumerate()?;
    if dev.ep_in == 0 {
        return Err("usb: no HID interrupt-IN endpoint (not a keyboard?)");
    }
    let sp = setup_set_protocol(dev.hid_iface, true);
    let _ = control_transfer(dev.addr, dev.ep0_maxpkt, dev.eps, &sp, None, false);
    unsafe {
        KBD = Some(dev);
        PREV_KEYS = [0; 6];
        PEND_HEAD = 0;
        PEND_TAIL = 0;
    }
    Ok(dev)
}

/// Poll the attached keyboard for one newly-pressed character. Cheap: returns
/// `None` if no keyboard is attached. Proper HID boot-keyboard handling — a
/// character is emitted once, on the key-DOWN edge (a keycode present in this
/// report but not the previous one), across the full 6-key array. This handles
/// key rollover: typing fast, where several keys overlap, no longer scrambles or
/// duplicates. Extra new-downs in one report are queued and drained on later polls.
pub fn poll_key() -> Option<u8> {
    unsafe {
        // Drain any queued characters first.
        if PEND_HEAD != PEND_TAIL {
            let c = PEND[PEND_HEAD];
            PEND_HEAD = (PEND_HEAD + 1) % PEND_LEN;
            return Some(c);
        }
        let dev = match KBD {
            Some(d) => d,
            None => return None,
        };
        let mut buf = [0u8; 8];
        let n = interrupt_in(dev.addr, dev.ep_in, dev.ep_in_maxpkt, dev.eps, &mut buf, 2)
            .unwrap_or(0);
        rearm_controller_irq();
        if n < 3 {
            return None;
        }
        let shift = (buf[0] & 0x22) != 0; // LShift | RShift
        let cur = [buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]];
        for &kc in cur.iter() {
            if kc == 0 {
                continue;
            }
            let was_down = PREV_KEYS.iter().any(|&p| p == kc);
            if !was_down {
                if let Some(ch) = hid_to_ascii(kc, shift) {
                    let next = (PEND_TAIL + 1) % PEND_LEN;
                    if next != PEND_HEAD {
                        PEND[PEND_TAIL] = ch;
                        PEND_TAIL = next;
                    }
                }
            }
        }
        PREV_KEYS = cur;
        if PEND_HEAD != PEND_TAIL {
            let c = PEND[PEND_HEAD];
            PEND_HEAD = (PEND_HEAD + 1) % PEND_LEN;
            return Some(c);
        }
        None
    }
}

/// Stop the boot-keyboard fallback so the HID host can own the interrupt pipe.
pub fn release_keyboard_fallback() {
    unsafe {
        KBD = None;
        PREV_KEYS = [0; 6];
        PEND_HEAD = 0;
        PEND_TAIL = 0;
    }
}

pub fn port_ccs() -> bool {
    unsafe { (r(OFF_PORTSC1) & PORTSC_CCS) != 0 }
}

pub fn ticks() -> u64 {
    read_gtc()
}

pub const TICKS_PER_MILLISECOND: u64 = TICKS_PER_MS;

pub fn get_config_blob(
    addr: u8,
    ep0_maxpkt: u16,
    eps: u32,
    buf: &mut [u8],
) -> Result<usize, &'static str> {
    let mut hdr = [0u8; 9];
    control_transfer(
        addr,
        ep0_maxpkt,
        eps,
        &setup_get_config(0, 9),
        Some(&mut hdr),
        true,
    )?;
    let total = (((hdr[3] as usize) << 8) | hdr[2] as usize)
        .min(buf.len())
        .min(512);
    control_transfer(
        addr,
        ep0_maxpkt,
        eps,
        &setup_get_config(0, total as u16),
        Some(&mut buf[..total]),
        true,
    )
}

pub fn hid_control_in(
    addr: u8,
    ep0_maxpkt: u16,
    eps: u32,
    setup: &[u8; 8],
    data: &mut [u8],
) -> Result<usize, &'static str> {
    control_transfer(addr, ep0_maxpkt, eps, setup, Some(data), true)
}

pub fn hid_control_nodata(
    addr: u8,
    ep0_maxpkt: u16,
    eps: u32,
    setup: &[u8; 8],
) -> Result<usize, &'static str> {
    control_transfer(addr, ep0_maxpkt, eps, setup, None, false)
}
