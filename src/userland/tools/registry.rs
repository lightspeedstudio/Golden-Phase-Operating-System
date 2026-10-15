//! Central engineering tool command registry.

use crate::userland::tools::prelude::*;
use crate::userland::tools::types::{
    AuthorityRequirement, CommandDescriptor, CommandGroup, TargetSupport, ToolHandler,
};

macro_rules! cmd {
    ($name:expr, $aliases:expr, $group:expr, $usage:expr, $desc:expr, $auth:expr, $target:expr, $handler:expr) => {
        CommandDescriptor {
            name: $name,
            aliases: $aliases,
            group: $group,
            usage: $usage,
            description: $desc,
            authority: $auth,
            target_support: $target,
            handler: $handler,
        }
    };
}

static REGISTRY: &[CommandDescriptor] = &[
    // shell
    cmd!(
        "help",
        &["help"],
        CommandGroup::Shell,
        "help [group|command]",
        "list commands",
        AuthorityRequirement::Public,
        TargetSupport::Both,
        super::shell::help
    ),
    cmd!(
        "clear",
        &["clear"],
        CommandGroup::Shell,
        "clear",
        "clear screen",
        AuthorityRequirement::Public,
        TargetSupport::Both,
        super::shell::clear
    ),
    cmd!(
        "history",
        &["history"],
        CommandGroup::Shell,
        "history [clear|count]",
        "command history",
        AuthorityRequirement::Public,
        TargetSupport::Both,
        super::shell::history
    ),
    cmd!(
        "echo",
        &["echo"],
        CommandGroup::Shell,
        "echo <text>",
        "print text",
        AuthorityRequirement::Public,
        TargetSupport::Both,
        super::shell::echo
    ),
    cmd!(
        "which",
        &["which"],
        CommandGroup::Shell,
        "which <cmd>",
        "resolve command",
        AuthorityRequirement::Public,
        TargetSupport::Both,
        super::shell::which
    ),
    cmd!(
        "version",
        &["version"],
        CommandGroup::Shell,
        "version",
        "build info",
        AuthorityRequirement::Public,
        TargetSupport::Both,
        super::shell::version_cmd
    ),
    cmd!(
        "exit",
        &["exit", "quit"],
        CommandGroup::Shell,
        "exit",
        "exit shell",
        AuthorityRequirement::Public,
        TargetSupport::Both,
        super::shell::exit_cmd
    ),
    // file
    cmd!(
        "pwd",
        &["pwd"],
        CommandGroup::File,
        "pwd",
        "print cwd",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::file::pwd
    ),
    cmd!(
        "cd",
        &["cd"],
        CommandGroup::File,
        "cd <path>",
        "change directory",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::file::cd
    ),
    cmd!(
        "ls",
        &["ls"],
        CommandGroup::File,
        "ls [-l] [-a] [path]",
        "list directory",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::file::ls
    ),
    cmd!(
        "tree",
        &["tree"],
        CommandGroup::File,
        "tree [path] [--depth n]",
        "tree view",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::file::tree
    ),
    cmd!(
        "mkdir",
        &["mkdir"],
        CommandGroup::File,
        "mkdir [-p] <path>",
        "create directory",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::file::mkdir
    ),
    cmd!(
        "touch",
        &["touch"],
        CommandGroup::File,
        "touch <file>",
        "create or update file",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::file::touch
    ),
    cmd!(
        "cp",
        &["cp"],
        CommandGroup::File,
        "cp <src> <dst>",
        "copy file",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::file::cp
    ),
    cmd!(
        "mv",
        &["mv"],
        CommandGroup::File,
        "mv <src> <dst>",
        "move file",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::file::mv
    ),
    cmd!(
        "rm",
        &["rm"],
        CommandGroup::File,
        "rm <path>",
        "remove file",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::file::rm
    ),
    cmd!(
        "cat",
        &["cat"],
        CommandGroup::File,
        "cat <file>",
        "print file",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::file::cat
    ),
    cmd!(
        "head",
        &["head"],
        CommandGroup::File,
        "head <file> [n]",
        "first lines",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::file::head
    ),
    cmd!(
        "tail",
        &["tail"],
        CommandGroup::File,
        "tail <file> [n]",
        "last lines",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::file::tail
    ),
    cmd!(
        "stat",
        &["stat"],
        CommandGroup::File,
        "stat <path>",
        "file metadata",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::file::stat
    ),
    cmd!(
        "find",
        &["find"],
        CommandGroup::File,
        "find <path> <pattern>",
        "find by name",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::file::find
    ),
    cmd!(
        "fsj status",
        &["fsj"],
        CommandGroup::File,
        "fsj status",
        "PhaseFS journal status",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::file::fsj_status
    ),
    cmd!(
        "fsj check",
        &[],
        CommandGroup::File,
        "fsj check",
        "PhaseFS journal invariant check",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::file::fsj_check
    ),
    cmd!(
        "fsj format",
        &[],
        CommandGroup::File,
        "fsj format [CONFIRM]",
        "format journal partition 1 (requires CONFIRM)",
        AuthorityRequirement::Root,
        TargetSupport::Both,
        super::file::fsj_format
    ),
    cmd!(
        "fsj recover",
        &[],
        CommandGroup::File,
        "fsj recover",
        "replay or isolate the PhaseFS journal",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::file::fsj_recover
    ),
    cmd!(
        "fsj crash",
        &[],
        CommandGroup::File,
        "fsj crash <point>|off",
        "arm a journal crash point or fsj crash off",
        AuthorityRequirement::Root,
        TargetSupport::Both,
        super::file::fsj_crash
    ),
    cmd!(
        "fsj p6",
        &[],
        CommandGroup::File,
        "fsj p6 CONFIRM",
        "arm one-shot P6 recovery crash plan (research)",
        AuthorityRequirement::Root,
        TargetSupport::Both,
        super::file::fsj_p6
    ),
    cmd!(
        "fsj p7",
        &[],
        CommandGroup::File,
        "fsj p7 <run_id> PHASEFS-P7-CORRUPT-TAIL",
        "corrupt uncommitted WAL tail then SLCR (research)",
        AuthorityRequirement::Root,
        TargetSupport::Both,
        super::file::fsj_p7
    ),
    cmd!(
        "fsj p8",
        &[],
        CommandGroup::File,
        "fsj p8 <run_id> PHASEFS-P8-WRAP",
        "fill WAL past wrap, commit wrap file, SLCR (research)",
        AuthorityRequirement::Root,
        TargetSupport::Both,
        super::file::fsj_p8
    ),
    cmd!(
        "fsj p10",
        &[],
        CommandGroup::File,
        "fsj p10 <run_id> PHASEFS-P10-BEFORE-COMMIT",
        "write uncommitted WAL then wait for power cut (research)",
        AuthorityRequirement::Root,
        TargetSupport::Both,
        super::file::fsj_p10
    ),
    cmd!(
        "fsj p11",
        &[],
        CommandGroup::File,
        "fsj p11 <run_id> PHASEFS-P11-AFTER-COMMIT",
        "commit WAL then wait for power cut (research)",
        AuthorityRequirement::Root,
        TargetSupport::Both,
        super::file::fsj_p11
    ),
    cmd!(
        "fsj p1reset",
        &[],
        CommandGroup::File,
        "fsj p1reset",
        "SLCR after committed write+mkdir+rename (research P1)",
        AuthorityRequirement::Root,
        TargetSupport::Both,
        super::file::fsj_p1reset
    ),
    // text
    cmd!(
        "edit",
        &["edit"],
        CommandGroup::Text,
        "edit <file>|subcmd",
        "line editor",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::text::edit
    ),
    cmd!(
        "grep",
        &["grep"],
        CommandGroup::Text,
        "grep <pat> <path>",
        "search text",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::text::grep
    ),
    cmd!(
        "sort",
        &["sort"],
        CommandGroup::Text,
        "sort <file>",
        "sort lines",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::text::sort
    ),
    cmd!(
        "uniq",
        &["uniq"],
        CommandGroup::Text,
        "uniq <file>",
        "unique lines",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::text::uniq
    ),
    cmd!(
        "wc",
        &["wc"],
        CommandGroup::Text,
        "wc <file>",
        "word count",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::text::wc
    ),
    cmd!(
        "diff",
        &["diff"],
        CommandGroup::Text,
        "diff <a> <b>",
        "line diff",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::text::diff
    ),
    cmd!(
        "patch",
        &["patch"],
        CommandGroup::Text,
        "patch <file> <patch>",
        "apply patch",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::text::patch
    ),
    // build
    cmd!(
        "build",
        &["build"],
        CommandGroup::Build,
        "build [--release]",
        "cargo build",
        AuthorityRequirement::Operator,
        TargetSupport::Hosted,
        super::build::build
    ),
    cmd!(
        "test",
        &["test"],
        CommandGroup::Build,
        "test [filter]",
        "cargo test",
        AuthorityRequirement::Operator,
        TargetSupport::Hosted,
        super::build::test
    ),
    cmd!(
        "check",
        &["check"],
        CommandGroup::Build,
        "check",
        "cargo check",
        AuthorityRequirement::Operator,
        TargetSupport::Hosted,
        super::build::check
    ),
    cmd!(
        "format",
        &["format"],
        CommandGroup::Build,
        "format [--check]",
        "cargo fmt",
        AuthorityRequirement::Operator,
        TargetSupport::Hosted,
        super::build::format_cmd
    ),
    cmd!(
        "lint",
        &["lint"],
        CommandGroup::Build,
        "lint",
        "cargo clippy",
        AuthorityRequirement::Operator,
        TargetSupport::Hosted,
        super::build::lint
    ),
    // process
    cmd!(
        "ps",
        &["ps"],
        CommandGroup::Process,
        "ps",
        "process list",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::process::ps
    ),
    cmd!(
        "run",
        &[],
        CommandGroup::Process,
        "run <program>",
        "run program",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::process::run_program
    ),
    cmd!(
        "kill",
        &["kill"],
        CommandGroup::Process,
        "kill <pid>",
        "terminate process",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::process::kill
    ),
    cmd!(
        "jobs",
        &["jobs"],
        CommandGroup::Process,
        "jobs",
        "background jobs",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::process::jobs
    ),
    cmd!(
        "wait",
        &["wait"],
        CommandGroup::Process,
        "wait [pid|all]",
        "wait for task",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::process::wait_cmd
    ),
    cmd!(
        "sleep",
        &["sleep"],
        CommandGroup::Process,
        "sleep <secs>",
        "delay",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::process::sleep
    ),
    // system
    cmd!(
        "status",
        &[],
        CommandGroup::System,
        "status",
        "system summary",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::system::status
    ),
    cmd!(
        "mem status",
        &["mem"],
        CommandGroup::System,
        "mem status",
        "memory status",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::system::mem
    ),
    cmd!(
        "cpu status",
        &["cpu"],
        CommandGroup::System,
        "cpu status",
        "cpu status",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::system::cpu
    ),
    cmd!(
        "uptime",
        &["uptime"],
        CommandGroup::System,
        "uptime",
        "uptime",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::system::uptime
    ),
    cmd!(
        "clock status",
        &[],
        CommandGroup::System,
        "clock status",
        "clocks",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::system::clock
    ),
    cmd!(
        "trace show",
        &["trace"],
        CommandGroup::System,
        "trace show",
        "kernel trace",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::system::trace
    ),
    cmd!(
        "log show",
        &["log"],
        CommandGroup::System,
        "log show",
        "event log",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::system::log_cmd
    ),
    cmd!(
        "watchdog status",
        &["watchdog"],
        CommandGroup::System,
        "watchdog status",
        "watchdog",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::system::watchdog
    ),
    // debug
    cmd!(
        "hexdump",
        &["hexdump"],
        CommandGroup::Debug,
        "hexdump <file>|hexdump memory <addr> [--length n]",
        "hex dump",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::debug::hexdump
    ),
    cmd!(
        "strings",
        &["strings"],
        CommandGroup::Debug,
        "strings <file>",
        "print strings",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::debug::strings
    ),
    cmd!(
        "checksum",
        &["checksum"],
        CommandGroup::Debug,
        "checksum <file>",
        "file hash",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::debug::checksum
    ),
    cmd!(
        "size",
        &["size"],
        CommandGroup::Debug,
        "size <file>",
        "file size",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::debug::size
    ),
    cmd!(
        "symbols",
        &["symbols"],
        CommandGroup::Debug,
        "symbols <file>",
        "symbols",
        AuthorityRequirement::Viewer,
        TargetSupport::Hosted,
        super::debug::symbols
    ),
    cmd!(
        "objdump headers",
        &["objdump"],
        CommandGroup::Debug,
        "objdump headers <file>",
        "objdump",
        AuthorityRequirement::Viewer,
        TargetSupport::Hosted,
        super::debug::objdump
    ),
    // device
    cmd!(
        "dev list",
        &["dev"],
        CommandGroup::Device,
        "dev list",
        "device list",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::device::dev
    ),
    cmd!(
        "irq list",
        &["irq"],
        CommandGroup::Device,
        "irq list",
        "interrupt list",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::device::irq
    ),
    cmd!(
        "mmio read",
        &["mmio"],
        CommandGroup::Device,
        "mmio read <addr> <w>",
        "mmio read",
        AuthorityRequirement::Root,
        TargetSupport::Both,
        super::device::mmio
    ),
    // network
    cmd!(
        "ping",
        &["ping"],
        CommandGroup::Network,
        "ping <mode> <target>",
        "transport ping",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::network::ping
    ),
    cmd!(
        "connect",
        &["connect"],
        CommandGroup::Network,
        "connect <t> <target>",
        "connect",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::network::connect
    ),
    cmd!(
        "listen",
        &["listen"],
        CommandGroup::Network,
        "listen <t> [ep]",
        "listen",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::network::listen
    ),
    cmd!(
        "send",
        &["send"],
        CommandGroup::Network,
        "send <t> <target> <data>",
        "send",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::network::send
    ),
    cmd!(
        "receive",
        &["receive"],
        CommandGroup::Network,
        "receive <t>",
        "receive",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::network::receive
    ),
    cmd!(
        "ssh status",
        &["ssh"],
        CommandGroup::Network,
        "ssh status",
        "ssh server",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::network::ssh
    ),
    // phase
    cmd!(
        "phase status",
        &["phase"],
        CommandGroup::Phase,
        "phase status",
        "phase status",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::phase::status
    ),
    cmd!(
        "phase compose",
        &[],
        CommandGroup::Phase,
        "phase compose <a> <b>",
        "compose",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::phase::compose_cmd
    ),
    cmd!(
        "phase reflect",
        &[],
        CommandGroup::Phase,
        "phase reflect <s>",
        "reflect",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::phase::reflect_cmd
    ),
    cmd!(
        "phase transition",
        &[],
        CommandGroup::Phase,
        "phase transition <sub>",
        "transitions",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::phase::transition
    ),
    cmd!(
        "phase topology",
        &[],
        CommandGroup::Phase,
        "phase topology",
        "topology",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::phase::topology
    ),
    cmd!(
        "phase chi",
        &[],
        CommandGroup::Phase,
        "phase chi <op>",
        "chi",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::phase::chi
    ),
    cmd!(
        "phase table",
        &[],
        CommandGroup::Phase,
        "phase table",
        "lookup table",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::phase::table
    ),
    // security
    cmd!(
        "authority status",
        &["authority"],
        CommandGroup::Security,
        "authority status",
        "authority",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::security::authority
    ),
    cmd!(
        "anom status",
        &[],
        CommandGroup::Security,
        "anom status",
        "anom",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::security::anom
    ),
    cmd!(
        "statechain status",
        &["statechain"],
        CommandGroup::Security,
        "statechain status",
        "statechain",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::security::statechain
    ),
    cmd!(
        "audit status",
        &["audit"],
        CommandGroup::Security,
        "audit status",
        "audit",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::security::audit
    ),
    cmd!(
        "quarantine list",
        &["quarantine"],
        CommandGroup::Security,
        "quarantine list",
        "quarantine",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::security::quarantine
    ),
    cmd!(
        "crypto selftest",
        &["crypto"],
        CommandGroup::Security,
        "crypto selftest",
        "AES-GCM + ed25519 self-test",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::security::crypto
    ),
    // benchmark
    cmd!(
        "bench status",
        &["bench"],
        CommandGroup::Benchmark,
        "bench status",
        "benchmarks",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::benchmark::bench
    ),
    cmd!(
        "phi-stability status",
        &["phi-stability"],
        CommandGroup::Benchmark,
        "phi-stability status",
        "PHI-STABILITY",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::benchmark::phi_stability
    ),
    cmd!(
        "phaseperf status",
        &["phaseperf"],
        CommandGroup::Benchmark,
        "phaseperf status",
        "phaseperf",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::benchmark::phaseperf
    ),
    cmd!(
        "test-system quick",
        &["test-system"],
        CommandGroup::Benchmark,
        "test-system quick",
        "system tests",
        AuthorityRequirement::Operator,
        TargetSupport::Hosted,
        super::benchmark::test_system
    ),
    // script
    cmd!(
        "script run",
        &["script"],
        CommandGroup::Script,
        "script run <file>",
        "run script",
        AuthorityRequirement::Operator,
        TargetSupport::Both,
        super::script::run
    ),
    cmd!(
        "script check",
        &[],
        CommandGroup::Script,
        "script check <file>",
        "check script",
        AuthorityRequirement::Viewer,
        TargetSupport::Both,
        super::script::check
    ),
];

pub fn registry_entries() -> &'static [CommandDescriptor] {
    REGISTRY
}

pub fn lookup_descriptor(key: &str) -> Option<&'static CommandDescriptor> {
    resolve_longest(key.trim())
}

fn resolve_longest(key: &str) -> Option<&'static CommandDescriptor> {
    for desc in REGISTRY {
        if desc.name == key {
            return Some(desc);
        }
        for alias in desc.aliases {
            if *alias == key {
                return Some(desc);
            }
        }
        let full = format!("{} {}", desc.group.label(), desc.name);
        if full == key {
            return Some(desc);
        }
    }
    None
}

pub fn resolve_command(line: &str) -> Option<(&'static CommandDescriptor, Vec<String>)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parts: Vec<String> = trimmed.split_whitespace().map(|s| s.into()).collect();
    if parts[0] == "kairos-tool" || parts[0] == "tool" {
        if parts.len() < 3 {
            return None;
        }
        let key = format!("{} {}", parts[1], parts[2]);
        let desc = lookup_descriptor(&key)?;
        return Some((desc, parts[3..].to_vec()));
    }
    for len in (1..=parts.len()).rev() {
        let key = parts[..len].join(" ");
        if let Some(desc) = lookup_descriptor(&key) {
            return Some((desc, parts[len..].to_vec()));
        }
    }
    None
}

pub fn all_names_unique() -> bool {
    use crate::platform::HashMap;
    let mut seen = HashMap::new();
    for desc in REGISTRY {
        let full = format!("{} {}", desc.group.label(), desc.name);
        if !seen.insert(full, true).is_none() {
            return false;
        }
        for alias in desc.aliases {
            if alias.is_empty() || *alias == desc.name {
                continue;
            }
            if !seen.insert((*alias).into(), true).is_none() {
                return false;
            }
        }
    }
    true
}
