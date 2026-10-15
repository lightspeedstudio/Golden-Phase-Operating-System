//! PZ7020 bare-metal entry — boots the same kernel/shell stack over PS UART0.
//!
//! Mirror of the RISC-V [`crate::boot::kernel_main`]: init, then Runtime +
//! serial shell loop. Entered from `arch/armv7/startup.S` after the FSBL has
//! performed PS7 init and PL configuration — PS7 init is NOT re-run here.

use crate::boot::heap;
use crate::boot::kernel_main::BootError;
use crate::kernel::syscall::IoBuffer;
use crate::runtime::Runtime;
use crate::userland::shell::ShellAction;
use crate::userland::splash::{self, BootSplashInfo};
use crate::userland::syscall;

use super::g1p::{G1p, EXPECTED_ID, EXPECTED_VERSION, G1P_IRQ_ID};
use super::gic;
use super::memory_map::G1P_AXI_BASE;
use super::uart;

/// SCU global timer (PERIPHCLK = APU/2), used as a polled tick source.
const GLOBAL_TIMER_COUNT_L32: u32 = 0xF8F0_0200;
const GLOBAL_TIMER_COUNT_U32: u32 = 0xF8F0_0204;
const GLOBAL_TIMER_CONTROL: u32 = 0xF8F0_0208;

/// One kernel tick every 10 ms of global-timer time.
const TICK_CYCLES: u64 = (super::ps7_init_data::APU_FREQ_HZ as u64) / 2 / 100;

/// `sched2`'s real hardware tick source: the private-timer (PPI 29)
/// auto-reload period, in `PTIMER_LOAD` counts. Reuses the exact same 5 ms
/// period already `PHYSICAL_HARDWARE_TEST`-proven by `irqreg`'s Gate E run
/// (`kernel::net_ip::arm_irq_idle`'s `TICKS_PER_MS.saturating_mul(5)`, same
/// `333_333` PERIPHCLK-ticks-per-ms constant), so `sched2`'s own arm is a
/// period this board has already been shown to sustain, not a new untested
/// magic number. See `fpga/zynq/evidence/SCHED2_STATUS_<date>.md`.
#[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
const SCHED2_TICK_LOAD: u32 = 333_333 * 5;

pub fn kernel_main() -> ! {
    heap::init();
    #[cfg(all(feature = "cpu1", not(feature = "mmu-app-isolation")))]
    super::cpu1::enable_mmu_scu_cpu0();
    #[cfg(all(feature = "cpu1", feature = "mmu-app-isolation"))]
    crate::arch::armv7::isolation::enable();

    let mut runtime = alloc::boxed::Box::new(Runtime::new());
    #[cfg(all(feature = "netsess", any(feature = "std", feature = "baremetal")))]
    runtime.hosted.kernel.bind_netsess_live();
    // Real no-echo UART secret input for `kernel::identity`'s `passwd`
    // command (Gate 4) — see `uart::read_line_silent`'s docs.
    #[cfg(feature = "identity")]
    runtime.hosted.io.set_secret_reader(uart::read_line_silent);
    #[cfg(feature = "identity")]
    runtime.hosted.io.set_output_sink(uart::write_str);
    #[cfg(all(
        feature = "phasefs-journal",
        feature = "baremetal",
        feature = "zynq-a9",
        not(feature = "std")
    ))]
    {
        uart::write_str(&super::journal_sd::boot_recover(&mut runtime.hosted.kernel));
        #[cfg(feature = "identity")]
        runtime.hosted.kernel.identity_import_fat_if_wal_empty();
    }
    match runtime.boot() {
        Ok(init_pid) => {
            let info = BootSplashInfo::from_kernel(&runtime.hosted.kernel, init_pid, false);
            uart::write_str(&splash::render_boot_screen(&info));
        }
        Err(err) => {
            uart::write_str(&alloc::format!("boot failed: {err}\n"));
            halt();
        }
    }

    #[cfg(feature = "cpu1")]
    super::cpu1::wake_cpu1_and_report();

    attach_phase_consensus(&mut runtime.hosted.kernel);
    report_g1p();
    gic::init_g1p_spi();
    uart::write_str("GIC: SPI 61 enabled");
    #[cfg(feature = "usb")]
    uart::write_str(" USB 53 registered");
    uart::write_str("\n");
    gic::enable_cpsr_irqs();
    timer_start();
    #[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
    gic::enable_tick(SCHED2_TICK_LOAD);

    if let Err(err) = run_serial_shell(&mut runtime) {
        uart::write_str(&alloc::format!("runtime failed: {err}\n"));
    }

    uart::write_str("KairOS: halt\n");
    halt();
}

/// Attach live G1P when identity probe passes. Probe failure leaves Emulated.
fn attach_phase_consensus(kernel: &mut crate::kernel::Kernel) {
    let g = G1p::default_window();
    if g.probe_identity().is_ok() {
        kernel.attach_phase_accelerator(alloc::boxed::Box::new(g));
    }
}

/// Read and report the live G1P identity through the FSBL-configured PL.
/// Reports honestly on mismatch — never a stub success.
fn report_g1p() {
    let g = G1p::default_window();
    let id = g.id();
    let version = g.version();
    if id == EXPECTED_ID && version == EXPECTED_VERSION {
        uart::write_str(&alloc::format!(
            "G1P: id GPL4 (0x{id:08X}) version 0x{version:08X} @ 0x{G1P_AXI_BASE:08X}, IRQ {G1P_IRQ_ID} (level-high SPI)\n"
        ));
    } else {
        uart::write_str(&alloc::format!(
            "G1P: UNEXPECTED id=0x{id:08X} version=0x{version:08X} @ 0x{G1P_AXI_BASE:08X} — PL mapping not confirmed\n"
        ));
    }
    #[cfg(feature = "tensor")]
    report_tensor_bank(&g);
}

#[cfg(feature = "tensor")]
fn report_tensor_bank(g: &G1p) {
    use super::g1p::{
        TENSOR_ID, TNSR_ADVANCE_M, TNSR_COMPOSE, TNSR_REFLECT, TNSR_REFRESH_V,
    };
    use crate::core::gphase_core::PhaseState;
    use crate::kernel::tensor_state::{
        commit_lockstep, evaluate, CapabilityClass, SchedulerPhase, TensorOp, TensorState,
    };

    let tid = g.tensor_id();
    let l0 = g.id();
    match g.probe_tensor_bank_result() {
        Ok(()) => uart::write_str(&alloc::format!(
            "TNSR: id TNSR (0x{TENSOR_ID:08X}) L0 still GPL4 (0x{l0:08X}) @ 0x{G1P_AXI_BASE:08X}+0x100\n"
        )),
        Err(_) => {
            uart::write_str(&alloc::format!(
                "TNSR: ABSENT id=0x{tid:08X} L0=0x{l0:08X} — 8-bit alias or Path 1 bit not loaded\n"
            ));
            return;
        }
    }

    let current = TensorState::new(
        PhaseState::Phi0,
        SchedulerPhase::M2,
        CapabilityClass::from_u8(9).expect("v=9"),
    );
    let cases: [(&str, TensorOp, u32, u8); 4] = [
        (
            "compose",
            TensorOp::Compose(PhaseState::Phi1),
            TNSR_COMPOSE,
            PhaseState::Phi1.as_u8(),
        ),
        ("reflect", TensorOp::Reflect, TNSR_REFLECT, 0),
        ("advance-M", TensorOp::AdvanceM, TNSR_ADVANCE_M, 0),
        (
            "refresh-V",
            TensorOp::RefreshV(0x05),
            TNSR_REFRESH_V,
            CapabilityClass::from_rights(0x05).as_u8(),
        ),
    ];
    let mut ok = 0u32;
    for (name, op, cmd, operand) in cases {
        let expect = evaluate(current, op);
        match g.tensor_evaluate(current.as_u8(), cmd, operand) {
            Ok(got) => match commit_lockstep(current, got, op) {
                Ok(_) => {
                    ok += 1;
                    uart::write_str(&alloc::format!(
                        "TNSR: {name} 0x{:02X} -> 0x{:02X} (CPU0 commit ok)\n",
                        current.as_u8(),
                        got
                    ));
                }
                Err(_) => uart::write_str(&alloc::format!(
                    "TNSR: {name} got 0x{got:02X} CPU0 commit rejected (expect 0x{:02X})\n",
                    expect.as_u8()
                )),
            },
            Err(e) => uart::write_str(&alloc::format!("TNSR: {name} mmio fail: {}\n", e.as_str())),
        }
    }
    uart::write_str(&alloc::format!("TNSR: T10 evaluate {ok}/4 CPU0 commit ok\n"));

    match g.compose_ab(0, 1) {
        Ok(1) => uart::write_str("G1P: Layer-0 compose(Phi0,Phi1)=Phi1 (frozen map ok)\n"),
        Ok(v) => uart::write_str(&alloc::format!("G1P: Layer-0 compose unexpected 0x{v:08X}\n")),
        Err(e) => uart::write_str(&alloc::format!("G1P: Layer-0 compose fail: {}\n", e.as_str())),
    }
    // #region agent log
    uart::write_str("G1P_DONE\n");
    // #endregion
}

fn timer_start() {
    unsafe {
        core::ptr::write_volatile(GLOBAL_TIMER_CONTROL as *mut u32, 0);
        core::ptr::write_volatile(GLOBAL_TIMER_COUNT_L32 as *mut u32, 0);
        core::ptr::write_volatile(GLOBAL_TIMER_COUNT_U32 as *mut u32, 0);
        core::ptr::write_volatile(GLOBAL_TIMER_CONTROL as *mut u32, 1);
    }
}

fn timer_now() -> u64 {
    // 64-bit read: upper, lower, upper again until stable.
    loop {
        let hi = unsafe { core::ptr::read_volatile(GLOBAL_TIMER_COUNT_U32 as *const u32) };
        let lo = unsafe { core::ptr::read_volatile(GLOBAL_TIMER_COUNT_L32 as *const u32) };
        let hi2 = unsafe { core::ptr::read_volatile(GLOBAL_TIMER_COUNT_U32 as *const u32) };
        if hi == hi2 {
            return ((hi as u64) << 32) | lo as u64;
        }
    }
}

fn flush_new(io: &IoBuffer, from: usize) {
    // Live sink already emitted each write_line; replaying would duplicate
    // every prompt (and would print password prompts only after passwd
    // returned — the Gate 13 hang).
    if io.writes_live() {
        return;
    }
    for line in io.output.iter().skip(from) {
        uart::write_str(line);
        uart::write_str("\n");
    }
}

/// Advance `kernel.sched2`'s timing wheel from `irqreg`'s real,
/// hardware-driven private-timer (PPI 29) dispatch counter
/// (`gic::tick_count()`) — the exact wiring the build contract's
/// "Phase/kernel integration" requires ("consume private timer IRQ through
/// irqreg"). Firing a timer only transitions `RunState::Blocked(Timer) ->
/// Ready` inside `Sched2` itself (see `kernel::sched2::advance`); there is
/// no separate action to take on `woken` here because this kernel is
/// cooperative, not preemptive (see `kernel::sched2`'s module docs,
/// "Non-goals").
#[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
fn poll_sched2(runtime: &mut Runtime) {
    let now = gic::tick_count() as u64;
    let mut woken = crate::platform::Vec::new();
    runtime.hosted.kernel.sched2.advance(now, &mut woken);
}

/// Read one command line from EITHER the UART console OR an attached USB keyboard
/// (`usb kbd`). Non-blocking on each source; echoes input and handles backspace,
/// so a USB keyboard drives `gpos>` exactly like the serial console.
fn read_line_muxed(runtime: &mut Runtime) -> crate::platform::String {
    let mut buf = crate::platform::String::new();
    loop {
        #[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
        poll_sched2(runtime);
        if let Some(byte) = uart::try_read_byte() {
            match byte {
                b'\r' | b'\n' => {
                    uart::write_byte(b'\r');
                    uart::write_byte(b'\n');
                    return buf;
                }
                0x08 | 0x7f => {
                    if buf.pop().is_some() {
                        uart::write_str("\x08 \x08");
                    }
                }
                b => {
                    uart::write_byte(b);
                    buf.push(b as char);
                }
            }
            continue;
        }
        #[cfg(feature = "usb")]
        {
            let usb_ch = if super::usb_hid::hid_host_active() {
                super::usb_hid::poll_hid_char()
            } else {
                super::usb::poll_key()
            };
            if let Some(ch) = usb_ch {
                match ch {
                    b'\n' | b'\r' => {
                        uart::write_byte(b'\r');
                        uart::write_byte(b'\n');
                        return buf;
                    }
                    0x08 => {
                        if buf.pop().is_some() {
                            uart::write_str("\x08 \x08");
                        }
                    }
                    b => {
                        uart::write_byte(b);
                        buf.push(b as char);
                    }
                }
            }
        }
    }
}

fn run_serial_shell(runtime: &mut Runtime) -> Result<(), BootError> {
    uart::write_str(&runtime.hosted.shell.prompt);

    let mut flushed = 0usize;
    let mut last_tick = timer_now();
    loop {
        let now = timer_now();
        if now.wrapping_sub(last_tick) >= TICK_CYCLES {
            last_tick = now;
            runtime.force_timer_tick().map_err(BootError::Runtime)?;
        }

        let line = read_line_muxed(runtime);
        if line.is_empty() {
            continue;
        }

        let pid = runtime.hosted.init_pid.ok_or(BootError::Boot("no init"))?;
        runtime.hosted.kernel.advance_chronos();
        let state = &mut runtime.hosted.session_state;
        let mut session = crate::userland::shell::ShellSession {
            kernel: &mut runtime.hosted.kernel,
            caller: pid,
            io: &mut runtime.hosted.io,
            loader: &runtime.hosted.programs,
            vms: &mut runtime.hosted.vms,
            state,
        };

        syscall::ensure_running(session.kernel, pid).map_err(BootError::Syscall)?;
        // A single failed command (e.g. `cat` of a path that does not exist)
        // is never kernel-fatal — print it and return to the prompt, instead
        // of propagating it out of this loop and halting the whole kernel.
        let action = match runtime.hosted.shell.execute_session(&mut session, &line) {
            Ok(action) => action,
            Err(err) => {
                session.io.write_line(alloc::format!("error: {err}"));
                ShellAction::Continue
            }
        };
        match action {
            ShellAction::Continue => {}
            ShellAction::RunSshServer { octets, port, secs } => {
                #[cfg(feature = "ssh-bm")]
                {
                    let shell = &mut runtime.hosted.shell;
                    let result = crate::kernel::net_ssh::serve(
                        octets,
                        port,
                        secs,
                        &crate::kernel::net_ssh::AUTHORIZED_PUBKEY,
                        |line| {
                            let before = session.io.output.len();
                            let _ = shell.execute_session(&mut session, line);
                            session.io.output[before..].concat()
                        },
                    );
                    match result {
                        Ok(s) => uart::write_str(&s),
                        Err(e) => uart::write_str(e),
                    }
                    uart::write_str("\n");
                }
                #[cfg(not(feature = "ssh-bm"))]
                {
                    let _ = (octets, port, secs);
                }
                flush_new(&runtime.hosted.io, flushed);
                flushed = runtime.hosted.io.output.len();
            }
            ShellAction::RunAiPhaseLink { secs } => {
                #[cfg(all(target_arch = "arm", feature = "bm-net"))]
                {
                    let text = crate::kernel::ai::inbound_bm::serve(&mut runtime.hosted.kernel, secs);
                    uart::write_str(&text);
                    if !text.ends_with('\n') {
                        uart::write_str("\n");
                    }
                }
                #[cfg(not(all(target_arch = "arm", feature = "bm-net")))]
                {
                    let _ = secs;
                    uart::write_str("ai serve: bm-net required\n");
                }
                flush_new(&runtime.hosted.io, flushed);
                flushed = runtime.hosted.io.output.len();
            }
            ShellAction::Exit => {
                syscall::ensure_running(&mut runtime.hosted.kernel, pid)
                    .map_err(BootError::Syscall)?;
                syscall::exit(&mut runtime.hosted.kernel, pid, &mut runtime.hosted.io)
                    .map_err(BootError::Syscall)?;
                flush_new(&runtime.hosted.io, flushed);
                return Ok(());
            }
        }
        flush_new(&runtime.hosted.io, flushed);
        flushed = runtime.hosted.io.output.len();
        emit_prompt(runtime, pid, &mut flushed)?;
    }
}

fn emit_prompt(runtime: &mut Runtime, pid: u32, flushed: &mut usize) -> Result<(), BootError> {
    syscall::ensure_running(&mut runtime.hosted.kernel, pid).map_err(BootError::Syscall)?;
    syscall::write(
        &mut runtime.hosted.kernel,
        pid,
        &mut runtime.hosted.io,
        &runtime.hosted.shell.prompt,
    )
    .map_err(BootError::Syscall)?;
    flush_new(&runtime.hosted.io, *flushed);
    *flushed = runtime.hosted.io.output.len();
    Ok(())
}

fn halt() -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}
