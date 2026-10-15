//! Command dispatch table and execution router.

use crate::kernel::syscall::SyscallResult;
use crate::kernel::trace::TraceRecord;
use crate::platform::String;
use crate::stateclock::{self, KairosEventKind};
#[cfg(feature = "std")]
use crate::userland::ai_cmd;
use crate::userland::commands::g64_cmd;
#[cfg(feature = "phaseguard-research")]
use crate::userland::commands::phaseguard_cmd;
use crate::userland::commands::{
    about, anom_cmd, clear, clock_cmd, dump, g1p_cmd, help, host_cmd, isa, login_cmd, mod_cmd,
    passwd_cmd, regs,
    run_paste, selftest_cmd, spawn_cmd, state_cmd, trace_cmd, version, whoami_cmd,
};
use crate::userland::context::ShellContext;
use crate::userland::craft_cmd;
use crate::userland::curvature_cmd;
use crate::userland::fs_cmd;
use crate::userland::ipc_cmd;
use crate::userland::kill;
use crate::userland::log;
use crate::userland::mem;
use crate::userland::mmuiso_cmd;
use crate::userland::net_cmd;
use crate::userland::parser::Command;
use crate::userland::phasestack_cmd;
#[cfg(feature = "std")]
use crate::userland::phase_benchmark_cmd;
#[cfg(all(feature = "baremetal", not(feature = "std"), feature = "bm-net"))]
use crate::userland::phaselink_baremetal_cmd;
#[cfg(feature = "std")]
use crate::userland::phaselink_cmd;
use crate::userland::phasenet_cmd;
use crate::userland::phaseperf_cmd;
use crate::userland::phi_stability_cmd;
use crate::userland::ps;
use crate::userland::run;
use crate::userland::sentinel_cmd;
use crate::userland::shell::{ShellAction, ShellError};
#[cfg(feature = "ssh")]
use crate::userland::ssh_cmd;
use crate::userland::state_domain_cmd;
use crate::userland::syscall;
use crate::userland::terminal_cmd;
use crate::userland::top::TopOptions;
use crate::userland::transport_cmd;

pub struct CommandInfo {
    pub name: &'static str,
    pub description: &'static str,
}

pub const COMMAND_CATALOG: &[CommandInfo] = &[
    CommandInfo {
        name: "help",
        description: "list commands with one-line descriptions",
    },
    CommandInfo {
        name: "about",
        description: "print The Zen of KairOS",
    },
    CommandInfo {
        name: "clear",
        description: "clear the terminal screen",
    },
    CommandInfo {
        name: "version",
        description: "print KairOS version and build target",
    },
    CommandInfo {
        name: "state",
        description: "StateClock summary and StateDomain VM commands",
    },
    CommandInfo {
        name: "clock",
        description: "StateClock view and operator controls",
    },
    CommandInfo {
        name: "phasenet",
        description: "PhaseNet native state network (loopback, sim, stream)",
    },
    CommandInfo {
        name: "phaselink",
        description: "PhaseLink over GEM0 EtherType 0x88B5 (init/send/recv/replay); hosted sim/gateway stay std",
    },
    CommandInfo {
        name: "phaseperf",
        description: "Phase Math backend, queues, benchmark, verification",
    },
    CommandInfo {
        name: "phi-stability",
        description: "PHI-STABILITY full-stack stress test (Fibonacci, clocks, network)",
    },
    CommandInfo {
        name: "phase-benchmark",
        description: "phase-math bench: integer/scalar on BM (phi-stability); BigUint hosted-only",
    },
    CommandInfo {
        name: "net",
        description: "GEM0 ladder + net wire (raw 0x88B5) on PZ7020",
    },
    CommandInfo {
        name: "regs",
        description: "VM register file if a program has run",
    },
    CommandInfo {
        name: "ps",
        description: "list processes with pid, name, phase, ticks, status",
    },
    CommandInfo {
        name: "whoami",
        description: "authority mode (root / anom) and visibility",
    },
    CommandInfo {
        name: "host",
        description: "machine identity, devices, and NIC summary",
    },
    CommandInfo {
        name: "spawn",
        description: "create a named test process",
    },
    CommandInfo {
        name: "kill",
        description: "stop a process by pid",
    },
    CommandInfo {
        name: "trace",
        description: "recent kernel trace events",
    },
    CommandInfo {
        name: "isa",
        description: "instruction set reference table",
    },
    CommandInfo {
        name: "run",
        description: "paste program (.end) or run named program",
    },
    CommandInfo {
        name: "dump",
        description: "combined state, ps, regs, and trace",
    },
    CommandInfo {
        name: "mem",
        description: "memory page phase tags",
    },
    CommandInfo {
        name: "log",
        description: "topology reflection log",
    },
    CommandInfo {
        name: "exit",
        description: "exit the shell",
    },
    CommandInfo {
        name: "dashboard",
        description: "operator dashboard",
    },
    CommandInfo {
        name: "top",
        description: "live phase monitor",
    },
    CommandInfo {
        name: "watchdog",
        description: "statechain watchdog view",
    },
    CommandInfo {
        name: "phase",
        description: "subsystem phase counts",
    },
    CommandInfo {
        name: "topo",
        description: "topology events",
    },
    CommandInfo {
        name: "curvature",
        description: "causal-curvature watchdog: defect rate + rolling z-score",
    },
    CommandInfo {
        name: "root",
        description: "state roots",
    },
    CommandInfo {
        name: "audit",
        description: "statechain audit",
    },
    CommandInfo {
        name: "checkpoint",
        description: "seal HMAC integrity MAC (not a signature)",
    },
    CommandInfo {
        name: "checkpoint-auth",
        description: "secret-backed checkpoint HMAC (not Ed25519)",
    },
    CommandInfo {
        name: "phasecache",
        description: "PhaseCache stats: hits/misses/trust-rejections/size",
    },
    CommandInfo {
        name: "pmm",
        description: "PMM stats: frames/allocations/frees/quarantines",
    },
    CommandInfo {
        name: "pmm selftest",
        description: "PMM real alloc/sentinel-write/verify/free/realloc round trip",
    },
    CommandInfo {
        name: "irqreg",
        description: "IRQ registry stats: per-IRQ counts/storm-mask, ring drops/rejections",
    },
    CommandInfo {
        name: "sched2",
        description: "sched2 stats: run-state counts, vruntime/deadline/wheel counters",
    },
    CommandInfo {
        name: "sleep",
        description: "sleep <ticks>: block the caller in sched2 for N ticks (PROC_SLEEP)",
    },
    CommandInfo {
        name: "ai",
        description: "AI control — PhaseLink inbound (ai serve / jobs / tools); outbound ask is hosted",
    },
    CommandInfo {
        name: "ssh",
        description: "SSH remote operator shell (hosted)",
    },
    CommandInfo {
        name: "anom",
        description: "ANOM security mode (Audited Non-Observable Mode)",
    },
    CommandInfo {
        name: "g1p",
        description: "G1P PL accelerator (info/exec/reflect/selftest/consensus; polled)",
    },
    CommandInfo {
        name: "g64",
        description: "G64 phase-fabric sidecar research (oracle-compared; opt-in)",
    },
    #[cfg(feature = "phaseguard-research")]
    CommandInfo {
        name: "phaseguard",
        description: "PhaseGuard Gate-4 observer (stateclock-bound; read-only)",
    },
    CommandInfo {
        name: "ls",
        description: "list PhaseFS directory",
    },
    CommandInfo {
        name: "cat",
        description: "read PhaseFS file bytes",
    },
    CommandInfo {
        name: "write",
        description: "write text to a PhaseFS path",
    },
    CommandInfo {
        name: "mkdir",
        description: "create PhaseFS directory",
    },
    CommandInfo {
        name: "rename",
        description: "rename a PhaseFS path",
    },
    CommandInfo {
        name: "stat",
        description: "PhaseFS path phase tag",
    },
    CommandInfo {
        name: "open",
        description: "mint a capability for a PhaseFS path (fs_open)",
    },
    CommandInfo {
        name: "close",
        description: "revoke a capability handle (fs_close)",
    },
    CommandInfo {
        name: "captable",
        description: "capability table stats: objects/capabilities/authorize outcomes",
    },
    CommandInfo {
        name: "passwd",
        description: "set/change the Root password (identity)",
    },
    CommandInfo {
        name: "passwd --status",
        description: "Root credential state and password policy (identity)",
    },
    CommandInfo {
        name: "login",
        description: "authenticate Root and open a boot-bound session",
    },
    CommandInfo {
        name: "logout",
        description: "close the current identity session",
    },
    CommandInfo {
        name: "mod",
        description: "structured operator elevation (mod <op>, -l, -v, -k, --lease; no mod shell)",
    },
    #[cfg(feature = "phasestack")]
    CommandInfo {
        name: "app",
        description: "verified PhaseStack apps (verify|install|list|info|run|stop)",
    },
    #[cfg(feature = "phasestack")]
    CommandInfo {
        name: "plugin",
        description: "verified PhaseStack plugins (own GPA, own VM, cap intersection)",
    },
    #[cfg(feature = "phasesentinel")]
    CommandInfo {
        name: "sentinel",
        description: "PhaseSentinel status/evidence/cert/quarantine (Root/MOD)",
    },
];

pub fn execute(ctx: &mut ShellContext<'_>, cmd: Command) -> Result<ShellAction, ShellError> {
    match &cmd {
        Command::Unknown(s) if s.is_empty() => return Ok(ShellAction::Continue),
        Command::Usage(msg) => {
            ctx.writeln_line(msg)?;
            return Ok(ShellAction::Continue);
        }
        Command::Unknown(name) => {
            ctx.writeln_line(&format!("unknown command: {name}"))?;
            return Ok(ShellAction::Continue);
        }
        _ => {}
    }

    if let Command::Unknown(_) = cmd {
        return Ok(ShellAction::Continue);
    }

    if let Command::Exit = cmd {
        return Ok(ShellAction::Exit);
    }

    let line = command_trace_line(&cmd);
    let skip_kairos = matches!(cmd, Command::ClockResync | Command::ClockSetDilation { .. });
    ctx.kernel
        .trace
        .record(TraceRecord::Command { line: line.clone() });

    match cmd {
        Command::Help => help::run(ctx)?,
        Command::About => about::run(ctx)?,
        Command::Clear => clear::run(ctx)?,
        Command::Version => version::run(ctx)?,
        Command::State => state_cmd::run(ctx)?,
        Command::Clock => clock_cmd::run(ctx)?,
        Command::ClockSkew => clock_cmd::run_skew(ctx)?,
        Command::ClockSetDilation { value } => clock_cmd::run_set_dilation(ctx, value)?,
        Command::ClockResync => clock_cmd::run_resync(ctx)?,
        Command::Regs => regs::run(ctx)?,
        Command::Ps => {
            ps::run(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Whoami => whoami_cmd::run(ctx)?,
        Command::Host => host_cmd::run(ctx)?,
        Command::Spawn { name } => spawn_cmd::run(ctx, &name)?,
        Command::Kill { pid } => {
            kill::run(ctx.kernel, ctx.caller, ctx.io, pid)?;
        }
        Command::Trace => trace_cmd::run(ctx)?,
        Command::Isa => isa::run(ctx)?,
        Command::Dump => dump::run(ctx)?,
        Command::RunPaste => run_paste::run(ctx)?,
        Command::RunFile { filename } => run_paste::run_file(ctx, &filename)?,
        Command::RunProgram { name } => {
            if let Err(err) =
                run::run_program(ctx.kernel, ctx.caller, ctx.io, ctx.loader, &name, ctx.vms)
            {
                syscall::ensure_running(ctx.kernel, ctx.caller)?;
                ctx.kernel.trace.record(TraceRecord::Error {
                    message: format!("{err}"),
                });
                syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{err}\n"))?;
            }
        }
        Command::Mem => {
            mem::run(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Log => {
            log::run(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Ls { path } => {
            fs_cmd::ls(ctx.kernel, ctx.caller, ctx.io, &path)?;
        }
        Command::Cat { path } => {
            fs_cmd::cat(ctx.kernel, ctx.caller, ctx.io, &path)?;
        }
        Command::Write { path, text } => {
            match fs_cmd::write_path(ctx.kernel, ctx.caller, ctx.io, &path, &text) {
                Ok(()) => {
                    ctx.writeln_line(&format!("wrote {} bytes to {path}", text.len()))?;
                }
                Err(err) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        &fs_cmd::format_fs_error(&err),
                    )?;
                }
            }
        }
        Command::Mkdir { path } => {
            fs_cmd::mkdir(ctx.kernel, ctx.caller, ctx.io, &path)?;
        }
        Command::Rename { from, to } => {
            match fs_cmd::rename(ctx.kernel, ctx.caller, ctx.io, &from, &to) {
                Ok(()) => {
                    ctx.writeln_line(&format!("renamed {from} -> {to}"))?;
                }
                Err(err) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        &fs_cmd::format_fs_error(&err),
                    )?;
                }
            }
        }
        Command::Rm { path } => {
            if let Err(err) = fs_cmd::rm(ctx.kernel, ctx.caller, ctx.io, &path) {
                syscall::ensure_running(ctx.kernel, ctx.caller)?;
                syscall::write(
                    ctx.kernel,
                    ctx.caller,
                    ctx.io,
                    &fs_cmd::format_fs_error(&err),
                )?;
            }
        }
        Command::Stat { path } => {
            fs_cmd::stat(ctx.kernel, ctx.caller, ctx.io, &path)?;
        }
        Command::Lock { path } => {
            fs_cmd::lock(ctx.kernel, ctx.caller, ctx.io, &path)?;
        }
        Command::Unlock { path } => {
            fs_cmd::unlock(ctx.kernel, ctx.caller, ctx.io, &path)?;
        }
        Command::Open { path, mode } => {
            match fs_cmd::open(ctx.kernel, ctx.caller, ctx.io, &path, &mode) {
                Ok(SyscallResult::Handle(slot, generation)) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        &format!("opened {path} mode={mode}: handle {slot}:{generation}\n"),
                    )?;
                }
                Ok(_) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        "captable feature not enabled; fs_open is a no-op stub\n",
                    )?;
                }
                Err(err) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        &fs_cmd::format_fs_error(&err),
                    )?;
                }
            }
        }
        Command::Close { slot, generation } => {
            match fs_cmd::close(ctx.kernel, ctx.caller, ctx.io, slot, generation) {
                Ok(()) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        &format!("closed handle {slot}:{generation}\n"),
                    )?;
                }
                Err(err) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        &fs_cmd::format_fs_error(&err),
                    )?;
                }
            }
        }
        Command::Captable => {
            terminal_cmd::run_captable(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Passwd => passwd_cmd::run(ctx)?,
        Command::PasswdStatus => passwd_cmd::run_status(ctx)?,
        Command::Login => login_cmd::run_login(ctx)?,
        Command::Logout => login_cmd::run_logout(ctx)?,
        Command::ModList => mod_cmd::run_list(ctx)?,
        Command::Mod { verb, lease } => mod_cmd::run(ctx, &verb, lease)?,
        Command::ModKill { ticket_id } => mod_cmd::run_kill(ctx, ticket_id)?,
        Command::App { args } => phasestack_cmd::run_app(ctx, &args)?,
        Command::Plugin { args } => phasestack_cmd::run_plugin(ctx, &args)?,
        Command::ModApp { args } => phasestack_cmd::run_mod_app(ctx, &args)?,
        Command::ModCertDeveloperRevoke { hash_hex } => {
            phasestack_cmd::run_mod_cert_revoke(ctx, &hash_hex)?
        },
        Command::Sentinel { args } => sentinel_cmd::run(ctx, &args)?,
        Command::MmuIso { args } => mmuiso_cmd::run(ctx, &args)?,
        Command::Dashboard => {
            terminal_cmd::run_dashboard(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Top { frames } => {
            terminal_cmd::run_top(
                ctx.kernel,
                ctx.caller,
                ctx.io,
                ctx.vms,
                TopOptions { frames },
            )?;
        }
        Command::Watchdog => {
            terminal_cmd::run_watchdog(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Phase => {
            terminal_cmd::run_phase(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Topo => {
            terminal_cmd::run_topo(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Curvature => curvature_cmd::run(ctx)?,
        Command::CurvatureInject { reps } => curvature_cmd::run_inject(ctx, reps)?,
        Command::TransportInit { size } => transport_cmd::run_init(ctx, size)?,
        Command::TransportStatus => transport_cmd::run_status(ctx)?,
        Command::TransportSend {
            sr,
            sc,
            dr,
            dc,
            opcode,
            payload,
        } => transport_cmd::run_send(ctx, sr, sc, dr, dc, &opcode, &payload)?,
        Command::TransportTick { n } => transport_cmd::run_tick(ctx, n)?,
        Command::TransportReceive { lane } => transport_cmd::run_receive(ctx, lane)?,
        Command::TransportRoutes => transport_cmd::run_routes(ctx)?,
        Command::TransportTrace { packet_id } => transport_cmd::run_trace(ctx, packet_id)?,
        Command::TransportReplay { packet_id } => transport_cmd::run_replay(ctx, packet_id)?,
        Command::TransportInjectFault { packet_id, fault } => {
            transport_cmd::run_inject_fault(ctx, packet_id, &fault)?
        }
        Command::TransportReset => transport_cmd::run_reset(ctx)?,
        Command::TransportDemo => transport_cmd::run_demo(ctx)?,
        Command::CraftInit => craft_cmd::run_init(ctx)?,
        Command::CraftStatus => craft_cmd::run_status(ctx)?,
        Command::CraftTick { n } => craft_cmd::run_tick(ctx, n)?,
        Command::CraftCommand {
            subsystem,
            operation,
        } => craft_cmd::run_command(ctx, &subsystem, &operation)?,
        Command::CraftFaultInject { reason } => craft_cmd::run_fault_inject(ctx, &reason)?,
        Command::CraftQuarantineStatus => craft_cmd::run_quarantine_status(ctx)?,
        Command::CraftEpisode { episode_id } => craft_cmd::run_episode(ctx, episode_id)?,
        Command::CraftReplay { episode_id } => craft_cmd::run_replay(ctx, episode_id)?,
        Command::ScenarioRun { name } => {
            if name == "prop-stuck" {
                craft_cmd::run_scenario_prop_stuck(ctx)?;
            } else if name == "prop-stuck-glider" || name == "craft-prop-stuck-glider" {
                craft_cmd::run_scenario_prop_stuck_glider(ctx)?;
            } else {
                ctx.writeln_line(&format!("unknown scenario: {name}"))?;
            }
        }
        Command::Root => {
            terminal_cmd::run_root(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Audit => {
            terminal_cmd::run_audit(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Checkpoint => {
            terminal_cmd::run_checkpoint(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::CheckpointAuthStatus => {
            crate::userland::checkpoint_auth_cmd::run_status(ctx)?;
        }
        Command::CheckpointAuthProvision => {
            crate::userland::checkpoint_auth_cmd::run_provision(ctx)?;
        }
        Command::CheckpointAuthSeal => {
            crate::userland::checkpoint_auth_cmd::run_seal(ctx)?;
        }
        Command::CheckpointAuthVerify => {
            crate::userland::checkpoint_auth_cmd::run_verify(ctx)?;
        }
        Command::CheckpointAuthHardware => {
            crate::userland::checkpoint_auth_cmd::run_hardware(ctx)?;
        }
        Command::PhaseCache => {
            terminal_cmd::run_phase_cache(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Pmm => {
            terminal_cmd::run_pmm(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Irqreg => {
            terminal_cmd::run_irqreg(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Sched2 => {
            terminal_cmd::run_sched2(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Extmap => {
            terminal_cmd::run_extmap(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Txlog => {
            terminal_cmd::run_txlog(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::TxlogDemo => {
            terminal_cmd::run_txlog_demo(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::TxlogFault => {
            terminal_cmd::run_txlog_fault(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::TxlogRecover => {
            terminal_cmd::run_txlog_recover(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Fsj => {
            let text = crate::userland::fsj_cmd::status(ctx.kernel);
            syscall::ensure_running(ctx.kernel, ctx.caller)?;
            syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{text}\n"))?;
        }
        Command::FsjRecover => {
            let text = crate::userland::fsj_cmd::recover(ctx.kernel);
            syscall::ensure_running(ctx.kernel, ctx.caller)?;
            syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{text}\n"))?;
        }
        Command::FsjCheck => {
            let text = crate::userland::fsj_cmd::check(ctx.kernel);
            syscall::ensure_running(ctx.kernel, ctx.caller)?;
            syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{text}\n"))?;
        }
        Command::FsjFormat { confirm } => {
            let text = crate::userland::fsj_cmd::format_cmd(ctx.kernel, confirm);
            syscall::ensure_running(ctx.kernel, ctx.caller)?;
            syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{text}\n"))?;
        }
        Command::FsjCrash { n } => {
            let text = crate::userland::fsj_cmd::crash(ctx.kernel, n);
            syscall::ensure_running(ctx.kernel, ctx.caller)?;
            syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{text}\n"))?;
        }
        Command::FsjP6 { confirm } => {
            let text = crate::userland::fsj_cmd::p6_arm(ctx.kernel, confirm);
            syscall::ensure_running(ctx.kernel, ctx.caller)?;
            syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{text}\n"))?;
        }
        Command::FsjP7 { run_id, token_ok } => {
            let text = crate::userland::fsj_cmd::p7_run(ctx.kernel, run_id, token_ok);
            syscall::ensure_running(ctx.kernel, ctx.caller)?;
            syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{text}\n"))?;
        }
        Command::FsjP8 { run_id, token_ok } => {
            let text = crate::userland::fsj_cmd::p8_run(ctx.kernel, run_id, token_ok);
            syscall::ensure_running(ctx.kernel, ctx.caller)?;
            syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{text}\n"))?;
        }
        Command::FsjP10 { run_id, token_ok } => {
            let text = crate::userland::fsj_cmd::p10_run(ctx.kernel, run_id, token_ok);
            syscall::ensure_running(ctx.kernel, ctx.caller)?;
            syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{text}\n"))?;
        }
        Command::FsjP11 { run_id, token_ok } => {
            let text = crate::userland::fsj_cmd::p11_run(ctx.kernel, run_id, token_ok);
            syscall::ensure_running(ctx.kernel, ctx.caller)?;
            syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{text}\n"))?;
        }
        Command::FsjP1Reset => {
            let text = crate::userland::fsj_cmd::p1_reset(ctx.kernel);
            syscall::ensure_running(ctx.kernel, ctx.caller)?;
            syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{text}\n"))?;
        }
        Command::Netsess => {
            terminal_cmd::run_netsess(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::NetsessDemo => {
            terminal_cmd::run_netsess_demo(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::NetsessFault => {
            terminal_cmd::run_netsess_fault(ctx.kernel, ctx.caller, ctx.io)?;
        }
        Command::Sleep { ticks } => {
            terminal_cmd::run_sleep(ctx.kernel, ctx.caller, ctx.io, ticks)?;
        }
        Command::Ipc => {
            let text = ipc_cmd::render(ctx.kernel);
            syscall::ensure_running(ctx.kernel, ctx.caller)?;
            syscall::write(ctx.kernel, ctx.caller, ctx.io, &format!("{text}\n"))?;
        }
        Command::IpcCreate { capacity } => {
            match ipc_cmd::create(ctx.kernel, ctx.caller, ctx.io, capacity) {
                Ok(SyscallResult::Handle(slot, generation)) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        &format!("created queue capacity={capacity}: handle {slot}:{generation}\n"),
                    )?;
                }
                Ok(_) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        "ipc feature not enabled; ipc_create_queue is a no-op\n",
                    )?;
                }
                Err(err) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        &ipc_cmd::format_ipc_error(&err),
                    )?;
                }
            }
        }
        Command::IpcSend {
            slot,
            generation,
            text,
        } => match ipc_cmd::send(ctx.kernel, ctx.caller, ctx.io, slot, generation, &text) {
            Ok(()) => {
                syscall::ensure_running(ctx.kernel, ctx.caller)?;
                syscall::write(
                    ctx.kernel,
                    ctx.caller,
                    ctx.io,
                    &format!("sent {} byte(s) on {slot}:{generation}\n", text.len()),
                )?;
            }
            Err(err) => {
                syscall::ensure_running(ctx.kernel, ctx.caller)?;
                syscall::write(
                    ctx.kernel,
                    ctx.caller,
                    ctx.io,
                    &ipc_cmd::format_ipc_error(&err),
                )?;
            }
        },
        Command::IpcRecv { slot, generation } => {
            match ipc_cmd::recv(ctx.kernel, ctx.caller, ctx.io, slot, generation) {
                Ok((sender, data)) => {
                    let text = core::str::from_utf8(&data).unwrap_or("<binary>");
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        &format!("from pid {sender}: {text}\n"),
                    )?;
                }
                Err(err) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        &ipc_cmd::format_ipc_error(&err),
                    )?;
                }
            }
        }
        Command::IpcClose { slot, generation } => {
            match ipc_cmd::destroy(ctx.kernel, ctx.caller, ctx.io, slot, generation) {
                Ok(()) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        &format!("closed queue {slot}:{generation}\n"),
                    )?;
                }
                Err(err) => {
                    syscall::ensure_running(ctx.kernel, ctx.caller)?;
                    syscall::write(
                        ctx.kernel,
                        ctx.caller,
                        ctx.io,
                        &ipc_cmd::format_ipc_error(&err),
                    )?;
                }
            }
        }
        Command::PmmSelftest => {
            terminal_cmd::run_pmm_selftest(ctx.kernel, ctx.caller, ctx.io)?;
        }
        #[cfg(feature = "std")]
        Command::AiConnect { host, port } => ai_cmd::run_connect(ctx, &host, port)?,
        #[cfg(feature = "std")]
        Command::AiDisconnect => ai_cmd::run_disconnect(ctx)?,
        #[cfg(feature = "std")]
        Command::AiStatus => ai_cmd::run_status(ctx)?,
        #[cfg(feature = "std")]
        Command::AiAsk { prompt } => ai_cmd::run_ask(ctx, &prompt)?,
        #[cfg(feature = "std")]
        Command::AiJobs => ai_cmd::run_jobs(ctx)?,
        #[cfg(feature = "std")]
        Command::AiShow { job_id } => ai_cmd::run_show(ctx, job_id)?,
        #[cfg(feature = "std")]
        Command::AiTrace { job_id } => ai_cmd::run_trace(ctx, job_id)?,
        #[cfg(feature = "std")]
        Command::AiCancel { job_id } => ai_cmd::run_cancel(ctx, job_id)?,
        #[cfg(feature = "std")]
        Command::AiTools => ai_cmd::run_tools(ctx)?,
        #[cfg(feature = "std")]
        Command::AiAllow { tool } => ai_cmd::run_allow(ctx, &tool)?,
        #[cfg(feature = "std")]
        Command::AiDeny { tool } => ai_cmd::run_deny(ctx, &tool)?,
        #[cfg(feature = "std")]
        Command::AiServe { .. } => {
            ctx.writeln_line("ai serve: PhaseLink inbound is PZ7020-only (EtherType 0x88B5)")?;
        }
        #[cfg(feature = "ssh")]
        Command::SshStatus => ssh_cmd::run_status(ctx)?,
        #[cfg(feature = "ssh")]
        Command::SshStart => ssh_cmd::run_start(ctx)?,
        #[cfg(feature = "ssh")]
        Command::SshStop => ssh_cmd::run_stop(ctx)?,
        #[cfg(feature = "ssh")]
        Command::SshRestart => ssh_cmd::run_restart(ctx)?,
        #[cfg(feature = "ssh")]
        Command::SshSessions => ssh_cmd::run_sessions(ctx)?,
        #[cfg(feature = "ssh")]
        Command::SshSessionShow { id } => ssh_cmd::run_session_show(ctx, id)?,
        #[cfg(feature = "ssh")]
        Command::SshSessionClose { id } => ssh_cmd::run_session_close(ctx, id)?,
        #[cfg(feature = "ssh")]
        Command::SshConfig => ssh_cmd::run_config(ctx)?,
        #[cfg(feature = "ssh")]
        Command::SshKeyList => ssh_cmd::run_key_list(ctx)?,
        #[cfg(feature = "ssh")]
        Command::SshKeyAdd { public_key } => ssh_cmd::run_key_add(ctx, &public_key)?,
        #[cfg(feature = "ssh")]
        Command::SshKeyRemove { fingerprint } => ssh_cmd::run_key_remove(ctx, &fingerprint)?,
        #[cfg(feature = "ssh")]
        Command::SshKeyEnable { fingerprint } => ssh_cmd::run_key_enable(ctx, &fingerprint)?,
        #[cfg(feature = "ssh")]
        Command::SshKeyDisable { fingerprint } => ssh_cmd::run_key_disable(ctx, &fingerprint)?,
        #[cfg(feature = "ssh")]
        Command::SshHostKeyFingerprint => ssh_cmd::run_host_key_fingerprint(ctx)?,
        #[cfg(feature = "ssh")]
        Command::SshHostKeyStatus => ssh_cmd::run_host_key_status(ctx)?,
        #[cfg(feature = "ssh")]
        Command::SshHostKeyRegenerate => ssh_cmd::run_host_key_regenerate(ctx)?,
        #[cfg(feature = "ssh")]
        Command::SshAudit => ssh_cmd::run_audit(ctx)?,
        #[cfg(not(feature = "std"))]
        Command::AiConnect { .. } | Command::AiDisconnect | Command::AiAsk { .. } => {
            ctx.writeln_line(
                "ai: outbound connect/ask is hosted; inbound is PhaseLink `ai serve` (0x88B5)",
            )?;
        }
        #[cfg(not(feature = "std"))]
        Command::AiStatus => {
            ctx.writeln(&crate::kernel::ai::control::status(ctx.kernel))?;
        }
        #[cfg(not(feature = "std"))]
        Command::AiJobs => {
            ctx.writeln(&crate::kernel::ai::control::jobs_table(ctx.kernel))?;
        }
        #[cfg(not(feature = "std"))]
        Command::AiShow { job_id } => {
            match crate::kernel::ai::control::show_job(ctx.kernel, job_id) {
                Ok(t) => ctx.writeln(&t)?,
                Err(e) => ctx.writeln_line(&e.reason())?,
            }
        }
        #[cfg(not(feature = "std"))]
        Command::AiTrace { job_id } => {
            match crate::kernel::ai::control::trace_job(ctx.kernel, job_id) {
                Ok(t) => ctx.writeln(&t)?,
                Err(e) => ctx.writeln_line(&e.reason())?,
            }
        }
        #[cfg(not(feature = "std"))]
        Command::AiCancel { job_id } => {
            match crate::kernel::ai::control::cancel_job(ctx.kernel, job_id) {
                Ok(t) => ctx.writeln(&t)?,
                Err(e) => ctx.writeln_line(&e.reason())?,
            }
        }
        #[cfg(not(feature = "std"))]
        Command::AiTools => {
            ctx.writeln(&crate::kernel::ai::control::tools_table(ctx.kernel))?;
        }
        #[cfg(not(feature = "std"))]
        Command::AiAllow { tool } => {
            match crate::kernel::ai::control::allow(ctx.kernel, &tool) {
                Ok(t) => ctx.writeln_line(&t)?,
                Err(e) => ctx.writeln_line(&e.reason())?,
            }
        }
        #[cfg(not(feature = "std"))]
        Command::AiDeny { tool } => {
            match crate::kernel::ai::control::deny(ctx.kernel, &tool) {
                Ok(t) => ctx.writeln_line(&t)?,
                Err(e) => ctx.writeln_line(&e.reason())?,
            }
        }
        #[cfg(not(feature = "std"))]
        Command::AiServe { secs } => {
            return Ok(crate::userland::shell::ShellAction::RunAiPhaseLink { secs });
        }
        #[cfg(all(target_arch = "arm", feature = "ssh-bm"))]
        Command::AiAskBm { octets, port, prompt } => {
            use crate::kernel::ai::protocol::AiRequestMessage;
            let req = AiRequestMessage {
                request_id: 1,
                process_id: 0,
                model: "default".into(),
                input: prompt.clone(),
                input_hash: String::new(),
                allowed_tools: crate::platform::Vec::new(),
                deadline: 15000,
                current_phase: "Phi1".into(),
                parent_chain_hash: String::new(),
            };
            // Board takes host .50 on the server's /24 (mirrors the SSH-bm default).
            let local = [octets[0], octets[1], octets[2], 50];
            ctx.writeln_line(&format!(
                "ai remote: {}.{}.{}.{} -> {}.{}.{}.{}:{} ...",
                local[0], local[1], local[2], local[3],
                octets[0], octets[1], octets[2], octets[3], port
            ))?;
            match crate::kernel::ai::client_bm::ask(local, octets, port, &req, 15) {
                Ok(resp) => ctx.writeln_line(&format!("ai [{}]: {}", resp.status, resp.output))?,
                Err(e) => ctx.writeln_line(e)?,
            }
        }
        #[cfg(not(all(target_arch = "arm", feature = "ssh-bm")))]
        Command::AiAskBm { .. } => {
            ctx.writeln_line("ai remote: requires an arm ssh-bm baremetal build")?;
        }
        #[cfg(all(
            not(feature = "ssh"),
            all(target_arch = "arm", feature = "ssh-bm")
        ))]
        Command::SshStatus => {
            ctx.writeln_line("ssh-bm (sunset) over GEM0")?;
            ctx.writeln_line("auth: public-key ed25519 (compiled-in authorized key)")?;
            ctx.writeln_line("not russh/tokio")?;
            ctx.writeln_line(
                "idle: shell prompt means no serve running (ssh-bm serve is blocking)",
            )?;
            ctx.writeln_line("start: ssh start  |  net ssh serve [port] [seconds]")?;
        }
        #[cfg(all(
            not(feature = "ssh"),
            all(target_arch = "arm", feature = "ssh-bm")
        ))]
        Command::SshStart => {
            ctx.writeln_line("ssh-bm: starting sunset SSH on 192.168.5.50:2222 for 60s ...")?;
            return Ok(crate::userland::shell::ShellAction::RunSshServer {
                octets: [192, 168, 5, 50],
                port: 2222,
                secs: 60,
            });
        }
        #[cfg(all(
            not(feature = "ssh"),
            all(target_arch = "arm", feature = "ssh-bm")
        ))]
        Command::SshStop => {
            ctx.writeln_line(
                "ssh-bm: no background daemon — serve is a blocking poll loop; stop = wait for timeout or reset",
            )?;
        }
        #[cfg(all(
            not(feature = "ssh"),
            all(target_arch = "arm", feature = "ssh-bm")
        ))]
        Command::SshRestart
        | Command::SshSessions
        | Command::SshSessionShow { .. }
        | Command::SshSessionClose { .. }
        | Command::SshConfig
        | Command::SshKeyList
        | Command::SshKeyAdd { .. }
        | Command::SshKeyRemove { .. }
        | Command::SshKeyEnable { .. }
        | Command::SshKeyDisable { .. }
        | Command::SshHostKeyFingerprint
        | Command::SshHostKeyStatus
        | Command::SshHostKeyRegenerate
        | Command::SshAudit => {
            ctx.writeln_line(
                "ssh-bm: russh management verbs are hosted-only; use ssh start|status|stop",
            )?;
        }
        #[cfg(all(not(feature = "ssh"), not(all(target_arch = "arm", feature = "ssh-bm"))))]
        Command::SshStatus
        | Command::SshStart
        | Command::SshStop
        | Command::SshRestart
        | Command::SshSessions
        | Command::SshSessionShow { .. }
        | Command::SshSessionClose { .. }
        | Command::SshConfig
        | Command::SshKeyList
        | Command::SshKeyAdd { .. }
        | Command::SshKeyRemove { .. }
        | Command::SshKeyEnable { .. }
        | Command::SshKeyDisable { .. }
        | Command::SshHostKeyFingerprint
        | Command::SshHostKeyStatus
        | Command::SshHostKeyRegenerate
        | Command::SshAudit => {
            ctx.writeln_line("ssh: requires ssh feature (hosted russh) or ssh-bm on PZ7020")?;
        }
        Command::AnomSecurity { passphrase } => anom_cmd::run_security(ctx, passphrase)?,
        Command::AnomStatus => anom_cmd::run_status(ctx)?,
        Command::AnomObjectStatus { object } => anom_cmd::run_object_status(ctx, &object)?,
        Command::AnomOff { passphrase } => anom_cmd::run_off(ctx, passphrase)?,
        Command::AnomVerify => anom_cmd::run_verify(ctx)?,
        Command::AnomProtect { object, passphrase } => {
            anom_cmd::run_protect(ctx, &object, passphrase)?;
        }
        Command::AnomOpen { object, passphrase } => {
            anom_cmd::run_open(ctx, &object, passphrase)?;
        }
        Command::AnomClose { object } => anom_cmd::run_close(ctx, &object)?,
        Command::AnomRekey { object, passphrase } => {
            anom_cmd::run_rekey(ctx, &object, passphrase)?;
        }
        Command::AnomUnprotect { object, passphrase } => {
            anom_cmd::run_unprotect(ctx, &object, passphrase)?;
        }
        Command::AnomList => anom_cmd::run_list(ctx)?,
        Command::AnomSealTest { phase, passphrase } => {
            anom_cmd::run_seal_test(ctx, &phase, passphrase)?;
        }
        Command::StateModeQuery => state_domain_cmd::run_mode_query(ctx)?,
        Command::StateModeSet { mode } => state_domain_cmd::run_mode_set(ctx, &mode)?,
        Command::StateDomains => state_domain_cmd::run_domains(ctx)?,
        Command::StateMap { domain } => state_domain_cmd::run_map(ctx, &domain)?,
        Command::StateView { object } => state_domain_cmd::run_view(ctx, &object)?,
        Command::StateDomainChange { object, label } => {
            state_domain_cmd::run_change(ctx, &object, &label)?;
        }
        Command::StateProof { object } => state_domain_cmd::run_proof(ctx, &object)?,
        Command::NetStatus => net_cmd::run_status(ctx)?,
        Command::NetInit => net_cmd::run_init(ctx)?,
        Command::NetProbe => net_cmd::run_probe(ctx)?,
        Command::NetPhy => net_cmd::run_phy(ctx)?,
        Command::NetLink => net_cmd::run_link(ctx)?,
        Command::NetIpServe { octets, secs } => {
            #[cfg(all(target_arch = "arm", feature = "bm-net"))]
            match crate::kernel::net_ip::serve(octets, secs) {
                Ok(s) => ctx.writeln(&s)?,
                Err(e) => ctx.writeln_line(e)?,
            }
            #[cfg(not(all(target_arch = "arm", feature = "bm-net")))]
            {
                let _ = (octets, secs);
                ctx.writeln_line("net ip: requires a bm-net baremetal (zynq) build")?;
            }
        }
        Command::NetIpDhcp { secs } => {
            #[cfg(all(target_arch = "arm", feature = "bm-net"))]
            match crate::kernel::net_ip::dhcp(secs) {
                Ok(s) => ctx.writeln(&s)?,
                Err(e) => ctx.writeln_line(e)?,
            }
            #[cfg(not(all(target_arch = "arm", feature = "bm-net")))]
            {
                let _ = secs;
                ctx.writeln_line("net ip dhcp: requires a bm-net baremetal (zynq) build")?;
            }
        }
        Command::NetDnsResolve { host, secs } => {
            #[cfg(all(target_arch = "arm", feature = "bm-net"))]
            match crate::kernel::net_ip::dns(&host, secs) {
                Ok(s) => ctx.writeln(&s)?,
                Err(e) => ctx.writeln_line(e)?,
            }
            #[cfg(not(all(target_arch = "arm", feature = "bm-net")))]
            {
                let _ = (host, secs);
                ctx.writeln_line("net dns: requires a bm-net baremetal (zynq) build")?;
            }
        }
        Command::NetSshServe { octets, port, secs } => {
            #[cfg(all(target_arch = "arm", feature = "ssh-bm"))]
            {
                ctx.writeln_line(&format!(
                    "net ssh: starting SSH server on {}.{}.{}.{}:{} for {}s ...",
                    octets[0], octets[1], octets[2], octets[3], port, secs
                ))?;
                return Ok(crate::userland::shell::ShellAction::RunSshServer { octets, port, secs });
            }
            #[cfg(not(all(target_arch = "arm", feature = "ssh-bm")))]
            {
                let _ = (octets, port, secs);
                ctx.writeln_line("net ssh: requires an ssh-bm baremetal (zynq) build")?;
            }
        }
        Command::NetSshKey { hex } => {
            #[cfg(all(target_arch = "arm", feature = "ssh-bm"))]
            {
                let h = hex.trim();
                if h.len() != 64 {
                    ctx.writeln_line("net ssh key: need 64 hex chars (raw ed25519 pubkey)")?;
                } else {
                    let mut key = [0u8; 32];
                    let mut ok = true;
                    for i in 0..32 {
                        match u8::from_str_radix(&h[i * 2..i * 2 + 2], 16) {
                            Ok(b) => key[i] = b,
                            Err(_) => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok {
                        crate::kernel::net_ssh::set_authorized_key(key);
                        ctx.writeln_line("net ssh key: authorized pubkey set (not committed)")?;
                    } else {
                        ctx.writeln_line("net ssh key: invalid hex")?;
                    }
                }
            }
            #[cfg(not(all(target_arch = "arm", feature = "ssh-bm")))]
            {
                let _ = hex;
                ctx.writeln_line("net ssh key: requires ssh-bm")?;
            }
        }
        Command::RngStatus => {
            #[cfg(all(target_arch = "arm", any(feature = "ssh-bm", feature = "identity")))]
            {
                ctx.writeln(&crate::board::zynq7000::rng::status_lines())?;
            }
            #[cfg(not(all(target_arch = "arm", any(feature = "ssh-bm", feature = "identity"))))]
            {
                ctx.writeln_line("HARDWARE_TRNG: UNAVAILABLE_ON_CURRENT_BOARD")?;
                ctx.writeln_line("CURRENT_SOURCE: TIMER_JITTER_SEEDED_DRBG")?;
            }
        }
        Command::DmaStatus => {
            #[cfg(any(feature = "std", feature = "baremetal"))]
            {
                ctx.writeln(&ctx.kernel.dma.status_lines())?;
            }
            #[cfg(all(target_arch = "arm", feature = "usb"))]
            {
                ctx.writeln(&crate::board::zynq7000::usb::dma_status_lines())?;
            }
        }
        Command::UsbProbe => {
            #[cfg(all(target_arch = "arm", feature = "usb"))]
            {
                use crate::kernel::state_domain::DomainId;
                // Muscle: bring up USB0 host + read the root port.
                let (report, st) = crate::board::zynq7000::usb::probe();
                ctx.writeln_line(&report)?;
                // Brain: walk the DomainId::Usb phase lifecycle, statechain-recorded.
                ctx.kernel.init_state_domain();
                let mut applied: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
                for (cond, label, op) in [
                    (true, "powered", "usb_power"),
                    (st.connected, "connected", "usb_connect"),
                    (st.reset_done, "reset", "usb_reset"),
                ] {
                    if cond
                        && ctx
                            .kernel
                            .change_domain_state("usb.port0", &DomainId::Usb, label, op)
                            .is_ok()
                    {
                        applied.push(label);
                    }
                }
                ctx.writeln_line(&format!(
                    "usb: phase lifecycle -> {} (recorded in statechain, domain=usb)",
                    applied.join(" -> ")
                ))?;
            }
            #[cfg(not(all(target_arch = "arm", feature = "usb")))]
            {
                ctx.writeln_line("usb: requires an arm usb baremetal (zynq) build")?;
            }
        }
        Command::UsbEnum => {
            #[cfg(all(target_arch = "arm", feature = "usb"))]
            {
                use crate::kernel::state_domain::DomainId;
                let (report, st) = crate::board::zynq7000::usb::enum_spike();
                ctx.writeln_line(&report)?;
                ctx.kernel.init_state_domain();
                for (cond, label, op) in [
                    (true, "powered", "usb_power"),
                    (st.connected, "connected", "usb_connect"),
                    (st.reset_done, "reset", "usb_reset"),
                ] {
                    if cond {
                        let _ = ctx
                            .kernel
                            .change_domain_state("usb.port0", &DomainId::Usb, label, op);
                    }
                }
            }
            #[cfg(not(all(target_arch = "arm", feature = "usb")))]
            {
                ctx.writeln_line("usb enum: requires an arm usb baremetal (zynq) build")?;
            }
        }
        Command::UsbList => {
            #[cfg(all(target_arch = "arm", feature = "usb"))]
            {
                use crate::kernel::state_domain::DomainId;
                // Full U1 enumeration: device -> address -> config -> HID parse.
                let (report, st) = crate::board::zynq7000::usb::list();
                ctx.writeln_line(&report)?;
                // Brain: walk the lifecycle to `configured` on a successful enum.
                ctx.kernel.init_state_domain();
                let ok = st.connected && report.starts_with("usb1:");
                let mut applied: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
                for (cond, label, op) in [
                    (true, "powered", "usb_power"),
                    (st.connected, "connected", "usb_connect"),
                    (st.reset_done, "reset", "usb_reset"),
                    (ok, "addressed", "usb_set_address"),
                    (ok, "configured", "usb_set_config"),
                ] {
                    if cond
                        && ctx
                            .kernel
                            .change_domain_state("usb.port0", &DomainId::Usb, label, op)
                            .is_ok()
                    {
                        applied.push(label);
                    }
                }
                ctx.writeln_line(&format!(
                    "usb: phase lifecycle -> {} (recorded in statechain, domain=usb)",
                    applied.join(" -> ")
                ))?;
            }
            #[cfg(not(all(target_arch = "arm", feature = "usb")))]
            {
                ctx.writeln_line("usb list: requires an arm usb baremetal (zynq) build")?;
            }
        }
        Command::UsbKeys => {
            #[cfg(all(target_arch = "arm", feature = "usb"))]
            {
                use crate::kernel::state_domain::DomainId;
                // U2: enumerate, boot protocol, poll the interrupt-IN endpoint.
                let report = crate::board::zynq7000::usb::keys();
                ctx.writeln_line(&report)?;
                if report.contains("report(s) captured") && !report.contains(": 0 report") {
                    ctx.kernel.init_state_domain();
                    let _ = ctx.kernel.change_domain_state(
                        "usb.port0",
                        &DomainId::Usb,
                        "hid_ready",
                        "usb_hid_ready",
                    );
                    ctx.writeln_line("usb: phase -> hid_ready (recorded in statechain, domain=usb)")?;
                }
            }
            #[cfg(not(all(target_arch = "arm", feature = "usb")))]
            {
                ctx.writeln_line("usb keys: requires an arm usb baremetal (zynq) build")?;
            }
        }
        Command::UsbKbd => {
            #[cfg(all(target_arch = "arm", feature = "usb"))]
            {
                use crate::kernel::state_domain::DomainId;
                let _ = crate::board::zynq7000::usb_hid::detach_hid();
                match crate::board::zynq7000::usb::attach_keyboard() {
                    Ok(dev) => {
                        ctx.writeln_line(&format!(
                            "usb: keyboard attached {:04x}:{:04x} ep=0x{:02x} — now type at gpos>",
                            dev.vid, dev.pid, dev.ep_in
                        ))?;
                        ctx.kernel.init_state_domain();
                        for (label, op) in [("hid_ready", "usb_hid_ready"), ("polling", "usb_poll")] {
                            let _ = ctx.kernel.change_domain_state(
                                "usb.port0",
                                &DomainId::Usb,
                                label,
                                op,
                            );
                        }
                        ctx.writeln_line("usb: phase -> polling (keyboard drives the shell)")?;
                    }
                    Err(e) => ctx.writeln_line(e)?,
                }
            }
            #[cfg(not(all(target_arch = "arm", feature = "usb")))]
            {
                ctx.writeln_line("usb kbd: requires an arm usb baremetal (zynq) build")?;
            }
        }
        Command::UsbHid => {
            #[cfg(all(target_arch = "arm", feature = "usb"))]
            {
                match crate::board::zynq7000::usb_hid::attach_hid() {
                    Ok(report) => ctx.writeln_line(&report)?,
                    Err(e) => ctx.writeln_line(e)?,
                }
            }
            #[cfg(not(all(target_arch = "arm", feature = "usb")))]
            {
                ctx.writeln_line("usb hid: requires an arm usb baremetal (zynq) build")?;
            }
        }
        Command::UsbEvents => {
            #[cfg(all(target_arch = "arm", feature = "usb"))]
            {
                ctx.writeln_line(&crate::board::zynq7000::usb_hid::capture_events(3000))?;
            }
            #[cfg(not(all(target_arch = "arm", feature = "usb")))]
            {
                ctx.writeln_line("usb events: requires an arm usb baremetal (zynq) build")?;
            }
        }
        Command::UsbHidNeg => {
            use crate::kernel::hid::{run_negative_suite, NegCase};
            let mut cases = [NegCase {
                name: "",
                result: Ok(()),
            }; 16];
            let n = run_negative_suite(&mut cases);
            ctx.writeln_line("usb hid-neg: software fixtures (not hardware)")?;
            let mut unexpected = 0u32;
            for c in cases[..n].iter() {
                match c.result {
                    Err(e) => ctx.writeln_line(&format!("  {} -> rejected ({})", c.name, e.as_str()))?,
                    Ok(()) => {
                        unexpected += 1;
                        ctx.writeln_line(&format!("  {} -> accepted (unexpected)", c.name))?;
                    }
                }
            }
            ctx.writeln_line(&format!(
                "usb hid-neg: {n} cases, {unexpected} unexpected accept(s)"
            ))?;
        }
        Command::UsbHidDetach => {
            #[cfg(all(target_arch = "arm", feature = "usb"))]
            {
                ctx.writeln_line(&crate::board::zynq7000::usb_hid::detach_hid())?;
            }
            #[cfg(not(all(target_arch = "arm", feature = "usb")))]
            {
                ctx.writeln_line("usb hid-detach: requires an arm usb baremetal (zynq) build")?;
            }
        }
        Command::SystemSelftest => selftest_cmd::run(ctx)?,
        Command::Fault { args } => crate::userland::fault_cmd::run(ctx, &args)?,
        Command::FsSdList => {
            #[cfg(all(target_arch = "arm", feature = "baremetal"))]
            ctx.writeln(&crate::userland::fs_sd::list())?;
            #[cfg(not(all(target_arch = "arm", feature = "baremetal")))]
            ctx.writeln_line("fs: SD persistence requires a baremetal (zynq) build")?;
        }
        Command::FsSdSave { name, data } => {
            #[cfg(all(target_arch = "arm", feature = "baremetal"))]
            ctx.writeln_line(&crate::userland::fs_sd::save(&name, data.as_bytes()))?;
            #[cfg(not(all(target_arch = "arm", feature = "baremetal")))]
            {
                let _ = (name, data);
                ctx.writeln_line("fs: SD persistence requires a baremetal (zynq) build")?;
            }
        }
        Command::FsSdCat { name } => {
            #[cfg(all(target_arch = "arm", feature = "baremetal"))]
            ctx.writeln(&crate::userland::fs_sd::cat(&name))?;
            #[cfg(not(all(target_arch = "arm", feature = "baremetal")))]
            {
                let _ = name;
                ctx.writeln_line("fs: SD persistence requires a baremetal (zynq) build")?;
            }
        }
        Command::FsSdRemove { name } => {
            #[cfg(all(target_arch = "arm", feature = "baremetal"))]
            ctx.writeln_line(&crate::userland::fs_sd::remove(&name))?;
            #[cfg(not(all(target_arch = "arm", feature = "baremetal")))]
            {
                let _ = name;
                ctx.writeln_line("fs: SD persistence requires a baremetal (zynq) build")?;
            }
        }
        Command::NetLoopback => net_cmd::run_loopback(ctx)?,
        Command::NetTxTest => net_cmd::run_tx_test(ctx)?,
        Command::NetRxTest => net_cmd::run_rx_test(ctx)?,
        Command::NetWireInit => net_cmd::run_wire_init(ctx)?,
        Command::NetWireStatus => net_cmd::run_wire_status(ctx)?,
        Command::NetWirePing => net_cmd::run_wire_ping(ctx)?,
        Command::NetWirePingDebug => net_cmd::run_wire_ping_debug(ctx)?,
        Command::NetWireWrap => net_cmd::run_wire_wrap(ctx)?,
        Command::NetWireRecover => net_cmd::run_wire_recover(ctx)?,
        Command::NetWirePhyloop => net_cmd::run_wire_phyloop(ctx)?,
        Command::NetWireTxBurst { count } => net_cmd::run_wire_txburst(ctx, count)?,
        Command::NetPeer { count } => net_cmd::run_peer(ctx, count)?,
        Command::PhiStabilityStatus => phi_stability_cmd::run_status(ctx)?,
        Command::PhiStabilityRun { n } => phi_stability_cmd::run_local(ctx, n)?,
        Command::PhiStabilityDualClock { n } => phi_stability_cmd::run_dual_clock(ctx, n)?,
        Command::PhiStabilityCompare { n } => phi_stability_cmd::run_compare(ctx, n)?,
        Command::PhiStabilityNetworkSend { peer, n } => {
            phi_stability_cmd::run_network_send(ctx, &peer, n)?
        }
        Command::PhiStabilityNetworkReceive => phi_stability_cmd::run_network_receive(ctx)?,
        Command::PhiStabilityFault { fault } => phi_stability_cmd::run_fault(ctx, &fault)?,
        Command::G1pInfo => g1p_cmd::run_info(ctx)?,
        Command::G1pExec { a, b } => g1p_cmd::run_exec(ctx, a, b)?,
        Command::G1pReflect { a } => g1p_cmd::run_reflect(ctx, a)?,
        Command::G1pSelftest => g1p_cmd::run_selftest(ctx)?,
        Command::G1pConsensus => g1p_cmd::run_consensus(ctx)?,
        Command::G1pIrqTest { count } => g1p_cmd::run_irq_test(ctx, count)?,
        Command::G64Info => g64_cmd::run_info(ctx)?,
        Command::G64Oracle => g64_cmd::run_oracle(ctx)?,
        Command::G64Atlas => g64_cmd::run_atlas(ctx)?,
        Command::G64ComposeSweep => g64_cmd::run_compose_sweep(ctx)?,
        Command::G64ReflectSweep => g64_cmd::run_reflect_sweep(ctx)?,
        Command::G64Benchmark => g64_cmd::run_benchmark(ctx)?,
        #[cfg(feature = "phaseguard-research")]
        Command::PhaseGuardStatus => phaseguard_cmd::run_status(ctx)?,
        #[cfg(feature = "phaseguard-research")]
        Command::PhaseGuardTrace => phaseguard_cmd::run_trace(ctx)?,
        #[cfg(feature = "phaseguard-research")]
        Command::PhaseGuardSample => phaseguard_cmd::run_sample_lawful(ctx)?,
        #[cfg(feature = "std")]
        Command::PhaseBenchmarkStatus => phase_benchmark_cmd::run_status(ctx)?,
        #[cfg(feature = "std")]
        Command::PhaseBenchmarkRun { n } => phase_benchmark_cmd::run_single(ctx, n)?,
        #[cfg(feature = "std")]
        Command::PhaseBenchmarkCompare { n } => phase_benchmark_cmd::run_compare(ctx, n)?,
        #[cfg(feature = "std")]
        Command::PhaseBenchmarkSweep { start, end } => {
            phase_benchmark_cmd::run_sweep(ctx, start, end)?
        }
        #[cfg(feature = "std")]
        Command::PhaseBenchmarkRepeat { n, count } => {
            phase_benchmark_cmd::run_repeat(ctx, n, count)?
        }
        #[cfg(feature = "std")]
        Command::PhaseBenchmarkScale { n, workers } => {
            phase_benchmark_cmd::run_scale(ctx, n, workers)?
        }
        #[cfg(feature = "std")]
        Command::PhaseBenchmarkDualClock { n } => phase_benchmark_cmd::run_dual_clock(ctx, n)?,
        #[cfg(feature = "std")]
        Command::PhaseBenchmarkNetworkCompare { peer, n } => {
            phase_benchmark_cmd::run_network_compare(ctx, &peer, n)?
        }
        #[cfg(feature = "std")]
        Command::PhaseBenchmarkReport => phase_benchmark_cmd::run_report(ctx)?,
        #[cfg(not(feature = "std"))]
        Command::PhaseBenchmarkStatus => {
            ctx.writeln_line(
                "phase-benchmark: integer/scalar on BM (BigUint is hosted-only)",
            )?;
            phi_stability_cmd::run_status(ctx)?;
        }
        #[cfg(not(feature = "std"))]
        Command::PhaseBenchmarkRun { n } => {
            ctx.writeln_line(
                "phase-benchmark: integer/scalar on BM (not BigUint)",
            )?;
            phi_stability_cmd::run_local(ctx, n)?;
        }
        #[cfg(not(feature = "std"))]
        Command::PhaseBenchmarkCompare { n } => {
            ctx.writeln_line(
                "phase-benchmark: integer/scalar compare on BM (not BigUint)",
            )?;
            phi_stability_cmd::run_compare(ctx, n)?;
        }
        #[cfg(not(feature = "std"))]
        Command::PhaseBenchmarkDualClock { n } => {
            ctx.writeln_line(
                "phase-benchmark: integer/scalar dual-clock on BM (not BigUint)",
            )?;
            phi_stability_cmd::run_dual_clock(ctx, n)?;
        }
        #[cfg(not(feature = "std"))]
        Command::PhaseBenchmarkReport => {
            ctx.writeln_line(
                "phase-benchmark: integer/scalar on BM (BigUint report is hosted-only)",
            )?;
            phi_stability_cmd::run_status(ctx)?;
        }
        #[cfg(not(feature = "std"))]
        Command::PhaseBenchmarkSweep { .. }
        | Command::PhaseBenchmarkRepeat { .. }
        | Command::PhaseBenchmarkScale { .. }
        | Command::PhaseBenchmarkNetworkCompare { .. } => {
            ctx.writeln_line(
                "phase-benchmark sweep/repeat/scale/network-compare: hosted BigUint only; on BM use phase-benchmark run|compare|dual-clock",
            )?;
        }
        #[cfg(all(feature = "baremetal", not(feature = "std"), feature = "bm-net"))]
        Command::PhaseLinkStatus => phaselink_baremetal_cmd::run_status(ctx)?,
        #[cfg(all(feature = "baremetal", not(feature = "std"), feature = "bm-net"))]
        Command::PhaseLinkBaremetalInit { local, peer } => {
            phaselink_baremetal_cmd::run_init(ctx, &local, &peer)?
        }
        #[cfg(all(feature = "baremetal", not(feature = "std"), feature = "bm-net"))]
        Command::PhaseLinkBaremetalSend { peer, text } => {
            phaselink_baremetal_cmd::run_send(ctx, &peer, &text)?
        }
        #[cfg(all(feature = "baremetal", not(feature = "std"), feature = "bm-net"))]
        Command::PhaseLinkBaremetalRecv => phaselink_baremetal_cmd::run_recv(ctx)?,
        #[cfg(all(feature = "baremetal", not(feature = "std"), feature = "bm-net"))]
        Command::PhaseLinkBaremetalReplay => phaselink_baremetal_cmd::run_replay(ctx)?,
        #[cfg(all(feature = "baremetal", not(feature = "std"), feature = "bm-net"))]
        Command::PhaseLinkBaremetalBridge { text } => {
            #[cfg(target_arch = "arm")]
            phaselink_baremetal_cmd::run_bridge(ctx, &text)?;
            #[cfg(not(target_arch = "arm"))]
            {
                let _ = text;
                ctx.writeln_line("bridge: zynq GEM0 only")?;
            }
        }
        #[cfg(all(feature = "baremetal", not(feature = "std"), not(feature = "bm-net")))]
        Command::PhaseLinkStatus
        | Command::PhaseLinkBaremetalInit { .. }
        | Command::PhaseLinkBaremetalSend { .. }
        | Command::PhaseLinkBaremetalRecv
        | Command::PhaseLinkBaremetalReplay
        | Command::PhaseLinkBaremetalBridge { .. } => {
            ctx.writeln_line("phaselink requires bm-net (build with bm-embedded or bm-full)")?;
        }
        Command::PhasePerfStatus => phaseperf_cmd::run_status(ctx)?,
        Command::PhasePerfBackend { backend } => phaseperf_cmd::run_backend(ctx, &backend)?,
        Command::PhasePerfQueues => phaseperf_cmd::run_queues(ctx)?,
        Command::PhasePerfBenchmark => phaseperf_cmd::run_benchmark(ctx)?,
        Command::PhasePerfVerify => phaseperf_cmd::run_verify(ctx)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkStatus => phaselink_cmd::run_status(ctx)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkEncode { text } => phaselink_cmd::run_encode(ctx, &text)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkDecode { symbols } => phaselink_cmd::run_decode(ctx, &symbols)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkLoopback { text } => phaselink_cmd::run_loopback(ctx, &text)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkChain => phaselink_cmd::run_chain(ctx)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkRoot => phaselink_cmd::run_root(ctx)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkQuarantine => phaselink_cmd::run_quarantine(ctx)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkVerify { frame_id } => phaselink_cmd::run_verify(ctx, &frame_id)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkSimInit => phaselink_cmd::run_sim_init(ctx)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkSimStatus => phaselink_cmd::run_sim_status(ctx)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkSimSend {
            source,
            destination,
            text,
        } => phaselink_cmd::run_sim_send(ctx, &source, &destination, &text)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkSimReplay => phaselink_cmd::run_sim_replay(ctx)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkSimTamper { field } => phaselink_cmd::run_sim_tamper(ctx, &field)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkSimReboot { node } => phaselink_cmd::run_sim_reboot(ctx, &node)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkSimReset => phaselink_cmd::run_sim_reset(ctx)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkGatewayStatus => phaselink_cmd::run_gateway_status(ctx)?,
        #[cfg(feature = "std")]
        Command::PhaseLinkGatewayEnable { passphrase } => {
            phaselink_cmd::run_gateway_enable(ctx, passphrase)?
        }
        #[cfg(feature = "std")]
        Command::PhaseLinkGatewayDisable => phaselink_cmd::run_gateway_disable(ctx)?,
        Command::PhaseNetStatus => phasenet_cmd::run_status(ctx)?,
        Command::PhaseNetStreamEncode { text } => phasenet_cmd::run_stream_encode(ctx, &text)?,
        Command::PhaseNetStreamDecode { hex } => phasenet_cmd::run_stream_decode(ctx, &hex)?,
        Command::PhaseNetAddr { uri } => phasenet_cmd::run_addr(ctx, &uri)?,
        Command::PhaseNetLoopbackSend {
            uri,
            operation,
            old_phi,
            new_phi,
        } => phasenet_cmd::run_loopback_send(ctx, &uri, &operation, &old_phi, &new_phi)?,
        Command::PhaseNetLoopbackRecv => phasenet_cmd::run_loopback_recv(ctx)?,
        Command::PhaseNetSimInit => phasenet_cmd::run_sim_init(ctx)?,
        Command::PhaseNetSimSend {
            from,
            to,
            uri,
            operation,
            old_phi,
            new_phi,
        } => phasenet_cmd::run_sim_send(ctx, &from, &to, &uri, &operation, &old_phi, &new_phi)?,
        Command::PhaseNetSimRecv { node } => phasenet_cmd::run_sim_recv(ctx, &node)?,
        Command::PhaseNetView { uri } => phasenet_cmd::run_view(ctx, &uri)?,
        #[cfg(not(feature = "std"))]
        Command::PhaseLinkEncode { .. }
        | Command::PhaseLinkDecode { .. }
        | Command::PhaseLinkLoopback { .. }
        | Command::PhaseLinkChain
        | Command::PhaseLinkRoot
        | Command::PhaseLinkQuarantine
        | Command::PhaseLinkVerify { .. }
        | Command::PhaseLinkSimInit
        | Command::PhaseLinkSimStatus
        | Command::PhaseLinkSimSend { .. }
        | Command::PhaseLinkSimReplay
        | Command::PhaseLinkSimTamper { .. }
        | Command::PhaseLinkSimReboot { .. }
        | Command::PhaseLinkSimReset
        | Command::PhaseLinkGatewayStatus
        | Command::PhaseLinkGatewayEnable { .. }
        | Command::PhaseLinkGatewayDisable => {
            ctx.writeln_line(
                "phaselink sim/gateway: hosted-only; BM: phaselink status|init|send|recv|replay over GEM0 0x88B5",
            )?;
        }
        Command::Exit | Command::Unknown(_) | Command::Usage(_) => unreachable!(),
    }

    if !skip_kairos {
        stateclock::record_kairos_on_kernel(ctx.kernel, KairosEventKind::Command, line);
    }

    Ok(ShellAction::Continue)
}

fn command_trace_line(cmd: &Command) -> String {
    match cmd {
        Command::Help => "help".into(),
        Command::About => "about".into(),
        Command::Clear => "clear".into(),
        Command::Version => "version".into(),
        Command::State => "state".into(),
        Command::Clock => "clock".into(),
        Command::NetStatus => "net status".into(),
        Command::NetInit => "net init".into(),
        Command::NetProbe => "net probe".into(),
        Command::NetPhy => "net phy".into(),
        Command::NetLink => "net link".into(),
        Command::NetIpServe { .. } => "net ip".into(),
        Command::NetIpDhcp { .. } => "net ip dhcp".into(),
        Command::NetDnsResolve { .. } => "net dns".into(),
        Command::NetSshServe { .. } => "net ssh".into(),
        Command::NetSshKey { .. } => "net ssh key".into(),
        Command::RngStatus => "rng status".into(),
        Command::DmaStatus => "dma status".into(),
        Command::UsbProbe => "usb probe".into(),
        Command::UsbEnum => "usb enum".into(),
        Command::UsbList => "usb list".into(),
        Command::UsbKeys => "usb keys".into(),
        Command::UsbKbd => "usb kbd".into(),
        Command::UsbHid => "usb hid".into(),
        Command::UsbEvents => "usb events".into(),
        Command::UsbHidNeg => "usb hid-neg".into(),
        Command::UsbHidDetach => "usb hid-detach".into(),
        Command::SystemSelftest => "system selftest".into(),
        Command::Fault { .. } => "fault".into(),
        Command::FsSdList => "fs ls".into(),
        Command::FsSdSave { .. } => "fs save".into(),
        Command::FsSdCat { .. } => "fs cat".into(),
        Command::FsSdRemove { .. } => "fs rm".into(),
        Command::NetLoopback => "net loopback".into(),
        Command::NetTxTest => "net tx-test".into(),
        Command::NetRxTest => "net rx-test".into(),
        Command::NetWireInit => "net wire init".into(),
        Command::NetWireStatus => "net wire status".into(),
        Command::NetWirePing => "net wire ping".into(),
        Command::NetWirePingDebug => "net wire ping debug".into(),
        Command::NetWireWrap => "net wire wrap".into(),
        Command::NetWireRecover => "net wire recover".into(),
        Command::NetWirePhyloop => "net wire phyloop".into(),
        Command::NetWireTxBurst { count } => format!("net wire txburst {count}"),
        Command::NetPeer { count } => format!("net peer {count}"),
        Command::PhiStabilityStatus => "phi-stability status".into(),
        Command::PhiStabilityRun { n } => format!("phi-stability run {n}"),
        Command::PhiStabilityDualClock { n } => format!("phi-stability dual-clock {n}"),
        Command::PhiStabilityCompare { n } => format!("phi-stability compare {n}"),
        Command::PhiStabilityNetworkSend { peer, n } => {
            format!("phi-stability network-send {peer} {n}")
        }
        Command::PhiStabilityNetworkReceive => "phi-stability network-receive".into(),
        Command::PhiStabilityFault { fault } => format!("phi-stability fault {fault}"),
        Command::G1pInfo => "g1p info".into(),
        Command::G1pExec { a, b } => format!("g1p exec {a} {b}"),
        Command::G1pReflect { a } => format!("g1p reflect {a}"),
        Command::G1pSelftest => "g1p selftest".into(),
        Command::G1pConsensus => "g1p consensus".into(),
        Command::G1pIrqTest { count } => format!("g1p irq-test {count}"),
        Command::G64Info => "g64 info".into(),
        Command::G64Oracle => "g64 oracle".into(),
        Command::G64Atlas => "g64 atlas".into(),
        Command::G64ComposeSweep => "g64 compose".into(),
        Command::G64ReflectSweep => "g64 reflect".into(),
        Command::G64Benchmark => "g64 benchmark".into(),
        #[cfg(feature = "phaseguard-research")]
        Command::PhaseGuardStatus => "phaseguard status".into(),
        #[cfg(feature = "phaseguard-research")]
        Command::PhaseGuardTrace => "phaseguard trace".into(),
        #[cfg(feature = "phaseguard-research")]
        Command::PhaseGuardSample => "phaseguard sample".into(),
        Command::PhaseBenchmarkStatus => "phase-benchmark status".into(),
        Command::PhaseBenchmarkRun { n } => format!("phase-benchmark run {n}"),
        Command::PhaseBenchmarkCompare { n } => format!("phase-benchmark compare {n}"),
        Command::PhaseBenchmarkSweep { start, end } => {
            format!("phase-benchmark sweep {start} {end}")
        }
        Command::PhaseBenchmarkRepeat { n, count } => {
            format!("phase-benchmark repeat {n} {count}")
        }
        Command::PhaseBenchmarkScale { n, workers } => {
            format!("phase-benchmark scale {n} {workers}")
        }
        Command::PhaseBenchmarkDualClock { n } => format!("phase-benchmark dual-clock {n}"),
        Command::PhaseBenchmarkNetworkCompare { peer, n } => {
            format!("phase-benchmark network-compare {peer} {n}")
        }
        Command::PhaseBenchmarkReport => "phase-benchmark report".into(),
        Command::PhasePerfStatus => "phaseperf status".into(),
        Command::PhasePerfBackend { backend } => format!("phaseperf backend {backend}"),
        Command::PhasePerfQueues => "phaseperf queues".into(),
        Command::PhasePerfBenchmark => "phaseperf benchmark".into(),
        Command::PhasePerfVerify => "phaseperf verify".into(),
        Command::PhaseLinkStatus => "phaselink status".into(),
        Command::PhaseLinkEncode { text } => format!("phaselink encode {text}"),
        Command::PhaseLinkDecode { symbols } => format!("phaselink decode {}", symbols.join(" ")),
        Command::PhaseLinkLoopback { text } => format!("phaselink loopback {text}"),
        Command::PhaseLinkChain => "phaselink chain".into(),
        Command::PhaseLinkRoot => "phaselink root".into(),
        Command::PhaseLinkQuarantine => "phaselink quarantine".into(),
        Command::PhaseLinkVerify { frame_id } => format!("phaselink verify {frame_id}"),
        Command::PhaseLinkSimInit => "phaselink sim init".into(),
        Command::PhaseLinkSimStatus => "phaselink sim status".into(),
        Command::PhaseLinkSimSend {
            source,
            destination,
            text,
        } => {
            format!("phaselink sim send {source} {destination} {text}")
        }
        Command::PhaseLinkSimReplay => "phaselink sim replay".into(),
        Command::PhaseLinkSimTamper { field } => format!("phaselink sim tamper {field}"),
        Command::PhaseLinkSimReboot { node } => format!("phaselink sim reboot {node}"),
        Command::PhaseLinkSimReset => "phaselink sim reset".into(),
        Command::PhaseLinkGatewayStatus => "phaselink gateway status".into(),
        Command::PhaseLinkGatewayEnable { .. } => "phaselink gateway enable -p".into(),
        Command::PhaseLinkGatewayDisable => "phaselink gateway disable".into(),
        #[cfg(all(feature = "baremetal", not(feature = "std")))]
        Command::PhaseLinkBaremetalInit { local, peer } => {
            format!("phaselink init {local} {peer}")
        }
        #[cfg(all(feature = "baremetal", not(feature = "std")))]
        Command::PhaseLinkBaremetalSend { peer, text } => format!("phaselink send {peer} {text}"),
        #[cfg(all(feature = "baremetal", not(feature = "std")))]
        Command::PhaseLinkBaremetalRecv => "phaselink recv".into(),
        #[cfg(all(feature = "baremetal", not(feature = "std")))]
        Command::PhaseLinkBaremetalReplay => "phaselink replay".into(),
        #[cfg(all(feature = "baremetal", not(feature = "std")))]
        Command::PhaseLinkBaremetalBridge { text } => format!("phaselink bridge {text}"),
        Command::ClockSkew => "clock skew".into(),
        Command::ClockSetDilation { value } => format!("clock set-dilation {value}"),
        Command::ClockResync => "clock resync".into(),
        Command::Regs => "regs".into(),
        Command::Ps => "ps".into(),
        Command::Whoami => "whoami".into(),
        Command::Host => "host".into(),
        Command::Spawn { name } => format!("spawn {name}"),
        Command::Kill { pid } => format!("kill {pid}"),
        Command::Trace => "trace".into(),
        Command::Isa => "isa".into(),
        Command::Dump => "dump".into(),
        Command::RunPaste => "run".into(),
        Command::RunFile { filename } => format!("run file {filename}"),
        Command::RunProgram { name } => format!("run {name}"),
        Command::Mem => "mem".into(),
        Command::Log => "log".into(),
        Command::Ls { path } => format!("ls {path}"),
        Command::Cat { path } => format!("cat {path}"),
        Command::Write { path, text } => format!("write {path} {text}"),
        Command::Mkdir { path } => format!("mkdir {path}"),
        Command::Rename { from, to } => format!("rename {from} {to}"),
        Command::Rm { path } => format!("rm {path}"),
        Command::Stat { path } => format!("stat {path}"),
        Command::Lock { path } => format!("lock {path}"),
        Command::Unlock { path } => format!("unlock {path}"),
        Command::Open { path, mode } => format!("open {path} {mode}"),
        Command::Close { slot, generation } => format!("close {slot} {generation}"),
        Command::Captable => "captable".into(),
        Command::Passwd => "passwd".into(),
        Command::PasswdStatus => "passwd --status".into(),
        Command::Login => "login".into(),
        Command::Logout => "logout".into(),
        Command::ModList => "mod -l".into(),
        Command::Mod { verb, lease } => format!("mod {verb} --lease {lease}"),
        Command::ModKill { ticket_id } => format!("mod -k {ticket_id}"),
        Command::App { args } => format!("app {}", args.join(" ")),
        Command::Plugin { args } => format!("plugin {}", args.join(" ")),
        Command::ModApp { args } => format!("mod app {}", args.join(" ")),
        Command::ModCertDeveloperRevoke { hash_hex } => {
            format!("mod cert developer-revoke {hash_hex}")
        },
        Command::Sentinel { args } => format!("sentinel {}", args.join(" ")),
        Command::MmuIso { args } => format!("mmuiso {}", args.join(" ")),
        Command::Dashboard => "dashboard".into(),
        Command::Top { frames } => format!("top -n {frames}"),
        Command::Watchdog => "watchdog".into(),
        Command::Phase => "phase".into(),
        Command::Topo => "topo".into(),
        Command::Curvature => "curvature".into(),
        Command::CurvatureInject { reps } => format!("curvature inject {reps}"),
        Command::TransportInit { size } => format!("transport init {size}"),
        Command::TransportStatus => "transport status".into(),
        Command::TransportSend {
            sr,
            sc,
            dr,
            dc,
            opcode,
            payload,
        } => format!("transport send {sr} {sc} {dr} {dc} {opcode} {payload}"),
        Command::TransportTick { n } => format!("transport tick {n}"),
        Command::TransportReceive { lane } => format!("transport receive {lane}"),
        Command::TransportRoutes => "transport routes".into(),
        Command::TransportTrace { packet_id } => format!("transport trace {packet_id}"),
        Command::TransportReplay { packet_id } => format!("transport replay {packet_id}"),
        Command::TransportInjectFault { packet_id, fault } => {
            format!("transport inject-fault {packet_id} {fault}")
        }
        Command::TransportReset => "transport reset".into(),
        Command::TransportDemo => "transport demo".into(),
        Command::CraftInit => "craft init".into(),
        Command::CraftStatus => "craft status".into(),
        Command::CraftTick { n } => format!("craft tick {n}"),
        Command::CraftCommand {
            subsystem,
            operation,
        } => format!("craft command {subsystem} {operation}"),
        Command::CraftFaultInject { reason } => {
            format!("craft fault inject prop-stuck --reason {reason}")
        }
        Command::CraftQuarantineStatus => "craft quarantine status".into(),
        Command::CraftEpisode { episode_id } => format!("craft episode {episode_id}"),
        Command::CraftReplay { episode_id } => format!("craft replay {episode_id}"),
        Command::ScenarioRun { name } => format!("scenario run {name}"),
        Command::Root => "root".into(),
        Command::Audit => "audit".into(),
        Command::Checkpoint => "checkpoint".into(),
        Command::CheckpointAuthStatus => "checkpoint-auth status".into(),
        Command::CheckpointAuthProvision => "checkpoint-auth provision".into(),
        Command::CheckpointAuthSeal => "checkpoint-auth seal".into(),
        Command::CheckpointAuthVerify => "checkpoint-auth verify".into(),
        Command::CheckpointAuthHardware => "checkpoint-auth hardware".into(),
        Command::PhaseCache => "phasecache".into(),
        Command::Pmm => "pmm".into(),
        Command::PmmSelftest => "pmm selftest".into(),
        Command::Irqreg => "irqreg".into(),
        Command::Sched2 => "sched2".into(),
        Command::Extmap => "extmap".into(),
        Command::Txlog => "txlog".into(),
        Command::TxlogDemo => "txlog demo".into(),
        Command::TxlogFault => "txlog fault".into(),
        Command::TxlogRecover => "txlog recover".into(),
        Command::Fsj => "fsj".into(),
        Command::FsjRecover => "fsj recover".into(),
        Command::FsjCheck => "fsj check".into(),
        Command::FsjFormat { confirm } => {
            if *confirm {
                "fsj format CONFIRM".into()
            } else {
                "fsj format".into()
            }
        }
        Command::FsjCrash { n } => {
            if *n == 0 {
                "fsj crash off".into()
            } else {
                format!("fsj crash {n}")
            }
        }
        Command::FsjP6 { confirm } => {
            if *confirm {
                "fsj p6 CONFIRM".into()
            } else {
                "fsj p6".into()
            }
        }
        Command::FsjP7 { run_id, token_ok } => {
            if *token_ok {
                format!("fsj p7 {run_id} PHASEFS-P7-CORRUPT-TAIL")
            } else {
                format!("fsj p7 {run_id}")
            }
        }
        Command::FsjP8 { run_id, token_ok } => {
            if *token_ok {
                format!("fsj p8 {run_id} PHASEFS-P8-WRAP")
            } else {
                format!("fsj p8 {run_id}")
            }
        }
        Command::FsjP10 { run_id, token_ok } => {
            if *token_ok {
                format!("fsj p10 {run_id} PHASEFS-P10-BEFORE-COMMIT")
            } else {
                format!("fsj p10 {run_id}")
            }
        }
        Command::FsjP11 { run_id, token_ok } => {
            if *token_ok {
                format!("fsj p11 {run_id} PHASEFS-P11-AFTER-COMMIT")
            } else {
                format!("fsj p11 {run_id}")
            }
        }
        Command::FsjP1Reset => "fsj p1reset".into(),
        Command::Netsess => "netsess".into(),
        Command::NetsessDemo => "netsess demo".into(),
        Command::NetsessFault => "netsess fault".into(),
        Command::Sleep { ticks } => format!("sleep {ticks}"),
        Command::Ipc => "ipc".into(),
        Command::IpcCreate { capacity } => format!("ipccreate {capacity}"),
        Command::IpcSend {
            slot,
            generation,
            text,
        } => format!("ipcsend {slot} {generation} {text}"),
        Command::IpcRecv { slot, generation } => format!("ipcrecv {slot} {generation}"),
        Command::IpcClose { slot, generation } => format!("ipcclose {slot} {generation}"),
        Command::AiConnect { host, port } => format!("ai connect {host} {port}"),
        Command::AiDisconnect => "ai disconnect".into(),
        Command::AiStatus => "ai status".into(),
        Command::AiAsk { prompt } => format!("ai ask {prompt}"),
        Command::AiAskBm { octets, port, prompt } => format!(
            "ai remote {}.{}.{}.{} {port} {prompt}",
            octets[0], octets[1], octets[2], octets[3]
        ),
        Command::AiJobs => "ai jobs".into(),
        Command::AiShow { job_id } => format!("ai show {job_id}"),
        Command::AiTrace { job_id } => format!("ai trace {job_id}"),
        Command::AiCancel { job_id } => format!("ai cancel {job_id}"),
        Command::AiTools => "ai tools".into(),
        Command::AiAllow { tool } => format!("ai allow {tool}"),
        Command::AiDeny { tool } => format!("ai deny {tool}"),
        Command::AiServe { secs } => format!("ai serve {secs}"),
        Command::SshStatus => "ssh status".into(),
        Command::SshStart => "ssh start".into(),
        Command::SshStop => "ssh stop".into(),
        Command::SshRestart => "ssh restart".into(),
        Command::SshSessions => "ssh sessions".into(),
        Command::SshSessionShow { id } => format!("ssh session show {id}"),
        Command::SshSessionClose { id } => format!("ssh session close {id}"),
        Command::SshConfig => "ssh config".into(),
        Command::SshKeyList => "ssh key list".into(),
        Command::SshKeyAdd { public_key } => format!("ssh key add {public_key}"),
        Command::SshKeyRemove { fingerprint } => format!("ssh key remove {fingerprint}"),
        Command::SshKeyEnable { fingerprint } => format!("ssh key enable {fingerprint}"),
        Command::SshKeyDisable { fingerprint } => format!("ssh key disable {fingerprint}"),
        Command::SshHostKeyFingerprint => "ssh host-key fingerprint".into(),
        Command::SshHostKeyStatus => "ssh host-key status".into(),
        Command::SshHostKeyRegenerate => "ssh host-key regenerate".into(),
        Command::SshAudit => "ssh audit".into(),
        Command::AnomSecurity { .. } => "anom security -p".into(),
        Command::AnomStatus => "anom status".into(),
        Command::AnomObjectStatus { object } => format!("anom status {object}"),
        Command::AnomOff { .. } => "anom off -p".into(),
        Command::AnomVerify => "anom verify".into(),
        Command::AnomProtect { object, .. } => format!("anom protect {object} -p"),
        Command::AnomOpen { object, .. } => format!("anom open {object} -p"),
        Command::AnomClose { object } => format!("anom close {object}"),
        Command::AnomRekey { object, .. } => format!("anom rekey {object} -p"),
        Command::AnomUnprotect { object, .. } => format!("anom unprotect {object} -p"),
        Command::AnomList => "anom list".into(),
        Command::AnomSealTest { phase, .. } => format!("anom seal-test {phase} -p"),
        Command::StateModeQuery => "state mode".into(),
        Command::StateModeSet { mode } => format!("state mode {mode}"),
        Command::StateDomains => "state domains".into(),
        Command::StateMap { domain } => format!("state map {domain}"),
        Command::StateView { object } => format!("state view {object}"),
        Command::StateDomainChange { object, label } => {
            format!("state change {object} {label}")
        }
        Command::StateProof { object } => format!("state proof {object}"),
        Command::PhaseNetStatus => "phasenet status".into(),
        Command::PhaseNetStreamEncode { text } => format!("phasenet stream encode {text}"),
        Command::PhaseNetStreamDecode { hex } => format!("phasenet stream decode {hex}"),
        Command::PhaseNetAddr { uri } => format!("phasenet addr {uri}"),
        Command::PhaseNetLoopbackSend {
            uri,
            operation,
            old_phi,
            new_phi,
        } => format!("phasenet loopback send {uri} {operation} {old_phi} {new_phi}"),
        Command::PhaseNetLoopbackRecv => "phasenet loopback recv".into(),
        Command::PhaseNetSimInit => "phasenet sim init".into(),
        Command::PhaseNetSimSend {
            from,
            to,
            uri,
            operation,
            old_phi,
            new_phi,
        } => format!("phasenet sim send {from} {to} {uri} {operation} {old_phi} {new_phi}"),
        Command::PhaseNetSimRecv { node } => format!("phasenet sim recv {node}"),
        Command::PhaseNetView { uri } => format!("phasenet view {uri}"),
        Command::Exit => "exit".into(),
        Command::Unknown(s) => s.clone(),
        Command::Usage(s) => s.clone(),
    }
}
