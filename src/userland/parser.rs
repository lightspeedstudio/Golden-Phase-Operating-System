//! Shell command parser.

use crate::platform::{String, ToString, Vec};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Help,
    About,
    Clear,
    Version,
    State,
    Clock,
    ClockSkew,
    ClockSetDilation {
        value: f64,
    },
    ClockResync,
    Regs,
    Ps,
    Whoami,
    Host,
    Spawn {
        name: String,
    },
    Kill {
        pid: u32,
    },
    Trace,
    Isa,
    Dump,
    RunPaste,
    RunFile {
        filename: String,
    },
    RunProgram {
        name: String,
    },
    Mem,
    Log,
    Ls {
        path: String,
    },
    Cat {
        path: String,
    },
    Write {
        path: String,
        text: String,
    },
    Mkdir {
        path: String,
    },
    Rename {
        from: String,
        to: String,
    },
    Rm {
        path: String,
    },
    Stat {
        path: String,
    },
    Lock {
        path: String,
    },
    Unlock {
        path: String,
    },
    Open {
        path: String,
        mode: String,
    },
    Close {
        slot: u32,
        generation: u32,
    },
    Captable,
    Dashboard,
    Top {
        frames: u32,
    },
    Watchdog,
    Phase,
    Topo,
    Curvature,
    CurvatureInject {
        reps: u32,
    },
    TransportInit {
        size: usize,
    },
    TransportStatus,
    TransportSend {
        sr: usize,
        sc: usize,
        dr: usize,
        dc: usize,
        opcode: String,
        payload: String,
    },
    TransportTick {
        n: u64,
    },
    TransportReceive {
        lane: u32,
    },
    TransportRoutes,
    TransportTrace {
        packet_id: u64,
    },
    TransportReplay {
        packet_id: u64,
    },
    TransportInjectFault {
        packet_id: u64,
        fault: String,
    },
    TransportReset,
    TransportDemo,
    CraftInit,
    CraftStatus,
    CraftTick {
        n: u64,
    },
    CraftCommand {
        subsystem: String,
        operation: String,
    },
    CraftFaultInject {
        reason: String,
    },
    CraftQuarantineStatus,
    CraftEpisode {
        episode_id: u64,
    },
    CraftReplay {
        episode_id: u64,
    },
    ScenarioRun {
        name: String,
    },
    Root,
    Audit,
    Checkpoint,
    CheckpointAuthStatus,
    CheckpointAuthProvision,
    CheckpointAuthSeal,
    CheckpointAuthVerify,
    CheckpointAuthHardware,
    PhaseCache,
    Pmm,
    PmmSelftest,
    Irqreg,
    Sched2,
    Extmap,
    Txlog,
    TxlogDemo,
    TxlogFault,
    TxlogRecover,
    Fsj,
    FsjRecover,
    FsjCheck,
    FsjFormat {
        confirm: bool,
    },
    FsjCrash {
        n: u32,
    },
    FsjP6 {
        confirm: bool,
    },
    FsjP7 {
        run_id: u64,
        token_ok: bool,
    },
    FsjP8 {
        run_id: u64,
        token_ok: bool,
    },
    FsjP10 {
        run_id: u64,
        token_ok: bool,
    },
    FsjP11 {
        run_id: u64,
        token_ok: bool,
    },
    FsjP1Reset,
    Netsess,
    NetsessDemo,
    NetsessFault,
    Sleep {
        ticks: u64,
    },
    Ipc,
    IpcCreate {
        capacity: usize,
    },
    IpcSend {
        slot: u32,
        generation: u32,
        text: String,
    },
    IpcRecv {
        slot: u32,
        generation: u32,
    },
    IpcClose {
        slot: u32,
        generation: u32,
    },
    Passwd,
    PasswdStatus,
    Login,
    Logout,
    ModList,
    Mod {
        verb: String,
        lease: u64,
    },
    ModKill {
        ticket_id: u32,
    },
    App {
        args: Vec<String>,
    },
    Plugin {
        args: Vec<String>,
    },
    ModApp {
        args: Vec<String>,
    },
    ModCertDeveloperRevoke {
        hash_hex: String,
    },
    Sentinel {
        args: Vec<String>,
    },
    MmuIso {
        args: Vec<String>,
    },
    AnomSecurity {
        passphrase: bool,
    },
    AnomStatus,
    AnomObjectStatus {
        object: String,
    },
    AnomOff {
        passphrase: bool,
    },
    AnomVerify,
    AnomOpen {
        object: String,
        passphrase: bool,
    },
    AnomProtect {
        object: String,
        passphrase: bool,
    },
    AnomClose {
        object: String,
    },
    AnomRekey {
        object: String,
        passphrase: bool,
    },
    AnomUnprotect {
        object: String,
        passphrase: bool,
    },
    AnomList,
    AnomSealTest {
        phase: String,
        passphrase: bool,
    },
    StateModeQuery,
    StateModeSet {
        mode: String,
    },
    StateDomains,
    StateMap {
        domain: String,
    },
    StateView {
        object: String,
    },
    StateDomainChange {
        object: String,
        label: String,
    },
    StateProof {
        object: String,
    },
    PhaseNetStatus,
    PhaseNetStreamEncode {
        text: String,
    },
    PhaseNetStreamDecode {
        hex: String,
    },
    PhaseNetAddr {
        uri: String,
    },
    PhaseNetLoopbackSend {
        uri: String,
        operation: String,
        old_phi: String,
        new_phi: String,
    },
    PhaseNetLoopbackRecv,
    PhaseNetSimInit,
    PhaseNetSimSend {
        from: String,
        to: String,
        uri: String,
        operation: String,
        old_phi: String,
        new_phi: String,
    },
    PhaseNetSimRecv {
        node: String,
    },
    PhaseNetView {
        uri: String,
    },
    PhaseLinkStatus,
    PhaseLinkEncode {
        text: String,
    },
    PhaseLinkDecode {
        symbols: Vec<String>,
    },
    PhaseLinkLoopback {
        text: String,
    },
    PhaseLinkChain,
    PhaseLinkRoot,
    PhaseLinkQuarantine,
    PhaseLinkVerify {
        frame_id: String,
    },
    PhaseLinkSimInit,
    PhaseLinkSimStatus,
    PhaseLinkSimSend {
        source: String,
        destination: String,
        text: String,
    },
    PhaseLinkSimReplay,
    PhaseLinkSimTamper {
        field: String,
    },
    PhaseLinkSimReboot {
        node: String,
    },
    PhaseLinkSimReset,
    PhaseLinkGatewayStatus,
    PhaseLinkGatewayEnable {
        passphrase: bool,
    },
    PhaseLinkGatewayDisable,
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    PhaseLinkBaremetalInit {
        local: String,
        peer: String,
    },
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    PhaseLinkBaremetalSend {
        peer: String,
        text: String,
    },
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    PhaseLinkBaremetalRecv,
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    PhaseLinkBaremetalReplay,
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    PhaseLinkBaremetalBridge {
        text: String,
    },
    PhasePerfStatus,
    PhasePerfBackend {
        backend: String,
    },
    PhasePerfQueues,
    PhasePerfBenchmark,
    PhasePerfVerify,
    NetStatus,
    NetInit,
    NetProbe,
    NetPhy,
    NetLink,
    NetLoopback,
    NetTxTest,
    NetRxTest,
    NetWireInit,
    NetWireStatus,
    NetWirePing,
    NetWirePingDebug,
    NetWireWrap,
    NetWireRecover,
    NetWirePhyloop,
    NetWireTxBurst { count: u32 },
    NetPeer {
        count: u32,
    },
    NetIpServe {
        octets: [u8; 4],
        secs: u32,
    },
    NetIpDhcp {
        secs: u32,
    },
    NetDnsResolve {
        host: crate::platform::String,
        secs: u32,
    },
    NetSshServe {
        octets: [u8; 4],
        port: u16,
        secs: u32,
    },
    NetSshKey {
        hex: crate::platform::String,
    },
    RngStatus,
    DmaStatus,
    UsbProbe,
    UsbEnum,
    UsbList,
    UsbKeys,
    UsbKbd,
    UsbHid,
    UsbEvents,
    UsbHidNeg,
    UsbHidDetach,
    SystemSelftest,
    Fault {
        args: crate::platform::Vec<crate::platform::String>,
    },
    FsSdList,
    FsSdSave {
        name: crate::platform::String,
        data: crate::platform::String,
    },
    FsSdCat {
        name: crate::platform::String,
    },
    FsSdRemove {
        name: crate::platform::String,
    },
    PhiStabilityStatus,
    PhiStabilityRun {
        n: u64,
    },
    PhiStabilityDualClock {
        n: u64,
    },
    PhiStabilityCompare {
        n: u64,
    },
    PhiStabilityNetworkSend {
        peer: String,
        n: u64,
    },
    PhiStabilityNetworkReceive,
    PhiStabilityFault {
        fault: String,
    },
    G1pInfo,
    G1pExec {
        a: u8,
        b: u8,
    },
    G1pReflect {
        a: u8,
    },
    G1pSelftest,
    G1pConsensus,
    G1pIrqTest {
        count: u32,
    },
    G64Info,
    G64Oracle,
    G64Atlas,
    G64ComposeSweep,
    G64ReflectSweep,
    G64Benchmark,
    #[cfg(feature = "phaseguard-research")]
    PhaseGuardStatus,
    #[cfg(feature = "phaseguard-research")]
    PhaseGuardTrace,
    #[cfg(feature = "phaseguard-research")]
    PhaseGuardSample,
    PhaseBenchmarkStatus,
    PhaseBenchmarkRun {
        n: u64,
    },
    PhaseBenchmarkCompare {
        n: u64,
    },
    PhaseBenchmarkSweep {
        start: u64,
        end: u64,
    },
    PhaseBenchmarkRepeat {
        n: u64,
        count: u32,
    },
    PhaseBenchmarkScale {
        n: u64,
        workers: usize,
    },
    PhaseBenchmarkDualClock {
        n: u64,
    },
    PhaseBenchmarkNetworkCompare {
        peer: String,
        n: u64,
    },
    PhaseBenchmarkReport,
    AiConnect {
        host: String,
        port: u16,
    },
    AiDisconnect,
    AiStatus,
    AiAsk {
        prompt: String,
    },
    AiAskBm {
        octets: [u8; 4],
        port: u16,
        prompt: String,
    },
    AiJobs,
    AiShow {
        job_id: u64,
    },
    AiTrace {
        job_id: u64,
    },
    AiCancel {
        job_id: u64,
    },
    AiTools,
    AiAllow {
        tool: String,
    },
    AiDeny {
        tool: String,
    },
    AiServe {
        secs: u32,
    },
    SshStatus,
    SshStart,
    SshStop,
    SshRestart,
    SshSessions,
    SshSessionShow {
        id: u64,
    },
    SshSessionClose {
        id: u64,
    },
    SshConfig,
    SshKeyList,
    SshKeyAdd {
        public_key: String,
    },
    SshKeyRemove {
        fingerprint: String,
    },
    SshKeyEnable {
        fingerprint: String,
    },
    SshKeyDisable {
        fingerprint: String,
    },
    SshHostKeyFingerprint,
    SshHostKeyStatus,
    SshHostKeyRegenerate,
    SshAudit,
    Exit,
    Usage(String),
    Unknown(String),
}

pub fn parse(line: &str) -> Command {
    let line = line.trim();
    if line.is_empty() {
        return Command::Unknown(String::new());
    }
    let mut parts = line.split_whitespace();
    let head = parts.next().unwrap_or("").to_ascii_lowercase();
    match head.as_str() {
        "help" | "?" => Command::Help,
        "about" => Command::About,
        "clear" => Command::Clear,
        "version" => Command::Version,
        "state" => parse_state(&mut parts),
        "phasenet" => parse_phasenet(&mut parts),
        "phaselink" => parse_phaselink(&mut parts),
        "phaseperf" => parse_phaseperf(&mut parts),
        "net" => parse_net(&mut parts),
        "rng" => Command::RngStatus,
        "dma" => Command::DmaStatus,
        "usb" => match parts.next() {
            None | Some("probe") | Some("status") => Command::UsbProbe,
            Some("enum") => Command::UsbEnum,
            Some("list") => Command::UsbList,
            Some("keys") => Command::UsbKeys,
            Some("kbd") => Command::UsbKbd,
            Some("hid") => Command::UsbHid,
            Some("events") => Command::UsbEvents,
            Some("hid-neg") => Command::UsbHidNeg,
            Some("hid-detach") => Command::UsbHidDetach,
            _ => Command::Usage(
                "usage: usb probe | usb enum | usb list | usb keys | usb kbd | usb hid | usb events | usb hid-neg | usb hid-detach".into(),
            ),
        },
        "fs" => parse_fs(&mut parts),
        "phi-stability" => parse_phi_stability(&mut parts),
        "g1p" => parse_g1p(&mut parts),
        "g64" => parse_g64(&mut parts),
        #[cfg(feature = "phaseguard-research")]
        "phaseguard" => parse_phaseguard(&mut parts),
        "phase-benchmark" => parse_phase_benchmark(&mut parts),
        "ai" => parse_ai(&mut parts),
        "ssh" => parse_ssh(&mut parts),
        "system" => match parts.next() {
            None | Some("selftest") | Some("self_test") | Some("self-test") => {
                Command::SystemSelftest
            }
            _ => Command::Usage("usage: system selftest".into()),
        },
        "selftest" | "self_test" | "self-test" => Command::SystemSelftest,
        "fault" => Command::Fault {
            args: parts.map(|s| s.into()).collect(),
        },
        "clock" => match parts.next() {
            None => Command::Clock,
            Some("skew") => Command::ClockSkew,
            Some("set-dilation") => {
                let value = parts
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(f64::NAN);
                if !value.is_finite() {
                    Command::Usage("usage: clock set-dilation <value>".into())
                } else {
                    Command::ClockSetDilation { value }
                }
            }
            Some("resync") => Command::ClockResync,
            Some(_) => Command::Usage("usage: clock [skew|set-dilation <v>|resync]".into()),
        },
        "regs" => Command::Regs,
        "ps" => Command::Ps,
        "whoami" => Command::Whoami,
        "host" => Command::Host,
        "spawn" => {
            let name = parts.next().unwrap_or("").to_string();
            if name.is_empty() {
                Command::Usage("usage: spawn <name>".into())
            } else {
                Command::Spawn { name }
            }
        }
        "trace" => Command::Trace,
        "isa" => Command::Isa,
        "dump" => Command::Dump,
        "mem" => Command::Mem,
        "log" => Command::Log,
        "dashboard" => Command::Dashboard,
        "top" => {
            let mut frames = 1u32;
            let rest: Vec<&str> = parts.collect();
            let mut i = 0;
            while i < rest.len() {
                if rest[i] == "-n" && i + 1 < rest.len() {
                    frames = rest[i + 1].parse().unwrap_or(1);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            Command::Top { frames }
        }
        "watchdog" => Command::Watchdog,
        "phase" => Command::Phase,
        "topo" => Command::Topo,
        "curvature" => match parts.next() {
            None => Command::Curvature,
            Some("inject") => {
                let reps = parts.next().and_then(|s| s.parse().ok()).unwrap_or(20);
                Command::CurvatureInject { reps }
            }
            Some(_) => Command::Usage("usage: curvature [inject <reps>]".into()),
        },
        "transport" => match parts.next() {
            Some("init") => {
                let size = parts.next().and_then(|s| s.parse().ok()).unwrap_or(4);
                Command::TransportInit { size }
            }
            Some("status") => Command::TransportStatus,
            Some("send") => {
                let sr = parts.next().and_then(|s| s.parse().ok());
                let sc = parts.next().and_then(|s| s.parse().ok());
                let dr = parts.next().and_then(|s| s.parse().ok());
                let dc = parts.next().and_then(|s| s.parse().ok());
                let opcode = parts.next().map(|s| s.to_string());
                let payload = parts.collect::<Vec<_>>().join(" ");
                match (sr, sc, dr, dc, opcode) {
                    (Some(sr), Some(sc), Some(dr), Some(dc), Some(opcode)) => {
                        Command::TransportSend {
                            sr,
                            sc,
                            dr,
                            dc,
                            opcode,
                            payload,
                        }
                    }
                    _ => Command::Usage(
                        "usage: transport send <sr> <sc> <dr> <dc> <opcode> <payload>".into(),
                    ),
                }
            }
            Some("tick") => {
                let n = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1);
                Command::TransportTick { n }
            }
            Some("receive") => {
                let lane = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                Command::TransportReceive { lane }
            }
            Some("routes") => Command::TransportRoutes,
            Some("trace") => match parts.next().and_then(|s| s.parse().ok()) {
                Some(packet_id) => Command::TransportTrace { packet_id },
                None => Command::Usage("usage: transport trace <packet-id>".into()),
            },
            Some("replay") => match parts.next().and_then(|s| s.parse().ok()) {
                Some(packet_id) => Command::TransportReplay { packet_id },
                None => Command::Usage("usage: transport replay <packet-id>".into()),
            },
            Some("inject-fault") => {
                let packet_id = parts.next().and_then(|s| s.parse().ok());
                let fault = parts.next().map(|s| s.to_string());
                match (packet_id, fault) {
                    (Some(packet_id), Some(fault)) => {
                        Command::TransportInjectFault { packet_id, fault }
                    }
                    _ => Command::Usage(
                        "usage: transport inject-fault <packet-id> <fault>".into(),
                    ),
                }
            }
            Some("reset") => Command::TransportReset,
            Some("demo") => Command::TransportDemo,
            _ => Command::Usage(
                "usage: transport <init|status|send|tick|receive|routes|trace|replay|inject-fault|reset|demo>"
                    .into(),
            ),
        },
        "craft" => match parts.next() {
            Some("init") => Command::CraftInit,
            Some("status") => Command::CraftStatus,
            Some("tick") => {
                let n = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1);
                Command::CraftTick { n }
            }
            Some("command") => {
                let subsystem = parts.next().unwrap_or("").to_string();
                let operation = parts.next().unwrap_or("").to_string();
                if subsystem.is_empty() || operation.is_empty() {
                    Command::Usage("usage: craft command <nav|prop|power|fdir> <operation>".into())
                } else {
                    Command::CraftCommand {
                        subsystem,
                        operation,
                    }
                }
            }
            Some("fault") => match parts.next() {
                Some("inject") => {
                    let _kind = parts.next(); // prop-stuck
                    let mut reason = String::new();
                    while let Some(tok) = parts.next() {
                        if tok == "--reason" {
                            reason = parts.collect::<Vec<_>>().join(" ");
                            break;
                        }
                    }
                    if reason.is_empty() {
                        Command::Usage(
                            "usage: craft fault inject prop-stuck --reason <text>".into(),
                        )
                    } else {
                        Command::CraftFaultInject { reason }
                    }
                }
                _ => Command::Usage("usage: craft fault inject prop-stuck --reason <text>".into()),
            },
            Some("quarantine") => match parts.next() {
                Some("status") => Command::CraftQuarantineStatus,
                _ => Command::Usage("usage: craft quarantine status".into()),
            },
            Some("episode") => match parts.next().and_then(|s| s.parse().ok()) {
                Some(episode_id) => Command::CraftEpisode { episode_id },
                None => Command::Usage("usage: craft episode <episode-id>".into()),
            },
            Some("replay") => match parts.next().and_then(|s| s.parse().ok()) {
                Some(episode_id) => Command::CraftReplay { episode_id },
                None => Command::Usage("usage: craft replay <episode-id>".into()),
            },
            _ => Command::Usage(
                "usage: craft <init|status|tick|command|fault|quarantine|episode|replay>".into(),
            ),
        },
        "scenario" => match parts.next() {
            Some("run") => {
                let name = parts.next().unwrap_or("").to_string();
                if name.is_empty() {
                    Command::Usage("usage: scenario run <name>".into())
                } else {
                    Command::ScenarioRun { name }
                }
            }
            _ => Command::Usage("usage: scenario run <name>".into()),
        },
        "root" => Command::Root,
        "audit" => Command::Audit,
        "checkpoint" => Command::Checkpoint,
        "checkpoint-auth" => match parts.next() {
            None | Some("status") => Command::CheckpointAuthStatus,
            Some("provision") => {
                if parts.next().is_some() {
                    Command::Usage(
                        "usage: checkpoint-auth provision  (secret on silent input, not argv)"
                            .into(),
                    )
                } else {
                    Command::CheckpointAuthProvision
                }
            }
            Some("seal") => Command::CheckpointAuthSeal,
            Some("verify") => Command::CheckpointAuthVerify,
            Some("hardware") => Command::CheckpointAuthHardware,
            _ => Command::Usage(
                "usage: checkpoint-auth [status|provision|seal|verify|hardware]".into(),
            ),
        },
        "phasecache" => Command::PhaseCache,
        "pmm" => match parts.next() {
            Some("selftest") => Command::PmmSelftest,
            None => Command::Pmm,
            _ => Command::Usage("usage: pmm [selftest]".into()),
        },
        "passwd" => match parts.next() {
            Some("--status") | Some("status") => Command::PasswdStatus,
            None => Command::Passwd,
            _ => Command::Usage("usage: passwd [--status]".into()),
        },
        "login" => Command::Login,
        "logout" => Command::Logout,
        "mod" => {
            let rest: Vec<&str> = parts.collect();
            if rest.first().copied() == Some("app") {
                return Command::ModApp {
                    args: rest.iter().skip(1).map(|s| (*s).into()).collect(),
                };
            }
            if rest.first().copied() == Some("cert")
                && rest.get(1).copied() == Some("developer-revoke")
            {
                let hash_hex = rest.get(2).unwrap_or(&"").to_string();
                if hash_hex.is_empty() {
                    return Command::Usage(
                        "usage: mod cert developer-revoke <cert-hash-hex>".into(),
                    );
                }
                return Command::ModCertDeveloperRevoke { hash_hex };
            }
            let mut verb: Option<String> = None;
            let mut lease = 64u64;
            let mut kill: Option<u32> = None;
            let mut rest_iter = rest.into_iter();
            while let Some(arg) = rest_iter.next() {
                match arg {
                    "-l" | "--list" => return Command::ModList,
                    "-v" | "--verbs" => return Command::ModList,
                    "-k" | "--kill" => {
                        kill = rest_iter.next().and_then(|s| s.parse().ok());
                        if kill.is_none() {
                            return Command::Usage("usage: mod -k <ticket-id>".into());
                        }
                    }
                    "--lease" => {
                        lease = rest_iter
                            .next()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(64);
                    }
                    "shell" => {
                        return Command::Usage("mod shell is not permitted".into());
                    }
                    other => verb = Some(other.into()),
                }
            }
            if let Some(ticket_id) = kill {
                return Command::ModKill { ticket_id };
            }
            match verb {
                Some(verb) => Command::Mod { verb, lease },
                None => Command::ModList,
            }
        }
        "irqreg" => Command::Irqreg,
        "sched2" => Command::Sched2,
        "extmap" => Command::Extmap,
        "txlog" => match parts.next() {
            Some("demo") => Command::TxlogDemo,
            Some("fault") => Command::TxlogFault,
            Some("recover") => Command::TxlogRecover,
            Some("stats") | None => Command::Txlog,
            _ => Command::Usage("usage: txlog [stats|demo|fault|recover]".into()),
        },
        "fsj" => match parts.next() {
            Some("recover") => Command::FsjRecover,
            Some("check") => Command::FsjCheck,
            Some("format") => match parts.next() {
                Some("CONFIRM") => Command::FsjFormat { confirm: true },
                None => Command::FsjFormat { confirm: false },
                _ => Command::Usage("usage: fsj format [CONFIRM]".into()),
            },
            Some("crash") => match parts.next() {
                Some("off") => Command::FsjCrash { n: 0 },
                Some(raw) => match raw.parse::<u32>() {
                    Ok(n) => Command::FsjCrash { n },
                    Err(_) => Command::Usage("usage: fsj crash <0-11>|off".into()),
                },
                None => Command::Usage("usage: fsj crash <0-11>|off".into()),
            },
            Some("p6") => match parts.next() {
                Some("CONFIRM") => Command::FsjP6 { confirm: true },
                None => Command::FsjP6 { confirm: false },
                _ => Command::Usage("usage: fsj p6 CONFIRM".into()),
            },
            Some("p7") => match (parts.next(), parts.next()) {
                (Some(run), Some(tok)) => match run.parse::<u64>() {
                    Ok(run_id) => Command::FsjP7 {
                        run_id,
                        token_ok: tok == "PHASEFS-P7-CORRUPT-TAIL",
                    },
                    Err(_) => Command::Usage("usage: fsj p7 <run_id> PHASEFS-P7-CORRUPT-TAIL".into()),
                },
                _ => Command::Usage("usage: fsj p7 <run_id> PHASEFS-P7-CORRUPT-TAIL".into()),
            },
            Some("p8") => match (parts.next(), parts.next()) {
                (Some(run), Some(tok)) => match run.parse::<u64>() {
                    Ok(run_id) => Command::FsjP8 {
                        run_id,
                        token_ok: tok == "PHASEFS-P8-WRAP",
                    },
                    Err(_) => Command::Usage("usage: fsj p8 <run_id> PHASEFS-P8-WRAP".into()),
                },
                _ => Command::Usage("usage: fsj p8 <run_id> PHASEFS-P8-WRAP".into()),
            },
            Some("p10") => match (parts.next(), parts.next()) {
                (Some(run), Some(tok)) => match run.parse::<u64>() {
                    Ok(run_id) => Command::FsjP10 {
                        run_id,
                        token_ok: tok == "PHASEFS-P10-BEFORE-COMMIT",
                    },
                    Err(_) => Command::Usage(
                        "usage: fsj p10 <run_id> PHASEFS-P10-BEFORE-COMMIT".into(),
                    ),
                },
                _ => Command::Usage("usage: fsj p10 <run_id> PHASEFS-P10-BEFORE-COMMIT".into()),
            },
            Some("p11") => match (parts.next(), parts.next()) {
                (Some(run), Some(tok)) => match run.parse::<u64>() {
                    Ok(run_id) => Command::FsjP11 {
                        run_id,
                        token_ok: tok == "PHASEFS-P11-AFTER-COMMIT",
                    },
                    Err(_) => Command::Usage(
                        "usage: fsj p11 <run_id> PHASEFS-P11-AFTER-COMMIT".into(),
                    ),
                },
                _ => Command::Usage("usage: fsj p11 <run_id> PHASEFS-P11-AFTER-COMMIT".into()),
            },
            Some("p1reset") => Command::FsjP1Reset,
            Some("status") | None => Command::Fsj,
            _ => Command::Usage(
                "usage: fsj [status|recover|check|format [CONFIRM]|crash <n>|crash off|p1reset|p6 CONFIRM|p7 <run> PHASEFS-P7-CORRUPT-TAIL|p8 <run> PHASEFS-P8-WRAP|p10 <run> PHASEFS-P10-BEFORE-COMMIT|p11 <run> PHASEFS-P11-AFTER-COMMIT]".into(),
            ),
        },
        "netsess" => match parts.next() {
            Some("demo") => Command::NetsessDemo,
            Some("fault") => Command::NetsessFault,
            Some("stats") | None => Command::Netsess,
            _ => Command::Usage("usage: netsess [stats|demo|fault]".into()),
        },
        "sleep" => match parts.next().and_then(|arg| arg.parse::<u64>().ok()) {
            Some(ticks) => Command::Sleep { ticks },
            None => Command::Usage("usage: sleep <ticks>".into()),
        },
        "ipc" => Command::Ipc,
        "ipccreate" => match parts.next().and_then(|arg| arg.parse::<usize>().ok()) {
            Some(capacity) => Command::IpcCreate { capacity },
            None => Command::Usage("usage: ipccreate <capacity>".into()),
        },
        "ipcsend" => {
            let slot = parts.next().and_then(|s| s.parse::<u32>().ok());
            let generation = parts.next().and_then(|s| s.parse::<u32>().ok());
            let text: String = parts.collect::<Vec<&str>>().join(" ");
            match (slot, generation) {
                (Some(slot), Some(generation)) if !text.is_empty() => Command::IpcSend {
                    slot,
                    generation,
                    text,
                },
                _ => Command::Usage("usage: ipcsend <slot> <generation> <text>".into()),
            }
        }
        "ipcrecv" => {
            let slot = parts.next().and_then(|s| s.parse::<u32>().ok());
            let generation = parts.next().and_then(|s| s.parse::<u32>().ok());
            match (slot, generation) {
                (Some(slot), Some(generation)) => Command::IpcRecv { slot, generation },
                _ => Command::Usage("usage: ipcrecv <slot> <generation>".into()),
            }
        }
        "ipcclose" => {
            let slot = parts.next().and_then(|s| s.parse::<u32>().ok());
            let generation = parts.next().and_then(|s| s.parse::<u32>().ok());
            match (slot, generation) {
                (Some(slot), Some(generation)) => Command::IpcClose { slot, generation },
                _ => Command::Usage("usage: ipcclose <slot> <generation>".into()),
            }
        }
        "anom" => parse_anom(&mut parts),
        "run" => match parts.next() {
            None => Command::RunPaste,
            Some("file") => {
                let filename = parts.next().unwrap_or("").to_string();
                if filename.is_empty() {
                    Command::Usage("usage: run file <filename>".into())
                } else {
                    Command::RunFile { filename }
                }
            }
            Some(name) => Command::RunProgram {
                name: name.to_string(),
            },
        },
        "kill" => {
            let pid = parts
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(u32::MAX);
            if pid == u32::MAX {
                Command::Usage("usage: kill <pid>".into())
            } else {
                Command::Kill { pid }
            }
        }
        "ls" => {
            let path = parts.next().unwrap_or("/").to_string();
            Command::Ls { path }
        }
        "cat" => {
            let path = parts.next().unwrap_or("").to_string();
            if path.is_empty() {
                Command::Usage("usage: cat <path>".into())
            } else {
                Command::Cat { path }
            }
        }
        "write" => {
            let path = parts.next().unwrap_or("").to_string();
            let text = parts.collect::<Vec<_>>().join(" ");
            if path.is_empty() {
                Command::Usage("usage: write <path> <text>".into())
            } else {
                Command::Write { path, text }
            }
        }
        "mkdir" => {
            let path = parts.next().unwrap_or("").to_string();
            if path.is_empty() {
                Command::Usage("usage: mkdir <path>".into())
            } else {
                Command::Mkdir { path }
            }
        }
        "rename" => {
            let from = parts.next().unwrap_or("").to_string();
            let to = parts.next().unwrap_or("").to_string();
            if from.is_empty() || to.is_empty() {
                Command::Usage("usage: rename <from> <to>".into())
            } else {
                Command::Rename { from, to }
            }
        }
        "rm" => {
            let path = parts.next().unwrap_or("").to_string();
            if path.is_empty() {
                Command::Usage("usage: rm <path>".into())
            } else {
                Command::Rm { path }
            }
        }
        "stat" => {
            let path = parts.next().unwrap_or("").to_string();
            if path.is_empty() {
                Command::Usage("usage: stat <path>".into())
            } else {
                Command::Stat { path }
            }
        }
        "lock" => {
            let path = parts.next().unwrap_or("").to_string();
            if path.is_empty() {
                Command::Usage("usage: lock <path>".into())
            } else {
                Command::Lock { path }
            }
        }
        "unlock" => {
            let path = parts.next().unwrap_or("").to_string();
            if path.is_empty() {
                Command::Usage("usage: unlock <path>".into())
            } else {
                Command::Unlock { path }
            }
        }
        "open" => {
            let path = parts.next().unwrap_or("").to_string();
            let mode = parts.next().unwrap_or("r").to_string();
            if path.is_empty() {
                Command::Usage("usage: open <path> [r|w|rw|rwd]".into())
            } else {
                Command::Open { path, mode }
            }
        }
        "close" => {
            let slot = parts.next().and_then(|s| s.parse::<u32>().ok());
            let generation = parts.next().and_then(|s| s.parse::<u32>().ok());
            match (slot, generation) {
                (Some(slot), Some(generation)) => Command::Close { slot, generation },
                _ => Command::Usage("usage: close <slot> <generation>".into()),
            }
        }
        "captable" => Command::Captable,
        "app" => {
            let args: Vec<String> = parts.map(|s| s.into()).collect();
            if args.iter().any(|a| a == "--no-verify" || a == "trust-any-cert") {
                Command::Usage("verification cannot be disabled".into())
            } else {
                Command::App { args }
            }
        }
        "plugin" => {
            let args: Vec<String> = parts.map(|s| s.into()).collect();
            if args.iter().any(|a| a == "--no-verify" || a == "trust-any-cert") {
                Command::Usage("verification cannot be disabled".into())
            } else {
                Command::Plugin { args }
            }
        }
        "sentinel" => {
            let args: Vec<String> = parts.map(|s| s.into()).collect();
            Command::Sentinel { args }
        }
        "mmuiso" => {
            let args: Vec<String> = parts.map(|s| s.into()).collect();
            Command::MmuIso { args }
        }
        "exit" | "quit" => Command::Exit,
        _ => Command::Unknown(head),
    }
}

fn parse_phaseperf(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        None | Some("status") => Command::PhasePerfStatus,
        Some("backend") => Command::PhasePerfBackend {
            backend: parts.next().unwrap_or("").to_string(),
        },
        Some("queues") => Command::PhasePerfQueues,
        Some("benchmark") => Command::PhasePerfBenchmark,
        Some("verify") => Command::PhasePerfVerify,
        _ => Command::Usage(
            "usage: phaseperf [status|backend [scalar|avx2|avx512|neon|gpu|fpga]|queues|benchmark|verify]".into(),
        ),
    }
}

fn parse_ssh(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        None | Some("status") => Command::SshStatus,
        Some("start") => Command::SshStart,
        Some("stop") => Command::SshStop,
        Some("restart") => Command::SshRestart,
        Some("sessions") => Command::SshSessions,
        Some("session") => match parts.next() {
            Some("show") => Command::SshSessionShow {
                id: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            },
            Some("close") => Command::SshSessionClose {
                id: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            },
            _ => Command::Usage("usage: ssh session [show <id>|close <id>]".into()),
        },
        Some("config") => Command::SshConfig,
        Some("key") => match parts.next() {
            None | Some("list") => Command::SshKeyList,
            Some("add") => Command::SshKeyAdd {
                public_key: parts.collect::<Vec<_>>().join(" "),
            },
            Some("remove") => Command::SshKeyRemove {
                fingerprint: parts.next().unwrap_or("").to_string(),
            },
            Some("enable") => Command::SshKeyEnable {
                fingerprint: parts.next().unwrap_or("").to_string(),
            },
            Some("disable") => Command::SshKeyDisable {
                fingerprint: parts.next().unwrap_or("").to_string(),
            },
            _ => Command::Usage("usage: ssh key [list|add <public-key>|remove <fp>|enable <fp>|disable <fp>]".into()),
        },
        Some("host-key") => match parts.next() {
            None | Some("status") => Command::SshHostKeyStatus,
            Some("fingerprint") => Command::SshHostKeyFingerprint,
            Some("regenerate") => Command::SshHostKeyRegenerate,
            _ => Command::Usage("usage: ssh host-key [status|fingerprint|regenerate]".into()),
        },
        Some("audit") => Command::SshAudit,
        _ => Command::Usage(
            "usage: ssh [status|start|stop|restart|sessions|session show <id>|session close <id>|config|key list|key add <pub>|host-key fingerprint|audit]".into(),
        ),
    }
}

fn parse_ai(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        Some("connect") => {
            let host = parts.next().unwrap_or("").to_string();
            let port = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            if host.is_empty() || port == 0 {
                Command::Usage("usage: ai connect <host> <port>".into())
            } else {
                Command::AiConnect { host, port }
            }
        }
        Some("disconnect") => Command::AiDisconnect,
        Some("status") => Command::AiStatus,
        Some("serve") => {
            let secs = parts.next().and_then(|s| s.parse().ok()).unwrap_or(60);
            Command::AiServe { secs }
        },
        Some("ask") => parse_ai_ask(parts),
        Some("remote") => parse_ai_remote(parts),
        Some("jobs") => Command::AiJobs,
        Some("show") => {
            let job_id = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            if job_id == 0 {
                Command::Usage("usage: ai show <job_id>".into())
            } else {
                Command::AiShow { job_id }
            }
        }
        Some("trace") => {
            let job_id = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            if job_id == 0 {
                Command::Usage("usage: ai trace <job_id>".into())
            } else {
                Command::AiTrace { job_id }
            }
        }
        Some("cancel") => {
            let job_id = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            if job_id == 0 {
                Command::Usage("usage: ai cancel <job_id>".into())
            } else {
                Command::AiCancel { job_id }
            }
        }
        Some("tools") => Command::AiTools,
        Some("allow") => {
            let tool = parts.next().unwrap_or("").to_string();
            if tool.is_empty() {
                Command::Usage("usage: ai allow <tool_name>".into())
            } else {
                Command::AiAllow { tool }
            }
        }
        Some("deny") => {
            let tool = parts.next().unwrap_or("").to_string();
            if tool.is_empty() {
                Command::Usage("usage: ai deny <tool_name>".into())
            } else {
                Command::AiDeny { tool }
            }
        }
        _ => Command::Usage(
            "usage: ai [serve [seconds]|status|ask ...|jobs|show <id>|trace <id>|cancel <id>|tools|allow <tool>|deny <tool>]".into(),
        ),
    }
}

fn parse_ai_ask(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    #[cfg(feature = "std")]
    {
        let prompt = parts.collect::<Vec<_>>().join(" ");
        if prompt.is_empty() {
            Command::Usage("usage: ai ask <prompt>".into())
        } else {
            Command::AiAsk { prompt }
        }
    }
    #[cfg(not(feature = "std"))]
    {
        parse_ai_remote(parts)
    }
}

fn parse_ai_remote(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    let ip = parts.next().unwrap_or("");
    let port = parts.next().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
    let prompt = parts.collect::<Vec<_>>().join(" ");
    match parse_ipv4(ip) {
        Some(octets) if port != 0 && !prompt.is_empty() => Command::AiAskBm {
            octets,
            port,
            prompt,
        },
        _ => Command::Usage(
            "usage: ai ask <a.b.c.d> <port> <prompt>  (GEM TCP client_bm; no on-device LLM)"
                .into(),
        ),
    }
}

fn parse_ipv4(s: &str) -> Option<[u8; 4]> {
    let mut it = s.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next()?.parse().ok()?;
    let d = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some([a, b, c, d])
}

fn parse_fs(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        None | Some("ls") | Some("list") => Command::FsSdList,
        Some("save") | Some("write") => match parts.next() {
            Some(name) => {
                let mut data = crate::platform::String::new();
                for (i, t) in parts.by_ref().enumerate() {
                    if i > 0 {
                        data.push(' ');
                    }
                    data.push_str(t);
                }
                Command::FsSdSave {
                    name: name.into(),
                    data,
                }
            }
            None => Command::Usage("usage: fs save <name> <text>".into()),
        },
        Some("cat") | Some("load") | Some("read") => match parts.next() {
            Some(name) => Command::FsSdCat { name: name.into() },
            None => Command::Usage("usage: fs cat <name>".into()),
        },
        Some("rm") | Some("del") | Some("delete") => match parts.next() {
            Some(name) => Command::FsSdRemove { name: name.into() },
            None => Command::Usage("usage: fs rm <name>".into()),
        },
        _ => Command::Usage(
            "usage: fs ls | fs save <name> <text> | fs cat <name> | fs rm <name>".into(),
        ),
    }
}

fn parse_net(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        None | Some("status") => Command::NetStatus,
        Some("init") | Some("ladder") => Command::NetInit,
        Some("probe") => Command::NetProbe,
        Some("phy") => Command::NetPhy,
        Some("link") => Command::NetLink,
        Some("ip") => match parts.next() {
            Some("dhcp") => {
                let secs = parts.next().and_then(|s| s.parse().ok()).unwrap_or(30);
                Command::NetIpDhcp { secs }
            }
            Some(tok) => match parse_ipv4(tok) {
                Some(octets) => {
                    let secs = parts.next().and_then(|s| s.parse().ok()).unwrap_or(30);
                    Command::NetIpServe { octets, secs }
                }
                None => Command::Usage(
                    "usage: net ip <a.b.c.d> [seconds] | net ip dhcp [seconds]".into(),
                ),
            },
            None => {
                Command::Usage("usage: net ip <a.b.c.d> [seconds] | net ip dhcp [seconds]".into())
            }
        },
        Some("dns") => match parts.next() {
            Some(host) => {
                let secs = parts.next().and_then(|s| s.parse().ok()).unwrap_or(30);
                Command::NetDnsResolve {
                    host: host.into(),
                    secs,
                }
            }
            None => Command::Usage("usage: net dns <hostname> [seconds]".into()),
        },
        Some("ssh") => match parts.next() {
            Some("serve") => {
                let port = parts.next().and_then(|s| s.parse().ok()).unwrap_or(2222);
                let secs = parts.next().and_then(|s| s.parse().ok()).unwrap_or(60);
                Command::NetSshServe {
                    octets: [192, 168, 5, 50],
                    port,
                    secs,
                }
            }
            Some("key") => Command::NetSshKey {
                hex: parts.next().unwrap_or("").into(),
            },
            _ => Command::Usage("usage: net ssh serve [port] [seconds] | net ssh key <64hex>".into()),
        },
        Some("loopback") => Command::NetLoopback,
        Some("tx-test") => Command::NetTxTest,
        Some("rx-test") => Command::NetRxTest,
        Some("wire") => match parts.next() {
            Some("init") => Command::NetWireInit,
            Some("status") => Command::NetWireStatus,
            Some("ping") => match parts.next() {
                Some("debug") => Command::NetWirePingDebug,
                None => Command::NetWirePing,
                Some(_) => Command::Usage("usage: net wire ping [debug]".into()),
            },
            Some("wrap") => Command::NetWireWrap,
            Some("recover") => Command::NetWireRecover,
            Some("phyloop") => Command::NetWirePhyloop,
            Some("txburst") => {
                let count = parts
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(32);
                Command::NetWireTxBurst { count }
            }
            _ => Command::Usage(
                "usage: net wire [init|status|ping [debug]|wrap|recover|phyloop|txburst [n]]"
                    .into(),
            ),
        },
        Some("peer") => {
            let count = parts
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8);
            Command::NetPeer { count }
        }
        _ => Command::Usage(
            "usage: net [status|init|ladder|probe|phy|link|loopback|tx-test|rx-test|wire ...|peer [n]]".into(),
        ),
    }
}

fn parse_phase_benchmark(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        None | Some("status") => Command::PhaseBenchmarkStatus,
        Some("run") => Command::PhaseBenchmarkRun {
            n: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        },
        Some("compare") => Command::PhaseBenchmarkCompare {
            n: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        },
        Some("sweep") => Command::PhaseBenchmarkSweep {
            start: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            end: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        },
        Some("repeat") => Command::PhaseBenchmarkRepeat {
            n: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            count: parts.next().and_then(|s| s.parse().ok()).unwrap_or(20),
        },
        Some("scale") => Command::PhaseBenchmarkScale {
            n: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            workers: parts.next().and_then(|s| s.parse().ok()).unwrap_or(1),
        },
        Some("dual-clock") => Command::PhaseBenchmarkDualClock {
            n: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        },
        Some("network-compare") => Command::PhaseBenchmarkNetworkCompare {
            peer: parts.next().unwrap_or("").to_string(),
            n: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        },
        Some("report") => Command::PhaseBenchmarkReport,
        _ => Command::Usage(
            "usage: phase-benchmark [status|run <n>|compare <n>|sweep <start> <end>|repeat <n> <count>|scale <n> <workers>|dual-clock <n>|network-compare <node> <n>|report]".into(),
        ),
    }
}

fn parse_phi_stability(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        None | Some("status") => Command::PhiStabilityStatus,
        Some("run") => Command::PhiStabilityRun {
            n: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        },
        Some("dual-clock") => Command::PhiStabilityDualClock {
            n: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        },
        Some("compare") => Command::PhiStabilityCompare {
            n: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        },
        Some("network-send") => Command::PhiStabilityNetworkSend {
            peer: parts.next().unwrap_or("").to_string(),
            n: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        },
        Some("network-receive") => Command::PhiStabilityNetworkReceive,
        Some("fault") => Command::PhiStabilityFault {
            fault: parts.next().unwrap_or("").to_string(),
        },
        _ => Command::Usage(
            "usage: phi-stability [status|run <n>|dual-clock <n>|compare <n>|network-send <node> <n>|network-receive|fault <type>]".into(),
        ),
    }
}

fn parse_g1p(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        None | Some("info") => Command::G1pInfo,
        Some("exec") => {
            let a = parts.next().and_then(|s| s.parse().ok());
            let b = parts.next().and_then(|s| s.parse().ok());
            match (a, b) {
                (Some(a), Some(b)) if a <= 3 && b <= 3 => Command::G1pExec { a, b },
                _ => Command::Usage("usage: g1p exec <a:0..3> <b:0..3>".into()),
            }
        }
        Some("reflect") => match parts.next().and_then(|s| s.parse().ok()) {
            Some(a) if a <= 3 => Command::G1pReflect { a },
            _ => Command::Usage("usage: g1p reflect <a:0..3>".into()),
        },
        Some("selftest") => Command::G1pSelftest,
        Some("consensus") => Command::G1pConsensus,
        Some("irq-test") => Command::G1pIrqTest {
            count: parts.next().and_then(|s| s.parse().ok()).unwrap_or(1),
        },
        Some(_) => Command::Usage(
            "usage: g1p [info|exec <a> <b>|reflect <a>|selftest|consensus|irq-test <count>]".into(),
        ),
    }
}

fn parse_g64(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        None | Some("info") => Command::G64Info,
        Some("oracle") => Command::G64Oracle,
        Some("atlas") => Command::G64Atlas,
        Some("compose") => Command::G64ComposeSweep,
        Some("reflect") => Command::G64ReflectSweep,
        Some("benchmark") => Command::G64Benchmark,
        Some(_) => Command::Usage(
            "usage: g64 [info|oracle|atlas|compose|reflect|benchmark]".into(),
        ),
    }
}

#[cfg(feature = "phaseguard-research")]
fn parse_phaseguard(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        None | Some("status") => Command::PhaseGuardStatus,
        Some("trace") => Command::PhaseGuardTrace,
        Some("sample") => Command::PhaseGuardSample,
        Some(_) => Command::Usage("usage: phaseguard [status|trace|sample]".into()),
    }
}

fn parse_state(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        None => Command::State,
        Some("mode") => match parts.next() {
            None => Command::StateModeQuery,
            Some(mode) => Command::StateModeSet {
                mode: mode.to_string(),
            },
        },
        Some("domains") => Command::StateDomains,
        Some("map") => Command::StateMap {
            domain: parts.next().unwrap_or("").to_string(),
        },
        Some("view") => Command::StateView {
            object: parts.next().unwrap_or("").to_string(),
        },
        Some("change") => Command::StateDomainChange {
            object: parts.next().unwrap_or("").to_string(),
            label: parts.next().unwrap_or("").to_string(),
        },
        Some("proof") => Command::StateProof {
            object: parts.next().unwrap_or("").to_string(),
        },
        _ => Command::State,
    }
}

fn parse_phasenet(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        None | Some("status") => Command::PhaseNetStatus,
        Some("stream") => match parts.next() {
            Some("encode") => Command::PhaseNetStreamEncode {
                text: parts.collect::<Vec<_>>().join(" "),
            },
            Some("decode") => Command::PhaseNetStreamDecode {
                hex: parts.next().unwrap_or("").to_string(),
            },
            _ => Command::PhaseNetStatus,
        },
        Some("addr") => Command::PhaseNetAddr {
            uri: parts.next().unwrap_or("").to_string(),
        },
        Some("view") => Command::PhaseNetView {
            uri: parts.next().unwrap_or("").to_string(),
        },
        Some("loopback") => match parts.next() {
            Some("send") => Command::PhaseNetLoopbackSend {
                uri: parts.next().unwrap_or("").to_string(),
                operation: parts.next().unwrap_or("").to_string(),
                old_phi: parts.next().unwrap_or("").to_string(),
                new_phi: parts.next().unwrap_or("").to_string(),
            },
            Some("recv") => Command::PhaseNetLoopbackRecv,
            _ => Command::PhaseNetStatus,
        },
        Some("sim") => match parts.next() {
            Some("init") => Command::PhaseNetSimInit,
            Some("send") => Command::PhaseNetSimSend {
                from: parts.next().unwrap_or("").to_string(),
                to: parts.next().unwrap_or("").to_string(),
                uri: parts.next().unwrap_or("").to_string(),
                operation: parts.next().unwrap_or("").to_string(),
                old_phi: parts.next().unwrap_or("").to_string(),
                new_phi: parts.next().unwrap_or("").to_string(),
            },
            Some("recv") => Command::PhaseNetSimRecv {
                node: parts.next().unwrap_or("").to_string(),
            },
            _ => Command::PhaseNetStatus,
        },
        _ => Command::PhaseNetStatus,
    }
}

fn parse_anom(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    let has_passphrase = |args: &[&str]| args.iter().any(|p| *p == "-p" || *p == "--passphrase");
    match parts.next() {
        Some("security") => {
            let args: Vec<&str> = parts.collect();
            Command::AnomSecurity {
                passphrase: has_passphrase(&args),
            }
        }
        Some("status") => {
            let args: Vec<&str> = parts.collect();
            if let Some(obj) = args.first() {
                Command::AnomObjectStatus {
                    object: (*obj).to_string(),
                }
            } else {
                Command::AnomStatus
            }
        }
        Some("off") => {
            let args: Vec<&str> = parts.collect();
            Command::AnomOff {
                passphrase: has_passphrase(&args),
            }
        }
        Some("verify") => Command::AnomVerify,
        Some("protect") => {
            let args: Vec<&str> = parts.collect();
            let object = args.first().copied().unwrap_or("").to_string();
            Command::AnomProtect {
                object,
                passphrase: has_passphrase(&args),
            }
        }
        Some("open") => {
            let args: Vec<&str> = parts.collect();
            let object = args.first().copied().unwrap_or("").to_string();
            Command::AnomOpen {
                object,
                passphrase: has_passphrase(&args),
            }
        }
        Some("close") => {
            let args: Vec<&str> = parts.collect();
            let object = args.first().copied().unwrap_or("").to_string();
            Command::AnomClose { object }
        }
        Some("rekey") => {
            let args: Vec<&str> = parts.collect();
            let object = args.first().copied().unwrap_or("").to_string();
            Command::AnomRekey {
                object,
                passphrase: has_passphrase(&args),
            }
        }
        Some("unprotect") => {
            let args: Vec<&str> = parts.collect();
            let object = args.first().copied().unwrap_or("").to_string();
            Command::AnomUnprotect {
                object,
                passphrase: has_passphrase(&args),
            }
        }
        Some("list") => Command::AnomList,
        Some("seal-test") => {
            let args: Vec<&str> = parts.collect();
            let phase = args.first().copied().unwrap_or("").to_string();
            Command::AnomSealTest {
                phase,
                passphrase: has_passphrase(&args),
            }
        }
        _ => Command::Usage(
            "usage: anom [security -p|status [<id>]|off -p|verify|protect <id> -p|open <id> -p|close <id>|rekey <id> -p|unprotect <id> -p|list|seal-test <Phi> -p]".into(),
        ),
    }
}

#[cfg(all(feature = "baremetal", not(feature = "std")))]
fn parse_phaselink(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    parse_phaselink_baremetal(parts)
}

#[cfg(feature = "std")]
fn parse_phaselink(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    parse_phaselink_hosted(parts)
}

#[cfg(all(feature = "baremetal", not(feature = "std")))]
fn parse_phaselink_baremetal(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    match parts.next() {
        None | Some("status") => Command::PhaseLinkStatus,
        Some("init") => Command::PhaseLinkBaremetalInit {
            local: parts.next().unwrap_or("").to_string(),
            peer: parts.next().unwrap_or("").to_string(),
        },
        Some("send") => Command::PhaseLinkBaremetalSend {
            peer: parts.next().unwrap_or("").to_string(),
            text: parts.collect::<Vec<_>>().join(" "),
        },
        Some("recv") => Command::PhaseLinkBaremetalRecv,
        Some("replay") => Command::PhaseLinkBaremetalReplay,
        Some("bridge") => Command::PhaseLinkBaremetalBridge {
            text: parts.collect::<Vec<_>>().join(" "),
        },
        _ => Command::Usage(
            "usage: phaselink [status|init <local> <peer>|send <peer> <text>|recv|replay|bridge <text>]".into(),
        ),
    }
}

#[cfg(feature = "std")]
fn parse_phaselink_hosted(parts: &mut core::str::SplitWhitespace<'_>) -> Command {
    let has_passphrase = |args: &[&str]| args.iter().any(|p| *p == "-p" || *p == "--passphrase");
    match parts.next() {
        None | Some("status") => Command::PhaseLinkStatus,
        Some("encode") => Command::PhaseLinkEncode {
            text: parts.collect::<Vec<_>>().join(" "),
        },
        Some("decode") => Command::PhaseLinkDecode {
            symbols: parts.map(|s| s.to_string()).collect(),
        },
        Some("loopback") => Command::PhaseLinkLoopback {
            text: parts.collect::<Vec<_>>().join(" "),
        },
        Some("chain") => Command::PhaseLinkChain,
        Some("root") => Command::PhaseLinkRoot,
        Some("quarantine") => Command::PhaseLinkQuarantine,
        Some("verify") => Command::PhaseLinkVerify {
            frame_id: parts.next().unwrap_or("").to_string(),
        },
        Some("sim") => match parts.next() {
            Some("init") => Command::PhaseLinkSimInit,
            Some("status") => Command::PhaseLinkSimStatus,
            Some("send") => Command::PhaseLinkSimSend {
                source: parts.next().unwrap_or("").to_string(),
                destination: parts.next().unwrap_or("").to_string(),
                text: parts.collect::<Vec<_>>().join(" "),
            },
            Some("replay") => Command::PhaseLinkSimReplay,
            Some("tamper") => Command::PhaseLinkSimTamper {
                field: parts.next().unwrap_or("").to_string(),
            },
            Some("reboot") => Command::PhaseLinkSimReboot {
                node: parts.next().unwrap_or("").to_string(),
            },
            Some("reset") => Command::PhaseLinkSimReset,
            _ => Command::PhaseLinkStatus,
        },
        Some("gateway") => match parts.next() {
            None | Some("status") => Command::PhaseLinkGatewayStatus,
            Some("enable") => {
                let args: Vec<&str> = parts.collect();
                Command::PhaseLinkGatewayEnable {
                    passphrase: has_passphrase(&args),
                }
            }
            Some("disable") => Command::PhaseLinkGatewayDisable,
            _ => Command::PhaseLinkGatewayStatus,
        },
        _ => Command::Usage(
            "usage: phaselink [status|encode <text>|decode <symbols>|loopback <text>|chain|root|quarantine|verify <id>|sim <init|status|send <src> <dst> <text>|replay|tamper <field>|reboot <node>|reset>|gateway <status|enable -p|disable>]".into(),
        ),
    }
}
