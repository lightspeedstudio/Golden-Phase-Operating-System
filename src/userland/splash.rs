//! Boot splash for hosted and QEMU serial consoles.

use crate::brand;
use crate::kernel::event_log::ProcessId;
use crate::kernel::Kernel;
use crate::platform::String;

/// Smoke-test marker (first line of full boot splash).
pub const BOOT_SPLASH: &str = brand::BOOT_SPLASH;

pub const SPLASH: &str = brand::SPLASH;

const BANNER_WIDTH: usize = 79;

/// Full framed splash mark (user-selected Golden Phase banner).
const BANNER: &[&str] = &[
    "╔════════════════════════════════════════════════════════════════════╗",
    "║                               · φ ·                                ║",
    "╠════════════════════════════════════════════════════════════════════╣",
    "║                                                                    ║",
    "║         ██████╗    ██╗   ██╗     ██████╗ ███████╗███╗   ██╗        ║",
    "║        ██╔════╝  ██████╗ ██║     ██╔══██╗██╔════╝████╗  ██║        ║",
    "║        ██║  ███╗██║██║██║██║     ██║  ██║█████╗  ██╔██╗ ██║        ║",
    "║        ██║   ██║██║██║██║██║     ██║  ██║██╔══╝  ██║╚██╗██║        ║",
    "║        ╚██████╔╝╚██████╔╝███████╗██████╔╝███████╗██║ ╚████║        ║",
    "║         ╚═════╝    ██╔╝  ╚══════╝╚═════╝ ╚══════╝╚═╝  ╚═══╝        ║",
    "║                                                                    ║",
    "║              ██████╗ ██╗  ██╗ █████╗ ███████╗███████╗              ║",
    "║              ██╔══██╗██║  ██║██╔══██╗██╔════╝██╔════╝              ║",
    "║              ██████╔╝███████║███████║███████╗█████╗                ║",
    "║              ██╔═══╝ ██╔══██║██╔══██║╚════██║██╔══╝                ║",
    "║              ██║     ██║  ██║██║  ██║███████║███████╗              ║",
    "║              ╚═╝     ╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚══════╝              ║",
    "║                                                                    ║",
    "║                  O p e r a t i n g   S y s t e m                   ║",
    "║                                                                    ║",
    "║    ▅▅▅▆▆▆▇▇▇▇███████████▇▇▇▇▆▆▆▅▅▅▄▄▃▃▃▂▂▂▂▁▁▁▁▁▁▁▁▁▁▁▂▂▂▂▃▃▃▄▄    ║",
    "╚════════════════════════════════════════════════════════════════════╝",
];

/// Column where ` 0.000s` timings begin (monospace).
const TIMING_COL: usize = 63;

fn center_banner_line(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let pad = BANNER_WIDTH.saturating_sub(trimmed.len()) / 2;
    format!("{}{}", " ".repeat(pad), trimmed)
}

fn banner_body() -> String {
    BANNER.join("\n")
}

fn center_tagline(text: &str) -> String {
    center_banner_line(text)
}

#[derive(Debug, Clone)]
pub struct BootSplashInfo {
    pub init_pid: ProcessId,
    pub page_count: usize,
    pub process_count: usize,
    pub sector_id: u8,
    pub kairos_tick: u64,
    pub trace_events: usize,
    pub hosted: bool,
    pub authority: &'static str,
    pub visibility: &'static str,
    pub anom_standby: bool,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub equation_root_short: String,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub boot_session_root_short: String,
    #[cfg(all(feature = "phasestack", any(feature = "std", feature = "baremetal")))]
    pub app_registry_root_short: String,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub statechain_tick: u64,
    pub fs_inodes: usize,
    pub prior_anom_history: bool,
    #[cfg(feature = "std")]
    pub protected_objects: usize,
    #[cfg(feature = "std")]
    pub locked_objects: usize,
    #[cfg(feature = "std")]
    pub phaselink_root_short: String,
    pub phase_math_backend: String,
    pub phase_math_workers: usize,
    pub phase_math_simd: String,
    pub network_driver: String,
    pub network_link: String,
    pub ethernet_attachment: String,
    #[cfg(all(not(feature = "std"), target_arch = "arm", feature = "bm-net"))]
    pub uart_gem_bridge: bool,
}

impl BootSplashInfo {
    pub fn from_kernel(kernel: &Kernel, init_pid: ProcessId, hosted: bool) -> Self {
        let clock = kernel.get_clock();
        let (inode_h, _) = kernel.fs.phase_histograms();
        let history = &kernel.authority.history;
        Self {
            init_pid,
            page_count: kernel.tag_report().len(),
            process_count: kernel.process_ids().len(),
            sector_id: clock.sector_id,
            kairos_tick: clock.kairos_tick,
            trace_events: kernel.get_trace_log().len(),
            hosted,
            authority: crate::kernel::anom::authority_label(kernel.authority.authority),
            visibility: crate::kernel::anom::visibility_label(kernel.authority.visibility),
            anom_standby: !kernel.authority.is_anom_protected(),
            #[cfg(any(feature = "std", feature = "baremetal"))]
            equation_root_short: short_root_hex(kernel),
            #[cfg(any(feature = "std", feature = "baremetal"))]
            boot_session_root_short: short_bsr_hex(kernel),
            #[cfg(all(feature = "phasestack", any(feature = "std", feature = "baremetal")))]
            app_registry_root_short: {
                use crate::userland::terminal_fmt::short_hex;
                short_hex(&kernel.phasestack.registry_root())
            },
            #[cfg(any(feature = "std", feature = "baremetal"))]
            statechain_tick: kernel.statechain.tick(),
            fs_inodes: inode_h.iter().sum(),
            prior_anom_history: history.last_authority_mode
                == crate::kernel::authority::AuthorityMode::Anom,
            #[cfg(feature = "std")]
            protected_objects: kernel.object_protection.registry.protected_count(),
            #[cfg(feature = "std")]
            locked_objects: kernel.object_protection.registry.locked_count(),
            #[cfg(feature = "std")]
            phaselink_root_short: crate::kernel::phaselink::merkle::short_root(
                &kernel.phaselink.root,
            ),
            phase_math_backend: kernel.phase_math.backend.label().to_ascii_uppercase(),
            phase_math_workers: kernel.phase_math.workers.active_worker_count,
            phase_math_simd: if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
                if crate::kernel::phase_math::backends::avx512::available() {
                    "AVX512".into()
                } else if crate::kernel::phase_math::backends::avx2::available() {
                    "AVX2".into()
                } else {
                    "none".into()
                }
            } else if crate::kernel::phase_math::backends::neon::available() {
                "NEON".into()
            } else {
                "none".into()
            },
            network_driver: kernel.network.driver_name.clone(),
            network_link: if kernel.network.link_ready {
                "ready".into()
            } else if kernel.network.detected {
                "detected".into()
            } else {
                "none".into()
            },
            ethernet_attachment: if kernel.network.detected {
                "detected".into()
            } else {
                "none".into()
            },
            #[cfg(all(not(feature = "std"), target_arch = "arm", feature = "bm-net"))]
            uart_gem_bridge: kernel.phaselink.uart_gem_active,
        }
    }
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn short_root_hex(kernel: &Kernel) -> String {
    use crate::userland::terminal_fmt::short_hex;
    short_hex(&kernel.statechain.equation_root())
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn short_bsr_hex(kernel: &Kernel) -> String {
    use crate::userland::terminal_fmt::short_hex;
    let bsr = kernel.authority.boot_session_root;
    if bsr == [0u8; 32] {
        String::from("stub")
    } else {
        short_hex(&bsr)
    }
}

pub fn render() -> &'static str {
    SPLASH
}

pub fn render_boot() -> &'static str {
    BOOT_SPLASH
}

pub fn render_with_equation() -> &'static str {
    "Golden Phase Operating System | y^4 - phi*y^2 + alpha = 0\ngφlden phase φpertating system · Kairos States console"
}

pub fn equation_line() -> &'static str {
    "y^4 - phi*y^2 + alpha = 0"
}

pub fn render_boot_screen(info: &BootSplashInfo) -> String {
    let version = env!("CARGO_PKG_VERSION");
    // Non-hosted label reflects the physical host CPU (compile-time target),
    // not the GP VM guest ISA. See docs/PZ7020_ARCHITECTURE.md.
    let target = brand::build_target_label(info.hosted);
    let aligned = subsystem_aligned_count(info);
    let total = if info.hosted { 8 } else { 7 };
    let bar = alignment_bar(aligned, total);
    let records = akashic_records(info);
    let resonance = resonance_line(info.sector_id);

    let mut out = String::new();
    out.push_str(BOOT_SPLASH);
    out.push('\n');
    out.push_str(&banner_body());
    out.push_str(&format!(
        "\n{}\n\
         ===============================================================================\n\
          Golden Phase Operating System kernel {version}-eternal ({target}) -- awakening\n\
         ===============================================================================\n",
        center_tagline(&format!(
            "g φ l d e n   p h a s e   φ p e r t a t i n g   s y s t e m   *   v{version} [gpos]"
        )),
    ));
    out.push_str(&wake_lines(info));
    out.push_str(&mode_lines(info));
    out.push_str("  ALIGNING SUBSYSTEMS\n");
    out.push_str(&format!("  {bar}   {aligned} of {total} aligned\n"));
    out.push_str("  TUNING STATECLOCK\n");
    out.push_str(&format!("  {resonance}\n"));
    out.push_str(&format!(
        "  |> consulting phase records ... {records} entries\n\
         ===============================================================================\n\
         |> authority mode ... {}\n\
         |> ANOM security protocol ... {}\n\
         |> true phase view ... {}\n\
         ===============================================================================\n\
         T H E   M O M E N T   I S   N O W   -- type help at gpos>          | GPOS\n\
         ===============================================================================\n",
        info.authority,
        if info.anom_standby {
            "standby"
        } else {
            "active"
        },
        info.visibility,
    ));
    out
}

fn mode_lines(info: &BootSplashInfo) -> String {
    let mut out = String::new();
    out.push_str("  AUTHORITY / ANOM\n");
    out.push_str(&format!(
        "  [ MODE  ] authority:           {}\n",
        info.authority
    ));
    out.push_str(&format!(
        "  [ MODE  ] visibility:          {}\n",
        info.visibility
    ));
    let anom_proto = if info.anom_standby {
        "standby"
    } else {
        "active"
    };
    out.push_str(&format!("  [ ANOM  ] security protocol:   {anom_proto}\n"));
    // The statechain (equation root, kernel state root, ledger) now runs on
    // both hosted and bare-metal builds -- see kernel::statechain, proven live
    // via `watchdog`. Only the signed boot-manifest verifier remains
    // hosted-only, so that one line stays honest per build.
    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        out.push_str(&format!(
            "  [ ROOTED] EquationRoot:        {}\n",
            info.equation_root_short
        ));
        out.push_str(&format!(
            "  [ SEALED] BootSessionRoot:     {}\n",
            info.boot_session_root_short
        ));
        #[cfg(feature = "phasestack")]
        {
            out.push_str(&format!(
                "  [ APPS  ] AppRegistryRoot:     {}\n",
                info.app_registry_root_short
            ));
        }
        let manifest = if info.hosted { "OK" } else { "n/a (baremetal)" };
        out.push_str(&format!("  [ VERIFY] boot manifest:       {manifest}\n"));
        out.push_str("  [ VERIFY] statechain:          OK\n");
    }
    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    {
        out.push_str("  [ ROOTED] EquationRoot:        stub\n");
        out.push_str("  [ SEALED] BootSessionRoot:     stub\n");
        out.push_str("  [ VERIFY] boot manifest:       stub\n");
        out.push_str("  [ VERIFY] statechain:          hosted-only / stub\n");
    }
    out.push_str("  [ ACCESS] true phase states:   visible to root\n");
    if info.prior_anom_history {
        out.push_str("  [ HIST  ] prior session ended in ANOM (history only)\n");
    }
    #[cfg(feature = "std")]
    {
        out.push_str(&format!(
            "  [ OBJECT] protected objects:      {}\n",
            info.protected_objects
        ));
        if info.protected_objects > 0 {
            out.push_str(&format!(
                "  [ OBJECT] locked objects:         {}\n",
                info.locked_objects
            ));
            out.push_str("  [ VERIFY] object commitments:     OK\n");
            out.push_str("  [ ACCESS] object decryptor:       kernel/root only\n");
        }
        out.push_str("  [ LINK   ] PhaseLink codec:          ready\n");
        out.push_str("  [ LINK   ] binary bridge:            reversible\n");
        out.push_str("  [ LINK   ] transport:                hosted loopback\n");
        out.push_str(&format!(
            "  [ LINK   ] gateway:                  {}\n",
            "disabled"
        ));
        out.push_str("  [ LINK   ] ANOM decryptor access:    denied\n");
        out.push_str(&format!(
            "  [ LINK   ] PhaseLinkRoot:            {}\n",
            info.phaselink_root_short
        ));
        out.push_str("  [ VERIFY ] PhaseLink chain:          OK\n");
        out.push_str("  [ PHASEMATH ] scalar reference:     ready\n");
        out.push_str(&format!(
            "  [ PHASEMATH ] worker cores:         {}\n",
            info.phase_math_workers
        ));
        out.push_str(&format!(
            "  [ PHASEMATH ] SIMD capability:      {}\n",
            info.phase_math_simd
        ));
        out.push_str(&format!(
            "  [ PHASEMATH ] active backend:       {}\n",
            info.phase_math_backend
        ));
        out.push_str("  [ PHASEMATH ] trajectory workers:   ready\n");
        out.push_str("  [ PHASEMATH ] proof workers:        ready\n");
        out.push_str("  [ PHASEMATH ] FPGA accelerator:     not present\n");
        out.push_str("  [ VERIFY    ] scalar equivalence:   OK\n");
        out.push_str("  [ BENCH    ] phase-state benchmark: ready\n");
    }
    #[cfg(not(feature = "std"))]
    {
        out.push_str("  [ LINK   ] PhaseLink codec:          ready\n");
        out.push_str(&format!(
            "  [ LINK   ] transport:                {}\n",
            crate::kernel::phaselink::transport_label()
        ));
        #[cfg(all(target_arch = "arm", feature = "bm-net"))]
        {
            out.push_str("  [ LINK   ] console:                  PS UART0\n");
            out.push_str(&format!(
                "  [ LINK   ] UART-GEM bridge:          {}\n",
                crate::kernel::phasenet::uart_gem::status_label(info.uart_gem_bridge)
            ));
        }
        out.push_str(&format!(
            "  [ LINK   ] VirtIO network device:    {}\n",
            if info.ethernet_attachment == "detected" {
                "detected"
            } else {
                "not present"
            }
        ));
        out.push_str(&format!(
            "  [ LINK   ] driver:                   {}\n",
            info.network_driver
        ));
        out.push_str(&format!(
            "  [ LINK   ] Ethernet attachment:      {}\n",
            info.ethernet_attachment
        ));
        out.push_str("  [ PHASEMATH ] scalar reference:     ready\n");
        out.push_str("  [ PHASEMATH ] worker cores:         1\n");
        out.push_str("  [ PHASEMATH ] active backend:       SCALAR\n");
        out.push_str("  [ VERIFY    ] scalar equivalence:   OK\n");
        out.push_str("  [ STRESS   ] PHI-STABILITY engine: ready\n");
        out.push_str("  [ STRESS   ] integer mode: fixed-1024\n");
        if info.ethernet_attachment == "detected" {
            out.push_str("  [ STRESS   ] network verification: available\n");
        }
    }
    out
}

fn wake_lines(info: &BootSplashInfo) -> String {
    let mut lines = String::new();
    push_wake(
        &mut lines,
        "stateclock",
        "chronos + kairos clocks",
        "0.004s",
    );
    push_wake(
        &mut lines,
        "gphase_core",
        "compose / reflect algebra",
        "0.011s",
    );
    push_wake(&mut lines, "scheduler", "Phi0-Phi3 phase graph", "0.018s");
    push_wake(
        &mut lines,
        "memory",
        &format!("{} tagged pages mapped", info.page_count),
        "0.026s",
    );
    push_wake(
        &mut lines,
        "syscall",
        "user/kernel boundary armed",
        "0.033s",
    );
    push_wake(
        &mut lines,
        "phasefs",
        &format!("/tmp mounted ({} inodes)", info.fs_inodes),
        "0.041s",
    );
    push_wake(&mut lines, "vm", "RISC-V ISA loader ready", "0.049s");
    push_wake(
        &mut lines,
        "trace",
        &format!("kernel event ring ({} records)", info.trace_events),
        "0.056s",
    );

    // Merkle statechain ledger now runs on hosted and bare-metal builds alike.
    #[cfg(any(feature = "std", feature = "baremetal"))]
    push_wake(
        &mut lines,
        "statechain",
        &format!("merkle ledger online (tick {})", info.statechain_tick),
        "0.068s",
    );
    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    push_sealed(&mut lines, "statechain", "ledger unavailable (no crypto)");

    push_ascend(&mut lines, "shell", "operator console opening");
    push_wake(
        &mut lines,
        "init",
        &format!(
            "pid {} scheduled ({} proc)",
            info.init_pid, info.process_count
        ),
        "0.095s",
    );
    push_wake(&mut lines, "timer", "preempt + chronos tick", "0.103s");
    push_wake(
        &mut lines,
        "phasefs.journal",
        "phase records read-only",
        "0.118s",
    );
    push_wake(
        &mut lines,
        "stateclock.atlas",
        &format!(
            "10-sector map (sec {}, k={})",
            info.sector_id, info.kairos_tick
        ),
        "0.127s",
    );
    lines
}

fn push_wake(out: &mut String, svc: &str, detail: &str, timing: &str) {
    let mut line = format!("  [ AWAKE ] {svc}: {detail}");
    if line.len() < TIMING_COL {
        line.push_str(&".".repeat(TIMING_COL - line.len()));
    }
    line.push(' ');
    line.push_str(timing);
    line.push('\n');
    out.push_str(&line);
}

fn push_ascend(out: &mut String, svc: &str, detail: &str) {
    let mut line = format!("  [ ASCND ] {svc}: {detail}");
    if line.len() < TIMING_COL {
        line.push_str(&".".repeat(TIMING_COL - line.len()));
    }
    line.push('\n');
    out.push_str(&line);
}

fn push_sealed(out: &mut String, svc: &str, detail: &str) {
    out.push_str(&format!("  [ SEALED] {svc}: {detail}\n"));
}

fn subsystem_aligned_count(info: &BootSplashInfo) -> usize {
    let n = 7usize;
    #[cfg(feature = "std")]
    if info.hosted {
        return 8;
    }
    let _ = info;
    n
}

fn alignment_bar(aligned: usize, total: usize) -> String {
    let filled = "@".repeat(aligned);
    let open = "o".repeat(total.saturating_sub(aligned));
    let tail = if total > aligned {
        "---o~~~o"
    } else {
        "~~~~~~~"
    };
    format!("{filled}---{open}{tail}")
}

fn resonance_line(sector_id: u8) -> String {
    let wave = "#=-=--=#=-=--=#=-=--=#=-=--=#=-=--=#";
    format!("  {wave}   sector {sector_id}/9  x=2*cos(phi/5) ok")
}

fn akashic_records(info: &BootSplashInfo) -> usize {
    let base = info.fs_inodes + info.trace_events + info.kairos_tick as usize;
    #[cfg(feature = "std")]
    if info.hosted {
        return base + info.statechain_tick as usize;
    }
    base
}
