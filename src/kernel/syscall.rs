//! Syscall boundary — protected entry from user processes into kernel operations.

use crate::core::gphase_core::PhaseState;
use crate::core::gphase_core::TopologyKind;
use crate::fs::{FsError, InodeId};
#[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
use crate::kernel::captable::{CapError, GenerationalHandle, RIGHT_DELETE, RIGHT_READ, RIGHT_WRITE};
use crate::kernel::event_log::{PageId, ProcessId, TopologyEvent};
#[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
use crate::kernel::extmap::ExtmapError;
#[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
use crate::kernel::ipc::IpcError;
use crate::kernel::memory::MemoryError;
#[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
use crate::kernel::sched2::Sched2Error;
#[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
use crate::kernel::txlog::TxlogError;
use crate::kernel::Kernel;
use crate::platform::{String, Vec};
use core::fmt;

pub const WRITE: u32 = 0;
pub const READ: u32 = 1;
pub const SPAWN: u32 = 2;
pub const EXIT: u32 = 3;
pub const YIELD: u32 = 4;
pub const ALLOC_PAGE: u32 = 5;
pub const FREE_PAGE: u32 = 6;
pub const PHASE_STATE: u32 = 7;
pub const TOPOLOGY_LOG: u32 = 8;
pub const CONTENTION_CHECK: u32 = 9;
pub const REFLECTION_LOG: u32 = 10;
pub const FS_OPEN: u32 = 11;
pub const FS_READ_FILE: u32 = 12;
pub const FS_WRITE_FILE: u32 = 13;
pub const FS_CLOSE: u32 = 14;
pub const FS_MKDIR: u32 = 15;
pub const FS_LS: u32 = 16;
pub const FS_RM: u32 = 17;
pub const FS_STAT: u32 = 18;
pub const FS_LOCK: u32 = 19;
pub const FS_UNLOCK: u32 = 20;
/// Block the caller (via `kernel::sched2`) until `SyscallArgs::sleep_ticks`
/// real ticks have elapsed on the caller-supplied `sched2` time base. A
/// no-op returning `Ok` when the `sched2` feature is disabled.
pub const PROC_SLEEP: u32 = 21;
/// Create a new bounded `ipc` message queue (`SyscallArgs::ipc_capacity`)
/// and mint an owning `captable` capability (`SyscallArgs::rights`) on it
/// for the caller in one call. Returns `SyscallResult::Handle`. A no-op
/// returning plain `Ok` when the `ipc` feature is disabled.
pub const IPC_CREATE_QUEUE: u32 = 22;
/// Send `SyscallArgs::text`'s bytes on the queue named by
/// `(SyscallArgs::cap_slot, SyscallArgs::cap_generation)`, after
/// authorizing `RIGHT_WRITE` on that capability. Wakes a real blocked
/// receiver, if any. Never blocks the sender (see `kernel::ipc`'s module
/// docs). A no-op returning plain `Ok` when the `ipc` feature is disabled.
pub const IPC_SEND: u32 = 23;
/// Receive the oldest message on the queue named by
/// `(SyscallArgs::cap_slot, SyscallArgs::cap_generation)`, after
/// authorizing `RIGHT_READ`. Returns `SyscallResult::Message` on success;
/// on an empty queue, blocks the caller via `kernel::sched2` and returns
/// `SyscallError::Ipc(IpcError::QueueEmpty)` (not a fatal condition — see
/// `kernel::ipc`'s module docs). A no-op returning plain `Ok` when the
/// `ipc` feature is disabled.
pub const IPC_RECV: u32 = 24;
/// Destroy the queue named by `(SyscallArgs::cap_slot,
/// SyscallArgs::cap_generation)`, after authorizing `RIGHT_DELETE`, and
/// close that capability. A no-op returning plain `Ok` when the `ipc`
/// feature is disabled.
pub const IPC_DESTROY_QUEUE: u32 = 25;
/// Read one line intended to hold a secret (e.g. a password), via
/// [`IoBuffer::read_secret_line`] — real no-echo terminal input where a
/// real terminal exists to protect (see that method's docs); otherwise
/// behaves exactly like [`READ`] (scripted/non-interactive input has no
/// terminal echo to suppress). Added for `kernel::identity`'s `passwd`
/// command (Gate 4).
pub const READ_SECRET: u32 = 26;
/// PhaseFS rename (`SyscallArgs::path` → `SyscallArgs::path_b`).
/// Journaled when `phasefs-journal` is on; `txlog` has no Rename op.
pub const FS_RENAME: u32 = 27;

#[derive(Debug, Clone, Default)]
pub struct SyscallArgs {
    pub target_pid: ProcessId,
    pub page_id: PageId,
    pub text: String,
    pub path: String,
    pub path_b: String,
    /// `FS_OPEN`: requested rights bitset (see `kernel::captable::Rights`).
    /// Ignored unless the `captable` feature is enabled.
    pub rights: u32,
    /// `FS_CLOSE`/`FS_READ_FILE`/`FS_WRITE_FILE`/`FS_MKDIR`/`FS_RM`: the
    /// capability handle, as `(slot, generation)`. Required when `captable`
    /// is enabled. Ignored otherwise.
    pub cap_slot: u32,
    pub cap_generation: u32,
    /// `PROC_SLEEP`: how many `sched2` ticks to sleep for. Ignored unless
    /// the `sched2` feature is enabled.
    pub sleep_ticks: u64,
    /// `IPC_CREATE_QUEUE`: requested ring capacity (must be a non-zero
    /// power of two, at most `kernel::ipc::MAX_QUEUE_CAPACITY`). Ignored
    /// unless the `ipc` feature is enabled.
    pub ipc_capacity: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyscallResult {
    Ok,
    Pid(ProcessId),
    PhaseState(PhaseState),
    PageId(PageId),
    Text(String),
    Bytes(Vec<u8>),
    Events(Vec<TopologyEvent>),
    /// `FS_OPEN`'s minted capability, as `(slot, generation)`. Only produced
    /// when the `captable` feature is enabled.
    Handle(u32, u32),
    /// `IPC_RECV`'s real received message: the real sender and payload
    /// bytes. Only produced when the `ipc` feature is enabled.
    Message(ProcessId, Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyscallError {
    BadSyscall,
    NotRunning,
    BadTarget,
    Kernel(&'static str),
    Memory(MemoryError),
    Fs(FsError),
    #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
    Captable(CapError),
    #[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
    Sched2(Sched2Error),
    #[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
    Ipc(IpcError),
    #[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
    Extmap(ExtmapError),
    #[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
    Txlog(TxlogError),
    #[cfg(all(
        feature = "phasefs-journal",
        any(feature = "std", feature = "baremetal")
    ))]
    Journal(crate::fs::journal::JournalError),
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    Identity(crate::kernel::identity::AuthorityError),
    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    Sentinel(crate::kernel::sentinel::SentinelError),
}

impl fmt::Display for SyscallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyscallError::BadSyscall => write!(f, "bad syscall number"),
            SyscallError::NotRunning => write!(f, "caller is not running"),
            SyscallError::BadTarget => write!(f, "bad syscall target"),
            SyscallError::Kernel(msg) => write!(f, "kernel: {msg}"),
            SyscallError::Memory(err) => write!(f, "memory: {err}"),
            SyscallError::Fs(err) => write!(f, "filesystem: {err}"),
            #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
            SyscallError::Captable(err) => write!(f, "captable: {err}"),
            #[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
            SyscallError::Sched2(err) => write!(f, "sched2: {err}"),
            #[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
            SyscallError::Ipc(err) => write!(f, "ipc: {err}"),
            #[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
            SyscallError::Extmap(err) => write!(f, "extmap: {err}"),
            #[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
            SyscallError::Txlog(err) => write!(f, "txlog: {err}"),
            #[cfg(all(
                feature = "phasefs-journal",
                any(feature = "std", feature = "baremetal")
            ))]
            SyscallError::Journal(err) => write!(f, "journal: {err}"),
            #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
            SyscallError::Identity(err) => write!(f, "identity: {err:?}"),
            #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
            SyscallError::Sentinel(err) => write!(f, "{}", err.as_str()),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SyscallError {}

/// Host-visible I/O buffer for read/write syscalls.
#[derive(Debug, Clone, Default)]
pub struct IoBuffer {
    pub output: Vec<String>,
    pub input: Vec<String>,
    input_pos: usize,
    /// When true, read from stdin and print writes immediately (hosted terminal).
    #[cfg(feature = "std")]
    interactive: bool,
    /// Platform hook for [`Self::read_secret_line`]'s real terminal path
    /// (e.g. `board::zynq7000::uart::read_line_silent` on bare-metal Zynq,
    /// which cannot be called from here directly without making `kernel`
    /// depend on `board`). See that method's docs.
    secret_reader: Option<fn() -> String>,
    /// If set, every [`Self::write_line`] is also emitted immediately.
    /// Required on bare-metal: the shell only drains `output` after a
    /// command returns, but `passwd`/`login` block inside
    /// [`Self::read_secret_line`] and would otherwise never show their
    /// prompts on UART.
    output_sink: Option<fn(&str)>,
}

impl IoBuffer {
    pub fn with_input<I, S>(lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            input: lines.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }

    #[cfg(feature = "std")]
    pub fn std_interactive() -> Self {
        Self {
            interactive: true,
            ..Self::default()
        }
    }

    #[cfg(feature = "std")]
    pub fn is_interactive(&self) -> bool {
        self.interactive
    }

    pub fn read_line(&mut self) -> Option<String> {
        if self.input_pos < self.input.len() {
            let line = self.input[self.input_pos].clone();
            self.input_pos += 1;
            return Some(line);
        }

        #[cfg(feature = "std")]
        if self.interactive {
            use std::io::{self, Write};
            let _ = io::stdout().flush();
            let mut buf = String::new();
            match io::stdin().read_line(&mut buf) {
                Ok(0) => return None,
                Ok(_) => return Some(buf.trim_end_matches(['\r', '\n']).to_string()),
                Err(_) => return None,
            }
        }

        None
    }

    /// Install a real platform-specific no-echo line reader (e.g. a
    /// bare-metal UART driver's `read_line_silent`) for
    /// [`Self::read_secret_line`] to call. See that method's docs.
    pub fn set_secret_reader(&mut self, reader: fn() -> String) {
        self.secret_reader = Some(reader);
    }

    pub fn set_output_sink(&mut self, sink: fn(&str)) {
        self.output_sink = Some(sink);
    }

    pub fn writes_live(&self) -> bool {
        self.output_sink.is_some()
    }

    /// Read one line intended to hold a secret (e.g. a password), without
    /// echoing it where a real terminal exists to protect:
    /// - Scripted/queued input (`self.input`, used by tests and non-
    ///   interactive composition) is consumed exactly like [`Self::read_line`]
    ///   — there is no real terminal there to secure, and tests need
    ///   deterministic behavior.
    /// - If a [`Self::set_secret_reader`] hook is installed (real bare-metal
    ///   hardware), it is called directly — a genuine no-echo UART read.
    /// - Otherwise, on hosted `std` interactive sessions (with the
    ///   `identity` feature, which pulls in the real termios/console
    ///   support), the real OS terminal echo is suppressed for one line via
    ///   [`secret_input::read_line_no_echo`].
    /// - Any other case (non-interactive, no reader, `identity` off) falls
    ///   back to [`Self::read_line`] — an honest degrade, not a fake secret
    ///   path: there is no real terminal control available to suppress
    ///   echo with.
    pub fn read_secret_line(&mut self) -> Option<String> {
        if self.input_pos < self.input.len() {
            return self.read_line();
        }
        if let Some(reader) = self.secret_reader {
            return Some(reader());
        }
        #[cfg(all(feature = "std", feature = "identity"))]
        {
            if self.interactive {
                return secret_input::read_line_no_echo();
            }
        }
        self.read_line()
    }

    pub fn write_line(&mut self, line: impl Into<String>) {
        let line = line.into();
        #[cfg(feature = "std")]
        if self.interactive {
            use std::io::{self, Write};
            println!("{line}");
            let _ = io::stdout().flush();
        }
        if let Some(sink) = self.output_sink {
            sink(&line);
            sink("\n");
        }
        self.output.push(line);
    }
}

fn enter_kernel(kernel: &mut Kernel, caller: ProcessId) -> Result<(), SyscallError> {
    if kernel
        .get_process(caller)
        .map_err(|_| SyscallError::BadTarget)?
        .state
        != PhaseState::Phi2
    {
        return Err(SyscallError::NotRunning);
    }
    kernel
        .syscall(caller)
        .map_err(SyscallError::Kernel)?
        .then_some(())
        .ok_or(SyscallError::Kernel("syscall transition rejected"))?;
    Ok(())
}

fn leave_kernel(kernel: &mut Kernel, caller: ProcessId) -> Result<(), SyscallError> {
    kernel
        .kreturn(caller)
        .map_err(SyscallError::Kernel)?
        .then_some(())
        .ok_or(SyscallError::Kernel("kreturn transition rejected"))?;
    Ok(())
}

fn ensure_target_kernel(kernel: &mut Kernel, target: ProcessId) -> Result<(), SyscallError> {
    let state = kernel
        .get_process(target)
        .map_err(|_| SyscallError::BadTarget)?
        .state;
    match state {
        PhaseState::Phi0 => return Err(SyscallError::BadTarget),
        PhaseState::Phi1 => {
            kernel
                .schedule(target)
                .map_err(SyscallError::Kernel)?
                .then_some(())
                .ok_or(SyscallError::Kernel("schedule rejected"))?;
        }
        PhaseState::Phi2 => {}
        PhaseState::Phi3 => {}
    }
    if kernel.get_process(target).unwrap().state == PhaseState::Phi2 {
        kernel
            .syscall(target)
            .map_err(SyscallError::Kernel)?
            .then_some(())
            .ok_or(SyscallError::Kernel("syscall rejected"))?;
    }
    Ok(())
}

#[cfg(feature = "std")]
fn syscall_name(nr: u32) -> &'static str {
    match nr {
        WRITE => "write",
        READ => "read",
        SPAWN => "spawn",
        EXIT => "exit",
        YIELD => "yield",
        ALLOC_PAGE => "alloc_page",
        FREE_PAGE => "free_page",
        PHASE_STATE => "phase_state",
        TOPOLOGY_LOG => "topology_log",
        CONTENTION_CHECK => "contention_check",
        REFLECTION_LOG => "reflection_log",
        FS_OPEN => "fs_open",
        FS_READ_FILE => "fs_read",
        FS_WRITE_FILE => "fs_write",
        FS_CLOSE => "fs_close",
        FS_MKDIR => "fs_mkdir",
        FS_LS => "fs_ls",
        FS_RM => "fs_rm",
        FS_STAT => "fs_stat",
        FS_LOCK => "fs_lock",
        FS_UNLOCK => "fs_unlock",
        FS_RENAME => "fs_rename",
        PROC_SLEEP => "proc_sleep",
        IPC_CREATE_QUEUE => "ipc_create_queue",
        IPC_SEND => "ipc_send",
        IPC_RECV => "ipc_recv",
        IPC_DESTROY_QUEUE => "ipc_destroy_queue",
        READ_SECRET => "read_secret",
        _ => "unknown",
    }
}

#[cfg(feature = "std")]
fn audit_syscall(kernel: &mut Kernel, caller: ProcessId, nr: u32, args: &SyscallArgs) {
    let name = syscall_name(nr);
    kernel.record_syscall_event(caller, nr, name);
    match nr {
        FS_OPEN | FS_READ_FILE | FS_WRITE_FILE | FS_CLOSE | FS_MKDIR | FS_LS | FS_RM | FS_STAT
        | FS_LOCK | FS_UNLOCK | FS_RENAME => {
            if !args.path.is_empty() {
                kernel.record_fs_event(&args.path, name);
            }
            if nr == FS_RENAME && !args.path_b.is_empty() {
                kernel.record_fs_event(&args.path_b, name);
            }
        }
        _ => {}
    }
}

pub fn dispatch(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    nr: u32,
    args: &SyscallArgs,
) -> Result<SyscallResult, SyscallError> {
    let phase_snapshot = if nr == PHASE_STATE {
        let pid = if args.target_pid == caller {
            caller
        } else {
            args.target_pid
        };
        Some(
            kernel
                .get_process(pid)
                .map_err(|_| SyscallError::BadTarget)?
                .state,
        )
    } else {
        None
    };

    enter_kernel(kernel, caller)?;

    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    {
        const OBSERVE: &[u32] = &[
            SPAWN,
            EXIT,
            FS_OPEN,
            FS_READ_FILE,
            FS_WRITE_FILE,
            FS_MKDIR,
            FS_RENAME,
            FS_RM,
            IPC_CREATE_QUEUE,
            IPC_SEND,
            IPC_RECV,
            IPC_DESTROY_QUEUE,
        ];
        if OBSERVE.contains(&nr) {
            if let Err(e) = kernel.sentinel_authorize_syscall(caller) {
                leave_kernel(kernel, caller)?;
                return Err(SyscallError::Sentinel(e));
            }
            kernel.sentinel_observe_syscall(caller, nr);
        }
    }

    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    if nr == FS_RM {
        if let Some(session) = kernel.identity.sessions().session_for_process(caller) {
            let now = kernel.statechain.tick();
            let boot = kernel.authority.boot_session_root;
            let allowed = kernel.identity.tickets().active().iter().any(|t| {
                t.session_id == session.session_id
                    && kernel
                        .identity
                        .require(
                            t.ticket_id,
                            crate::kernel::identity::ModOperation::FsRemove,
                            boot,
                            now,
                        )
                        .is_ok()
            });
            if !allowed {
                leave_kernel(kernel, caller)?;
                return Err(SyscallError::Identity(
                    crate::kernel::identity::AuthorityError::Ticket(
                        crate::kernel::identity::TicketError::NotFound,
                    ),
                ));
            }
        }
    }

    let result = match nr {
        WRITE => {
            io.write_line(&args.text);
            SyscallResult::Ok
        }
        READ => match io.read_line() {
            Some(line) => SyscallResult::Text(line),
            None => SyscallResult::Text(String::new()),
        },
        READ_SECRET => match io.read_secret_line() {
            Some(line) => SyscallResult::Text(line),
            None => SyscallResult::Text(String::new()),
        },
        SPAWN => {
            let pid = kernel.spawn();
            kernel.wake(pid).map_err(SyscallError::Kernel)?;
            SyscallResult::Pid(pid)
        }
        EXIT => {
            kernel
                .exit(caller)
                .map_err(SyscallError::Kernel)?
                .then_some(())
                .ok_or(SyscallError::Kernel("exit rejected"))?;
            #[cfg(feature = "std")]
            audit_syscall(kernel, caller, nr, args);
            return Ok(SyscallResult::Ok);
        }
        YIELD => {
            kernel
                .preempt(caller)
                .map_err(SyscallError::Kernel)?
                .then_some(())
                .ok_or(SyscallError::Kernel("yield rejected"))?;
            kernel
                .schedule(caller)
                .map_err(SyscallError::Kernel)?
                .then_some(())
                .ok_or(SyscallError::Kernel("reschedule rejected"))?;
            SyscallResult::Ok
        }
        ALLOC_PAGE => {
            let page = kernel.alloc_page();
            SyscallResult::PageId(page)
        }
        FREE_PAGE => {
            kernel
                .free_page(args.page_id)
                .map_err(SyscallError::Memory)?;
            SyscallResult::Ok
        }
        PHASE_STATE => SyscallResult::PhaseState(phase_snapshot.expect("phase snapshot")),
        TOPOLOGY_LOG => SyscallResult::Events(kernel.topology_report()),
        CONTENTION_CHECK => {
            let result = kernel
                .contention_check(caller, args.target_pid)
                .map_err(SyscallError::Kernel)?;
            SyscallResult::PhaseState(result)
        }
        REFLECTION_LOG => {
            let events: Vec<_> = kernel
                .topology_report()
                .into_iter()
                .filter(|e| e.kind == TopologyKind::Reflection)
                .collect();
            SyscallResult::Events(events)
        }
        FS_OPEN => open_file_capability(kernel, caller, &args.path, args.rights)?,
        FS_CLOSE => {
            close_file_capability(kernel, args.cap_slot, args.cap_generation)?;
            SyscallResult::Ok
        }
        FS_READ_FILE => {
            #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
            require_file_capability(
                kernel,
                caller,
                args.cap_slot,
                args.cap_generation,
                &args.path,
                RIGHT_READ,
                FileCapMode::Target,
            )?;
            let data = cached_read_file(kernel, &args.path).map_err(SyscallError::Fs)?;
            SyscallResult::Bytes(data)
        }
        FS_WRITE_FILE => {
            #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
            {
                let mode = if kernel.fs.inode_id_for_path(&args.path).is_ok() {
                    FileCapMode::Target
                } else {
                    FileCapMode::Parent
                };
                require_file_capability(
                    kernel,
                    caller,
                    args.cap_slot,
                    args.cap_generation,
                    &args.path,
                    RIGHT_WRITE,
                    mode,
                )?;
            }
            persist_write_file(kernel, &args.path, args.text.as_bytes())?;
            invalidate_cache_path(kernel, &args.path);
            stamp_and_index(kernel, &args.path).map_err(SyscallError::Fs)?;
            extmap_write_for_path(kernel, &args.path, args.text.as_bytes())?;
            SyscallResult::Ok
        }
        FS_MKDIR => {
            #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
            require_file_capability(
                kernel,
                caller,
                args.cap_slot,
                args.cap_generation,
                &args.path,
                RIGHT_WRITE,
                FileCapMode::Parent,
            )?;
            persist_mkdir(kernel, &args.path)?;
            invalidate_cache_path(kernel, &args.path);
            stamp_and_index(kernel, &args.path).map_err(SyscallError::Fs)?;
            SyscallResult::Ok
        }
        FS_RENAME => {
            let from_id = kernel.fs.inode_id_for_path(&args.path).ok();
            persist_rename(kernel, &args.path, &args.path_b)?;
            invalidate_cache_path(kernel, &args.path);
            invalidate_cache_path(kernel, &args.path_b);
            invalidate_cache_path_prefix(kernel, &args.path);
            if let Some(id) = from_id {
                invalidate_cache_inode(kernel, id);
            }
            stamp_and_index(kernel, &args.path_b).map_err(SyscallError::Fs)?;
            SyscallResult::Ok
        }
        FS_LS => {
            let names = kernel.fs.list_dir(&args.path).map_err(SyscallError::Fs)?;
            SyscallResult::Text(names.join("\n"))
        }
        FS_RM => {
            #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
            require_file_capability(
                kernel,
                caller,
                args.cap_slot,
                args.cap_generation,
                &args.path,
                RIGHT_DELETE,
                FileCapMode::Target,
            )?;
            let inode_id = kernel.fs.inode_id_for_path(&args.path).ok();
            persist_rm(kernel, &args.path)?;
            invalidate_cache_path(kernel, &args.path);
            if let Some(id) = inode_id {
                kernel.phase_index.remove(id);
                invalidate_cache_inode(kernel, id);
                destroy_file_capability_object(kernel, id);
                extmap_delete_inode(kernel, id)?;
            }
            SyscallResult::Ok
        }
        FS_STAT => {
            let tag = cached_stat(kernel, &args.path).map_err(SyscallError::Fs)?;
            SyscallResult::PhaseState(tag)
        }
        FS_LOCK => {
            persist_lock(kernel, &args.path)?;
            invalidate_cache_path(kernel, &args.path);
            stamp_and_index(kernel, &args.path).map_err(SyscallError::Fs)?;
            SyscallResult::Ok
        }
        FS_UNLOCK => {
            persist_unlock(kernel, &args.path)?;
            invalidate_cache_path(kernel, &args.path);
            stamp_and_index(kernel, &args.path).map_err(SyscallError::Fs)?;
            SyscallResult::Ok
        }
        PROC_SLEEP => {
            sched2_sleep(kernel, caller, args.sleep_ticks)?;
            SyscallResult::Ok
        }
        IPC_CREATE_QUEUE => ipc_create_queue(kernel, caller, args.ipc_capacity, args.rights)?,
        IPC_SEND => {
            ipc_send(
                kernel,
                caller,
                args.cap_slot,
                args.cap_generation,
                args.text.as_bytes(),
            )?;
            SyscallResult::Ok
        }
        IPC_RECV => ipc_recv(kernel, caller, args.cap_slot, args.cap_generation)?,
        IPC_DESTROY_QUEUE => {
            ipc_destroy_queue(kernel, caller, args.cap_slot, args.cap_generation)?;
            SyscallResult::Ok
        }
        _ => return Err(SyscallError::BadSyscall),
    };

    #[cfg(feature = "std")]
    audit_syscall(kernel, caller, nr, args);

    leave_kernel(kernel, caller)?;
    Ok(result)
}

/// Stamp `path`'s inode with the live `StateClock` (Chronos tick +
/// Kairos-sector) and (re-)place it in `kernel.phase_index` under its
/// current phase tag. Called after every `PhaseFs` create/mutate so
/// `PhaseIndex` never drifts from `fs`.
fn stamp_and_index(kernel: &mut Kernel, path: &str) -> Result<(), FsError> {
    let chronos = kernel.clock.chronos_tick;
    let kairos_sector = kernel.clock.sector_id;
    let inode_id = kernel.fs.stamp_phase_time(path, chronos, kairos_sector)?;
    let tag = kernel
        .fs
        .inode(inode_id)
        .map(|inode| inode.tag)
        .ok_or_else(|| FsError::NotFound(path.into()))?;
    kernel
        .phase_index
        .relocate(inode_id, tag, kairos_sector, chronos);
    Ok(())
}

/// `FS_STAT` via `kernel.phase_cache` when trusted, falling back to (and
/// populating) the real `PhaseFs` resolution on a miss or trust rejection.
#[cfg(any(feature = "std", feature = "baremetal"))]
fn cached_stat(kernel: &mut Kernel, path: &str) -> Result<PhaseState, FsError> {
    let chronos_tick = kernel.clock.chronos_tick;
    if let Some(inode_id) = kernel
        .phase_cache
        .get(path, &kernel.statechain, chronos_tick)
    {
        return kernel
            .fs
            .inode(inode_id)
            .map(|inode| inode.tag)
            .ok_or_else(|| FsError::NotFound(path.into()));
    }
    let tag = kernel.fs.stat(path)?;
    if let Ok(inode_id) = kernel.fs.inode_id_for_path(path) {
        kernel
            .phase_cache
            .insert(path, inode_id, tag, &kernel.statechain, chronos_tick);
    }
    Ok(tag)
}

#[cfg(not(any(feature = "std", feature = "baremetal")))]
fn cached_stat(kernel: &mut Kernel, path: &str) -> Result<PhaseState, FsError> {
    kernel.fs.stat(path)
}

/// `FS_READ_FILE` via `kernel.phase_cache` when trusted (skips path
/// resolution entirely via [`PhaseFs::read_file_by_inode`]), falling back to
/// (and populating) the real path-resolving read on a miss or rejection.
/// When the `extmap` feature is enabled and holds a real record for the
/// resolved inode, its content takes priority over `fs`'s own legacy
/// single-block copy (see `kernel::extmap`'s module docs).
#[cfg(any(feature = "std", feature = "baremetal"))]
fn cached_read_file(kernel: &mut Kernel, path: &str) -> Result<Vec<u8>, FsError> {
    let chronos_tick = kernel.clock.chronos_tick;
    if let Some(inode_id) = kernel
        .phase_cache
        .get(path, &kernel.statechain, chronos_tick)
    {
        if let Some(data) = extmap_read_inode(kernel, inode_id) {
            return Ok(data);
        }
        return kernel.fs.read_file_by_inode(inode_id);
    }
    let data = kernel.fs.read_file(path)?;
    if let Ok(inode_id) = kernel.fs.inode_id_for_path(path) {
        if let Some(tag) = kernel.fs.inode(inode_id).map(|inode| inode.tag) {
            kernel
                .phase_cache
                .insert(path, inode_id, tag, &kernel.statechain, chronos_tick);
        }
        if let Some(extmap_data) = extmap_read_inode(kernel, inode_id) {
            return Ok(extmap_data);
        }
    }
    Ok(data)
}

#[cfg(not(any(feature = "std", feature = "baremetal")))]
fn cached_read_file(kernel: &mut Kernel, path: &str) -> Result<Vec<u8>, FsError> {
    kernel.fs.read_file(path)
}

/// `Some(real extmap content)` when the `extmap` feature is enabled and
/// holds a record for `inode_id`; `None` otherwise (feature off, or this
/// inode was never written through `write_inode`), in which case callers
/// fall back to `fs`'s own legacy single-block copy.
#[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
fn extmap_read_inode(kernel: &mut Kernel, inode_id: InodeId) -> Option<Vec<u8>> {
    kernel.extmap.read_inode(inode_id).ok()
}

#[cfg(not(all(feature = "extmap", any(feature = "std", feature = "baremetal"))))]
fn extmap_read_inode(_kernel: &mut Kernel, _inode_id: InodeId) -> Option<Vec<u8>> {
    None
}

/// `FS_WRITE_FILE`: also store `data` through `kernel::extmap`'s real
/// multi-block allocator, keyed by `path`'s resolved inode. A no-op when
/// the `extmap` feature is disabled.
#[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
fn extmap_write_for_path(kernel: &mut Kernel, path: &str, data: &[u8]) -> Result<(), SyscallError> {
    if let Ok(inode_id) = kernel.fs.inode_id_for_path(path) {
        kernel
            .extmap
            .write_inode(inode_id, data)
            .map_err(SyscallError::Extmap)?;
    }
    Ok(())
}

#[cfg(not(all(feature = "extmap", any(feature = "std", feature = "baremetal"))))]
fn extmap_write_for_path(
    _kernel: &mut Kernel,
    _path: &str,
    _data: &[u8],
) -> Result<(), SyscallError> {
    Ok(())
}

/// `FS_RM`: also free `inode_id`'s real `kernel::extmap` blocks, if any. A
/// no-op when the `extmap` feature is disabled.
#[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
fn extmap_delete_inode(kernel: &mut Kernel, inode_id: InodeId) -> Result<(), SyscallError> {
    kernel
        .extmap
        .delete_inode(inode_id)
        .map_err(SyscallError::Extmap)
}

#[cfg(not(all(feature = "extmap", any(feature = "std", feature = "baremetal"))))]
fn extmap_delete_inode(_kernel: &mut Kernel, _inode_id: InodeId) -> Result<(), SyscallError> {
    Ok(())
}

/// Durable PhaseFS mutation when `phasefs-journal` is on (host RAM device).
/// Does not record `txlog`. WAL success and host RAM WAL are not the RAM ring.
#[cfg(all(feature = "phasefs-journal", feature = "std"))]
fn journal_one(kernel: &mut Kernel, op: crate::fs::journal::RedoOp) -> Result<(), SyscallError> {
    let crash = kernel.journal_crash;
    kernel.journal_crash = crate::fs::journal::CrashPoint::None;
    kernel.fs_journal.set_crash(crash);
    let Kernel { fs, fs_journal, .. } = kernel;
    match fs_journal.run_transaction(fs, core::slice::from_ref(&op)) {
        Ok(_) => Ok(()),
        Err(crate::fs::journal::JournalError::InjectedCrash) => {
            let _ = fs_journal.recover(fs);
            Err(SyscallError::Journal(
                crate::fs::journal::JournalError::InjectedCrash,
            ))
        }
        Err(e) => Err(SyscallError::Journal(e)),
    }
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
pub(crate) fn persist_write_file(kernel: &mut Kernel, path: &str, data: &[u8]) -> Result<(), SyscallError> {
    journal_one(
        kernel,
        crate::fs::journal::RedoOp::Write {
            path: path.into(),
            data: data.to_vec(),
        },
    )
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
pub(crate) fn persist_mkdir(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    journal_one(
        kernel,
        crate::fs::journal::RedoOp::Mkdir { path: path.into() },
    )
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
fn persist_rm(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    journal_one(
        kernel,
        crate::fs::journal::RedoOp::Remove { path: path.into() },
    )
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
pub(crate) fn persist_lock(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    journal_one(
        kernel,
        crate::fs::journal::RedoOp::Lock { path: path.into() },
    )
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
fn persist_unlock(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    journal_one(
        kernel,
        crate::fs::journal::RedoOp::Unlock { path: path.into() },
    )
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
fn persist_rename(kernel: &mut Kernel, from: &str, to: &str) -> Result<(), SyscallError> {
    journal_one(
        kernel,
        crate::fs::journal::RedoOp::Rename {
            from: from.into(),
            to: to.into(),
        },
    )
}

/// Board persist: SD WAL first. `Ok` returns without `txlog`.
#[cfg(all(feature = "phasefs-journal", not(feature = "std")))]
pub(crate) fn persist_write_file(kernel: &mut Kernel, path: &str, data: &[u8]) -> Result<(), SyscallError> {
    #[cfg(all(target_arch = "arm", feature = "baremetal"))]
    {
        let op = crate::fs::journal::RedoOp::Write {
            path: path.into(),
            data: data.to_vec(),
        };
        let jr = crate::board::zynq7000::journal_sd::run_ops(kernel, core::slice::from_ref(&op));
        if !crate::fs::journal::wal_unavailable_falls_back_to_txlog(jr) {
            return jr.map_err(SyscallError::Journal);
        }
    }
    txlog_write_file(kernel, path, data)
}

#[cfg(all(feature = "phasefs-journal", not(feature = "std")))]
pub(crate) fn persist_mkdir(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    #[cfg(all(target_arch = "arm", feature = "baremetal"))]
    {
        let op = crate::fs::journal::RedoOp::Mkdir { path: path.into() };
        let jr = crate::board::zynq7000::journal_sd::run_ops(kernel, core::slice::from_ref(&op));
        if !crate::fs::journal::wal_unavailable_falls_back_to_txlog(jr) {
            return jr.map_err(SyscallError::Journal);
        }
    }
    txlog_mkdir(kernel, path)
}

#[cfg(all(feature = "phasefs-journal", not(feature = "std")))]
fn persist_rm(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    #[cfg(all(target_arch = "arm", feature = "baremetal"))]
    {
        let op = crate::fs::journal::RedoOp::Remove { path: path.into() };
        let jr = crate::board::zynq7000::journal_sd::run_ops(kernel, core::slice::from_ref(&op));
        if !crate::fs::journal::wal_unavailable_falls_back_to_txlog(jr) {
            return jr.map_err(SyscallError::Journal);
        }
    }
    txlog_rm(kernel, path)
}

#[cfg(all(feature = "phasefs-journal", not(feature = "std")))]
pub(crate) fn persist_lock(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    #[cfg(all(target_arch = "arm", feature = "baremetal"))]
    {
        let op = crate::fs::journal::RedoOp::Lock { path: path.into() };
        let jr = crate::board::zynq7000::journal_sd::run_ops(kernel, core::slice::from_ref(&op));
        if !crate::fs::journal::wal_unavailable_falls_back_to_txlog(jr) {
            return jr.map_err(SyscallError::Journal);
        }
    }
    kernel.fs.lock_path(path).map_err(SyscallError::Fs)
}

#[cfg(all(feature = "phasefs-journal", not(feature = "std")))]
fn persist_unlock(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    #[cfg(all(target_arch = "arm", feature = "baremetal"))]
    {
        let op = crate::fs::journal::RedoOp::Unlock { path: path.into() };
        let jr = crate::board::zynq7000::journal_sd::run_ops(kernel, core::slice::from_ref(&op));
        if !crate::fs::journal::wal_unavailable_falls_back_to_txlog(jr) {
            return jr.map_err(SyscallError::Journal);
        }
    }
    kernel.fs.unlock_path(path).map_err(SyscallError::Fs)
}

#[cfg(all(feature = "phasefs-journal", not(feature = "std")))]
fn persist_rename(kernel: &mut Kernel, from: &str, to: &str) -> Result<(), SyscallError> {
    #[cfg(all(target_arch = "arm", feature = "baremetal"))]
    {
        let op = crate::fs::journal::RedoOp::Rename {
            from: from.into(),
            to: to.into(),
        };
        let jr = crate::board::zynq7000::journal_sd::run_ops(kernel, core::slice::from_ref(&op));
        if !crate::fs::journal::wal_unavailable_falls_back_to_txlog(jr) {
            return jr.map_err(SyscallError::Journal);
        }
    }
    kernel.fs.rename(from, to).map_err(SyscallError::Fs)
}

#[cfg(not(feature = "phasefs-journal"))]
pub(crate) fn persist_write_file(kernel: &mut Kernel, path: &str, data: &[u8]) -> Result<(), SyscallError> {
    txlog_write_file(kernel, path, data)
}

#[cfg(not(feature = "phasefs-journal"))]
pub(crate) fn persist_mkdir(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    txlog_mkdir(kernel, path)
}

#[cfg(not(feature = "phasefs-journal"))]
fn persist_rm(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    txlog_rm(kernel, path)
}

#[cfg(not(feature = "phasefs-journal"))]
pub(crate) fn persist_lock(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    kernel.fs.lock_path(path).map_err(SyscallError::Fs)
}

#[cfg(not(feature = "phasefs-journal"))]
fn persist_unlock(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    kernel.fs.unlock_path(path).map_err(SyscallError::Fs)
}

#[cfg(not(feature = "phasefs-journal"))]
fn persist_rename(kernel: &mut Kernel, from: &str, to: &str) -> Result<(), SyscallError> {
    kernel.fs.rename(from, to).map_err(SyscallError::Fs)
}

/// RAM `txlog` one-op write. Persist calls this when `phasefs-journal` is
/// off, or when board WAL cannot start. WAL success does not call this.
#[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
#[allow(dead_code)]
fn txlog_write_file(kernel: &mut Kernel, path: &str, data: &[u8]) -> Result<(), SyscallError> {
    let tick = kernel.clock.kairos_tick;
    let sector = kernel.clock.sector_id;
    let ops = vec![crate::kernel::txlog::TxOp::WriteFile {
        path: path.into(),
        data: data.to_vec(),
    }];
    let Kernel {
        fs,
        statechain,
        txlog,
        ..
    } = kernel;
    txlog
        .run_transaction(fs, statechain, tick, sector, ops)
        .map(|_| ())
        .map_err(SyscallError::Txlog)
}

#[cfg(not(all(feature = "txlog", any(feature = "std", feature = "baremetal"))))]
#[allow(dead_code)]
fn txlog_write_file(kernel: &mut Kernel, path: &str, data: &[u8]) -> Result<(), SyscallError> {
    kernel.fs.write_file(path, data).map_err(SyscallError::Fs)
}

/// `FS_MKDIR`: same real-transaction routing as [`txlog_write_file`].
#[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
#[allow(dead_code)]
fn txlog_mkdir(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    let tick = kernel.clock.kairos_tick;
    let sector = kernel.clock.sector_id;
    let ops = vec![crate::kernel::txlog::TxOp::Mkdir { path: path.into() }];
    let Kernel {
        fs,
        statechain,
        txlog,
        ..
    } = kernel;
    txlog
        .run_transaction(fs, statechain, tick, sector, ops)
        .map(|_| ())
        .map_err(SyscallError::Txlog)
}

#[cfg(not(all(feature = "txlog", any(feature = "std", feature = "baremetal"))))]
#[allow(dead_code)]
fn txlog_mkdir(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    kernel.fs.mkdir(path).map_err(SyscallError::Fs)
}

/// `FS_RM`: same real-transaction routing as [`txlog_write_file`].
#[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
#[allow(dead_code)]
fn txlog_rm(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    let tick = kernel.clock.kairos_tick;
    let sector = kernel.clock.sector_id;
    let ops = vec![crate::kernel::txlog::TxOp::Rm { path: path.into() }];
    let Kernel {
        fs,
        statechain,
        txlog,
        ..
    } = kernel;
    txlog
        .run_transaction(fs, statechain, tick, sector, ops)
        .map(|_| ())
        .map_err(SyscallError::Txlog)
}

#[cfg(not(all(feature = "txlog", any(feature = "std", feature = "baremetal"))))]
#[allow(dead_code)]
fn txlog_rm(kernel: &mut Kernel, path: &str) -> Result<(), SyscallError> {
    kernel.fs.rm(path).map_err(SyscallError::Fs)
}

/// Invalidate `path`'s `PhaseCache` entry, if any, after a mutation.
#[cfg(any(feature = "std", feature = "baremetal"))]
fn invalidate_cache_path(kernel: &mut Kernel, path: &str) {
    kernel.phase_cache.invalidate_path(path);
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn invalidate_cache_path_prefix(kernel: &mut Kernel, path: &str) {
    kernel.phase_cache.invalidate_path_prefix(path);
}

#[cfg(not(any(feature = "std", feature = "baremetal")))]
fn invalidate_cache_path_prefix(_kernel: &mut Kernel, _path: &str) {}

#[cfg(not(any(feature = "std", feature = "baremetal")))]
fn invalidate_cache_path(_kernel: &mut Kernel, _path: &str) {}

/// Invalidate every `PhaseCache` entry for `inode`, by id (used by `FS_RM`,
/// which resolves the path *before* deleting it, so the id is known but the
/// path may no longer resolve through `PhaseFs`).
#[cfg(any(feature = "std", feature = "baremetal"))]
fn invalidate_cache_inode(kernel: &mut Kernel, inode: InodeId) {
    kernel.phase_cache.invalidate_inode(inode);
}

#[cfg(not(any(feature = "std", feature = "baremetal")))]
fn invalidate_cache_inode(_kernel: &mut Kernel, _inode: InodeId) {}

/// `FS_OPEN`: resolve `path` to its inode and mint a capability naming it,
/// owned by `caller`, carrying `rights`. Fails with `FsError::NotFound` if
/// `path` does not resolve to an existing inode — `FS_OPEN` opens existing
/// files, it does not create them (use `FS_WRITE_FILE`/`FS_MKDIR` for
/// creation). Returns plain `SyscallResult::Ok` (no handle) when the
/// `captable` feature is disabled, matching the pre-`captable` stub ABI.
#[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
enum FileCapMode {
    Target,
    Parent,
}

#[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
fn require_file_capability(
    kernel: &mut Kernel,
    caller: ProcessId,
    cap_slot: u32,
    cap_generation: u32,
    path: &str,
    requested: u32,
    mode: FileCapMode,
) -> Result<(), SyscallError> {
    let cap = GenerationalHandle {
        slot: cap_slot,
        generation: cap_generation,
    };
    let now = kernel.clock.chronos_tick;
    let named = kernel
        .captable
        .resolve_file(cap, caller, requested, now)
        .map_err(SyscallError::Captable)?;
    let expected = match mode {
        FileCapMode::Target => kernel
            .fs
            .inode_id_for_path(path)
            .map_err(SyscallError::Fs)?,
        FileCapMode::Parent => {
            let (parent, _) = crate::fs::directory::split_parent_name(path)
                .map_err(|e| SyscallError::Fs(FsError::Path(e)))?;
            kernel
                .fs
                .inode_id_for_path(&parent)
                .map_err(SyscallError::Fs)?
        }
    };
    if named != expected {
        return Err(SyscallError::Captable(CapError::WrongObjectKind));
    }
    Ok(())
}

#[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
fn open_file_capability(
    kernel: &mut Kernel,
    caller: ProcessId,
    path: &str,
    rights: u32,
) -> Result<SyscallResult, SyscallError> {
    let inode_id = kernel
        .fs
        .inode_id_for_path(path)
        .map_err(SyscallError::Fs)?;
    let handle = kernel.captable.open_file(inode_id, caller, rights, None);
    Ok(SyscallResult::Handle(handle.slot, handle.generation))
}

#[cfg(not(all(feature = "captable", any(feature = "std", feature = "baremetal"))))]
fn open_file_capability(
    _kernel: &mut Kernel,
    _caller: ProcessId,
    _path: &str,
    _rights: u32,
) -> Result<SyscallResult, SyscallError> {
    Ok(SyscallResult::Ok)
}

/// `FS_CLOSE`: revoke exactly the capability slot named by `(slot,
/// generation)`. A no-op returning `Ok` when the `captable` feature is
/// disabled, matching the pre-`captable` stub ABI.
#[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
fn close_file_capability(
    kernel: &mut Kernel,
    slot: u32,
    generation: u32,
) -> Result<(), SyscallError> {
    kernel
        .captable
        .close(GenerationalHandle { slot, generation })
        .map_err(SyscallError::Captable)
}

#[cfg(not(all(feature = "captable", any(feature = "std", feature = "baremetal"))))]
fn close_file_capability(
    _kernel: &mut Kernel,
    _slot: u32,
    _generation: u32,
) -> Result<(), SyscallError> {
    Ok(())
}

/// `FS_RM`: destroy the `captable` object bound to `inode_id`, if any,
/// invalidating every outstanding capability naming it. A no-op when the
/// `captable` feature is disabled.
#[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
fn destroy_file_capability_object(kernel: &mut Kernel, inode_id: InodeId) {
    kernel.captable.destroy_object_for_inode(inode_id);
}

#[cfg(not(all(feature = "captable", any(feature = "std", feature = "baremetal"))))]
fn destroy_file_capability_object(_kernel: &mut Kernel, _inode_id: InodeId) {}

/// `PROC_SLEEP`: register `caller` with `kernel.sched2` on first use (lazy
/// registration — this subsystem does not hook `spawn_process`, keeping the
/// default (feature-off) spawn path byte-unaffected), dispatch it to
/// `Running` if it is not already (the syscall's own `PhaseState` machinery
/// already proved `caller` is live), then block it for `ticks` sched2 ticks
/// from the wheel's own last-observed tick (`Sched2::elapsed`). A no-op
/// returning `Ok` when the `sched2` feature is disabled.
#[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
fn sched2_sleep(kernel: &mut Kernel, caller: ProcessId, ticks: u64) -> Result<(), SyscallError> {
    use crate::kernel::sched2::{RunState, DEFAULT_WEIGHT};

    let handle = match kernel.sched2.handle_for_pid(caller) {
        Some(h) => h,
        None => kernel
            .sched2
            .register(caller, DEFAULT_WEIGHT, kernel.sched2.elapsed())
            .map_err(SyscallError::Sched2)?,
    };
    if kernel
        .sched2
        .run_state(handle)
        .map_err(SyscallError::Sched2)?
        == RunState::Ready
    {
        kernel
            .sched2
            .dispatch(handle)
            .map_err(SyscallError::Sched2)?;
    }
    let now = kernel.sched2.elapsed();
    kernel
        .sched2
        .sleep_until(handle, now.saturating_add(ticks), now)
        .map_err(SyscallError::Sched2)
}

#[cfg(not(all(feature = "sched2", any(feature = "std", feature = "baremetal"))))]
fn sched2_sleep(_kernel: &mut Kernel, _caller: ProcessId, _ticks: u64) -> Result<(), SyscallError> {
    Ok(())
}

/// `IPC_CREATE_QUEUE`: create a new bounded `kernel::ipc` queue, then mint
/// an owning `captable` capability on it for `caller` in the same call
/// (mirrors `open_file_capability`'s single combined create+mint shape). A
/// no-op returning plain `Ok` when the `ipc` feature is disabled.
#[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
fn ipc_create_queue(
    kernel: &mut Kernel,
    caller: ProcessId,
    capacity: usize,
    rights: u32,
) -> Result<SyscallResult, SyscallError> {
    let ipc_handle = kernel
        .ipc
        .create_queue(capacity)
        .map_err(SyscallError::Ipc)?;
    let cap = kernel
        .captable
        .open_queue(ipc_handle.pack(), caller, rights, None);
    Ok(SyscallResult::Handle(cap.slot, cap.generation))
}

#[cfg(not(all(feature = "ipc", any(feature = "std", feature = "baremetal"))))]
fn ipc_create_queue(
    _kernel: &mut Kernel,
    _caller: ProcessId,
    _capacity: usize,
    _rights: u32,
) -> Result<SyscallResult, SyscallError> {
    Ok(SyscallResult::Ok)
}

/// `IPC_SEND`: authorize `RIGHT_WRITE` on `(cap_slot, cap_generation)`,
/// resolve it to the real queue it names, enqueue `data`, then wake any
/// real receiver already blocked on this exact queue via `sched2`. Never
/// blocks the sender (see `kernel::ipc`'s module docs). A no-op returning
/// `Ok` when the `ipc` feature is disabled.
#[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
fn ipc_send(
    kernel: &mut Kernel,
    caller: ProcessId,
    cap_slot: u32,
    cap_generation: u32,
    data: &[u8],
) -> Result<(), SyscallError> {
    use crate::kernel::captable::RIGHT_WRITE;
    use crate::kernel::ipc::IpcHandle;

    let cap = GenerationalHandle {
        slot: cap_slot,
        generation: cap_generation,
    };
    let now_chronos = kernel.clock.chronos_tick;
    let queue_ref = kernel
        .captable
        .resolve_queue(cap, caller, RIGHT_WRITE, now_chronos)
        .map_err(SyscallError::Captable)?;
    let handle = IpcHandle::unpack(queue_ref);
    kernel
        .ipc
        .send(handle, caller, data)
        .map_err(SyscallError::Ipc)?;
    let now = kernel.sched2.elapsed();
    let mut woken = Vec::new();
    kernel.sched2.wake_matching(queue_ref, now, &mut woken);
    Ok(())
}

#[cfg(not(all(feature = "ipc", any(feature = "std", feature = "baremetal"))))]
fn ipc_send(
    _kernel: &mut Kernel,
    _caller: ProcessId,
    _cap_slot: u32,
    _cap_generation: u32,
    _data: &[u8],
) -> Result<(), SyscallError> {
    Ok(())
}

/// `IPC_RECV`: authorize `RIGHT_READ`, resolve the real queue, and either
/// return its oldest message or — on an empty queue — lazily register
/// `caller` with `sched2` (first use only, same pattern as
/// `sched2_sleep`), dispatch it to `Running` if needed, then
/// `block_on_ipc` it on this exact queue and return
/// `SyscallError::Ipc(IpcError::QueueEmpty)` (not a fatal condition — see
/// `kernel::ipc`'s module docs). A no-op returning plain `Ok` when the
/// `ipc` feature is disabled.
#[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
fn ipc_recv(
    kernel: &mut Kernel,
    caller: ProcessId,
    cap_slot: u32,
    cap_generation: u32,
) -> Result<SyscallResult, SyscallError> {
    use crate::kernel::captable::RIGHT_READ;
    use crate::kernel::ipc::IpcHandle;
    use crate::kernel::sched2::{RunState, DEFAULT_WEIGHT};

    let cap = GenerationalHandle {
        slot: cap_slot,
        generation: cap_generation,
    };
    let now_chronos = kernel.clock.chronos_tick;
    let queue_ref = kernel
        .captable
        .resolve_queue(cap, caller, RIGHT_READ, now_chronos)
        .map_err(SyscallError::Captable)?;
    let handle = IpcHandle::unpack(queue_ref);
    match kernel.ipc.recv(handle) {
        Ok(message) => Ok(SyscallResult::Message(message.sender, message.data)),
        Err(IpcError::QueueEmpty) => {
            let sched2_handle = match kernel.sched2.handle_for_pid(caller) {
                Some(h) => h,
                None => kernel
                    .sched2
                    .register(caller, DEFAULT_WEIGHT, kernel.sched2.elapsed())
                    .map_err(SyscallError::Sched2)?,
            };
            if kernel
                .sched2
                .run_state(sched2_handle)
                .map_err(SyscallError::Sched2)?
                == RunState::Ready
            {
                kernel
                    .sched2
                    .dispatch(sched2_handle)
                    .map_err(SyscallError::Sched2)?;
            }
            let now = kernel.sched2.elapsed();
            kernel
                .sched2
                .block_on_ipc(sched2_handle, queue_ref, now)
                .map_err(SyscallError::Sched2)?;
            Err(SyscallError::Ipc(IpcError::QueueEmpty))
        }
        Err(err) => Err(SyscallError::Ipc(err)),
    }
}

#[cfg(not(all(feature = "ipc", any(feature = "std", feature = "baremetal"))))]
fn ipc_recv(
    _kernel: &mut Kernel,
    _caller: ProcessId,
    _cap_slot: u32,
    _cap_generation: u32,
) -> Result<SyscallResult, SyscallError> {
    Ok(SyscallResult::Ok)
}

/// `IPC_DESTROY_QUEUE`: authorize `RIGHT_DELETE` on `(cap_slot,
/// cap_generation)`, destroy its `captable` object (invalidating every
/// outstanding capability on it) and close that capability, then destroy
/// the real `kernel::ipc` queue it named. A no-op returning `Ok` when the
/// `ipc` feature is disabled.
#[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
fn ipc_destroy_queue(
    kernel: &mut Kernel,
    caller: ProcessId,
    cap_slot: u32,
    cap_generation: u32,
) -> Result<(), SyscallError> {
    use crate::kernel::ipc::IpcHandle;

    let cap = GenerationalHandle {
        slot: cap_slot,
        generation: cap_generation,
    };
    let now_chronos = kernel.clock.chronos_tick;
    let queue_ref = kernel
        .captable
        .destroy_queue_via_cap(cap, caller, now_chronos)
        .map_err(SyscallError::Captable)?;
    kernel
        .ipc
        .destroy_queue(IpcHandle::unpack(queue_ref))
        .map_err(SyscallError::Ipc)
}

#[cfg(not(all(feature = "ipc", any(feature = "std", feature = "baremetal"))))]
fn ipc_destroy_queue(
    _kernel: &mut Kernel,
    _caller: ProcessId,
    _cap_slot: u32,
    _cap_generation: u32,
) -> Result<(), SyscallError> {
    Ok(())
}

/// Kill another process through the kernel syscall path.
pub fn kill_process(
    kernel: &mut Kernel,
    caller: ProcessId,
    target: ProcessId,
) -> Result<(), SyscallError> {
    enter_kernel(kernel, caller)?;
    if kernel.get_process(target).is_err() {
        leave_kernel(kernel, caller)?;
        return Err(SyscallError::BadTarget);
    }
    if kernel.get_process(target).unwrap().state == PhaseState::Phi0 {
        leave_kernel(kernel, caller)?;
        return Ok(());
    }
    ensure_target_kernel(kernel, target)?;
    kernel
        .exit(target)
        .map_err(SyscallError::Kernel)?
        .then_some(())
        .ok_or(SyscallError::Kernel("kill exit rejected"))?;
    leave_kernel(kernel, caller)?;
    Ok(())
}

/// Real hosted no-echo terminal support for [`IoBuffer::read_secret_line`].
/// Only compiled for `std` + `identity` — see `kernel::identity::credential`
/// module docs, "Honest disclosure: no-echo secret input" for why a real
/// terminal is required to secure at all, and what happens when one is not
/// present (redirected/piped stdin: falls back to a plain read since there
/// is no echo to suppress there either).
#[cfg(all(feature = "std", feature = "identity"))]
mod secret_input {
    use crate::platform::String;

    fn plain_read_line() -> Option<String> {
        let mut buf = std::string::String::new();
        match std::io::stdin().read_line(&mut buf) {
            Ok(0) => None,
            Ok(_) => Some(buf.trim_end_matches(['\r', '\n']).to_string()),
            Err(_) => None,
        }
    }

    /// Real POSIX termios no-echo read: clears `ECHO` on the real
    /// controlling terminal's local mode flags for the duration of one
    /// line read, then restores the original mode — even on early return.
    #[cfg(unix)]
    pub fn read_line_no_echo() -> Option<String> {
        use libc::{tcgetattr, tcsetattr, termios, ECHO, STDIN_FILENO, TCSANOW};
        let mut term: termios = unsafe { core::mem::zeroed() };
        if unsafe { tcgetattr(STDIN_FILENO, &mut term) } != 0 {
            // Not a real tty (e.g. redirected stdin) -- nothing to secure.
            return plain_read_line();
        }
        let original = term;
        term.c_lflag &= !ECHO;
        if unsafe { tcsetattr(STDIN_FILENO, TCSANOW, &term) } != 0 {
            return plain_read_line();
        }
        let line = plain_read_line();
        unsafe {
            tcsetattr(STDIN_FILENO, TCSANOW, &original);
        }
        line
    }

    /// Real Win32 console no-echo read: clears `ENABLE_ECHO_INPUT` on the
    /// real console input handle via the documented `kernel32.dll` console
    /// API for the duration of one line read, then restores the original
    /// mode. Raw FFI (no extra crate) — these three functions are stable,
    /// long-documented Win32 APIs.
    #[cfg(windows)]
    pub fn read_line_no_echo() -> Option<String> {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetStdHandle(nStdHandle: i32) -> *mut core::ffi::c_void;
            fn GetConsoleMode(hConsoleHandle: *mut core::ffi::c_void, lpMode: *mut u32) -> i32;
            fn SetConsoleMode(hConsoleHandle: *mut core::ffi::c_void, dwMode: u32) -> i32;
        }
        const STD_INPUT_HANDLE: i32 = -10;
        const ENABLE_ECHO_INPUT: u32 = 0x0004;

        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            if handle.is_null() || handle as isize == -1 {
                return plain_read_line();
            }
            let mut mode: u32 = 0;
            if GetConsoleMode(handle, &mut mode) == 0 {
                // Not a real console (e.g. redirected input) -- nothing to secure.
                return plain_read_line();
            }
            let original = mode;
            SetConsoleMode(handle, mode & !ENABLE_ECHO_INPUT);
            let line = plain_read_line();
            SetConsoleMode(handle, original);
            line
        }
    }

    /// No real terminal-control API is wired for this hosted target; fall
    /// back to a plain read rather than fake a no-echo guarantee this
    /// platform cannot actually provide.
    #[cfg(not(any(unix, windows)))]
    pub fn read_line_no_echo() -> Option<String> {
        plain_read_line()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::gphase_core::PhaseState;

    fn running(kernel: &mut Kernel, pid: ProcessId) {
        kernel.wake(pid).unwrap();
        kernel.schedule(pid).unwrap();
    }

    fn dispatch_fs(
        kernel: &mut Kernel,
        pid: ProcessId,
        io: &mut IoBuffer,
        nr: u32,
        args: &SyscallArgs,
    ) -> Result<SyscallResult, SyscallError> {
        #[allow(unused_mut)]
        let mut args = args.clone();
        #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
        {
            use crate::kernel::captable::{RIGHT_DELETE, RIGHT_READ, RIGHT_WRITE};
            let (open_path, rights) = match nr {
                FS_READ_FILE => (args.path.clone(), RIGHT_READ),
                FS_WRITE_FILE => {
                    if kernel.fs.inode_id_for_path(&args.path).is_ok() {
                        (args.path.clone(), RIGHT_WRITE)
                    } else {
                        let (parent, _) = crate::fs::directory::split_parent_name(&args.path)
                            .expect("absolute non-root test path");
                        (parent, RIGHT_WRITE)
                    }
                }
                FS_MKDIR => {
                    let (parent, _) = crate::fs::directory::split_parent_name(&args.path)
                        .expect("absolute non-root test path");
                    (parent, RIGHT_WRITE)
                }
                FS_RM => (args.path.clone(), RIGHT_DELETE),
                _ => {
                    running(kernel, pid);
                    return dispatch(kernel, pid, io, nr, &args);
                }
            };
            running(kernel, pid);
            let open_args = SyscallArgs {
                path: open_path,
                rights,
                ..Default::default()
            };
            match dispatch(kernel, pid, io, FS_OPEN, &open_args).unwrap() {
                SyscallResult::Handle(slot, generation) => {
                    args.cap_slot = slot;
                    args.cap_generation = generation;
                }
                other => panic!("expected file cap, got {other:?}"),
            }
        }
        running(kernel, pid);
        dispatch(kernel, pid, io, nr, &args)
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal"))]
    #[test]
    fn persist_write_skips_txlog_when_journal_succeeds() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let before = kernel.txlog.stats();
        let create = SyscallArgs {
            path: "/a10.txt".into(),
            text: "wal".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_WRITE_FILE, &create).unwrap();
        assert_eq!(kernel.fs.read_file("/a10.txt").unwrap(), b"wal");
        assert_eq!(
            kernel.txlog.stats(),
            before,
            "WAL success must not record txlog"
        );
        assert_eq!(before.committed, 0);
        assert_eq!(before.records, 0);
    }

    /// A12: WAL recover restores PhaseFs; extents stay empty until
    /// `journal_post_recover` rebuilds them from recovered file bytes.
    #[cfg(all(feature = "extmap", feature = "phasefs-journal"))]
    #[test]
    fn journal_recover_rebuilds_extmap_extents() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let payload: String = (0..1400u32)
            .map(|i| ((i % 26) as u8 + b'a') as char)
            .collect();
        let write_args = SyscallArgs {
            path: "/a12.bin".into(),
            text: payload.clone(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_WRITE_FILE, &write_args).unwrap();
        assert_eq!(kernel.extmap.stats().blocks_live, 3);

        let mut recovered = crate::fs::PhaseFs::new();
        kernel.fs_journal.recover(&mut recovered).unwrap();
        kernel.fs = recovered;
        kernel.extmap = crate::kernel::extmap::Extmap::default();
        assert_eq!(
            kernel.extmap.stats().blocks_live,
            0,
            "RAM extmap is empty after power-cut; namespace is in recovered PhaseFs"
        );
        assert_eq!(kernel.fs.read_file("/a12.bin").unwrap(), payload.as_bytes());

        kernel.journal_post_recover();
        assert_eq!(kernel.extmap.stats().blocks_live, 3);
        let inode = kernel.fs.inode_id_for_path("/a12.bin").unwrap();
        assert_eq!(
            kernel.extmap.read_inode(inode).unwrap(),
            payload.as_bytes()
        );

        running(&mut kernel, pid);
        let read_args = SyscallArgs {
            path: "/a12.bin".into(),
            ..Default::default()
        };
        match dispatch_fs(&mut kernel, pid, &mut io, FS_READ_FILE, &read_args).unwrap() {
            SyscallResult::Bytes(data) => assert_eq!(data, payload.as_bytes()),
            other => panic!("expected file bytes, got {other:?}"),
        }
    }

    #[test]
    fn write_syscall_requires_running() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        let mut io = IoBuffer::default();
        let err = dispatch(&mut kernel, pid, &mut io, WRITE, &SyscallArgs::default()).unwrap_err();
        assert_eq!(err, SyscallError::NotRunning);
    }

    #[test]
    fn read_secret_line_consumes_scripted_input_like_read_line() {
        let mut io = IoBuffer::with_input(["hunter2", "second line"]);
        assert_eq!(io.read_secret_line(), Some("hunter2".to_string()));
        assert_eq!(io.read_line(), Some("second line".to_string()));
        assert_eq!(io.read_secret_line(), None);
    }

    #[test]
    fn read_secret_line_uses_the_installed_platform_reader_over_scripted_input_when_present() {
        fn fake_reader() -> String {
            "from-hardware".to_string()
        }
        let mut io = IoBuffer::default();
        io.set_secret_reader(fake_reader);
        assert_eq!(io.read_secret_line(), Some("from-hardware".to_string()));
    }

    #[test]
    fn read_secret_line_prefers_scripted_input_over_the_installed_reader() {
        fn fake_reader() -> String {
            "from-hardware".to_string()
        }
        let mut io = IoBuffer::with_input(["scripted"]);
        io.set_secret_reader(fake_reader);
        assert_eq!(io.read_secret_line(), Some("scripted".to_string()));
        assert_eq!(io.read_secret_line(), Some("from-hardware".to_string()));
    }

    #[test]
    fn read_syscall_read_secret_round_trips_through_dispatch() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::with_input(["a secret value"]);
        let result = dispatch(
            &mut kernel,
            pid,
            &mut io,
            READ_SECRET,
            &SyscallArgs::default(),
        )
        .unwrap();
        assert_eq!(result, SyscallResult::Text("a secret value".to_string()));
    }

    #[test]
    fn write_and_phase_state_syscalls() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let args = SyscallArgs {
            text: "hello".into(),
            ..Default::default()
        };
        dispatch(&mut kernel, pid, &mut io, WRITE, &args).unwrap();
        assert_eq!(io.output, vec!["hello"]);
        running(&mut kernel, pid);
        let state = dispatch(
            &mut kernel,
            pid,
            &mut io,
            PHASE_STATE,
            &SyscallArgs::default(),
        )
        .unwrap();
        assert_eq!(state, SyscallResult::PhaseState(PhaseState::Phi2));
    }

    #[test]
    fn fs_mkdir_indexes_the_new_directory_in_phase_index() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let args = SyscallArgs {
            path: "/docs".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_MKDIR, &args).unwrap();

        let inode_id = kernel.fs.inode_id_for_path("/docs").unwrap();
        assert!(kernel.phase_index.contains(inode_id));
        let sector = kernel.clock.sector_id;
        let hits = kernel
            .phase_index
            .query_range(PhaseState::Phi2, sector, 0, u64::MAX);
        assert!(hits.contains(&inode_id));
    }

    #[test]
    fn fs_write_file_reindexes_on_the_current_clock() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let create = SyscallArgs {
            path: "/a.txt".into(),
            text: "one".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_WRITE_FILE, &create).unwrap();
        let inode_id = kernel.fs.inode_id_for_path("/a.txt").unwrap();
        let first_chronos = kernel.fs.inode(inode_id).unwrap().chronos;

        kernel.advance_chronos();
        running(&mut kernel, pid);
        let rewrite = SyscallArgs {
            path: "/a.txt".into(),
            text: "two".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_WRITE_FILE, &rewrite).unwrap();
        let second_chronos = kernel.fs.inode(inode_id).unwrap().chronos;

        assert!(second_chronos > first_chronos);
        assert!(kernel.phase_index.contains(inode_id));
    }

    #[test]
    fn fs_rm_removes_the_inode_from_phase_index() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let create = SyscallArgs {
            path: "/gone.txt".into(),
            text: "bye".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_WRITE_FILE, &create).unwrap();
        let inode_id = kernel.fs.inode_id_for_path("/gone.txt").unwrap();
        assert!(kernel.phase_index.contains(inode_id));

        running(&mut kernel, pid);
        let rm = SyscallArgs {
            path: "/gone.txt".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_RM, &rm).unwrap();
        assert!(!kernel.phase_index.contains(inode_id));
    }

    fn dispatch_rename(
        kernel: &mut Kernel,
        pid: ProcessId,
        io: &mut IoBuffer,
        from: &str,
        to: &str,
    ) -> Result<SyscallResult, SyscallError> {
        running(kernel, pid);
        let args = SyscallArgs {
            path: from.into(),
            path_b: to.into(),
            ..Default::default()
        };
        dispatch(kernel, pid, io, FS_RENAME, &args)
    }

    #[test]
    fn fs_rename_file_succeeds_and_preserves_data_and_phase() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let create = SyscallArgs {
            path: "/old.txt".into(),
            text: "payload".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_WRITE_FILE, &create).unwrap();
        let inode_id = kernel.fs.inode_id_for_path("/old.txt").unwrap();
        let tag = kernel.fs.stat("/old.txt").unwrap();
        let first_chronos = kernel.fs.inode(inode_id).unwrap().chronos;

        dispatch_rename(&mut kernel, pid, &mut io, "/old.txt", "/new.txt").unwrap();

        assert!(kernel.fs.inode_id_for_path("/old.txt").is_err());
        assert_eq!(kernel.fs.inode_id_for_path("/new.txt").unwrap(), inode_id);
        assert_eq!(kernel.fs.read_file("/new.txt").unwrap(), b"payload");
        assert_eq!(kernel.fs.stat("/new.txt").unwrap(), tag);
        assert_eq!(
            kernel.fs.owner_path_for("/new.txt").unwrap().as_deref(),
            Some("/new.txt")
        );
        assert!(kernel.phase_index.contains(inode_id));
        let second_chronos = kernel.fs.inode(inode_id).unwrap().chronos;
        assert!(second_chronos >= first_chronos);
        kernel.statechain.verify_all().unwrap();
    }

    #[test]
    fn fs_rename_directory_succeeds() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_MKDIR,
            &SyscallArgs {
                path: "/docs".into(),
                ..Default::default()
            },
        )
        .unwrap();
        running(&mut kernel, pid);
        dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_WRITE_FILE,
            &SyscallArgs {
                path: "/docs/a.txt".into(),
                text: "child".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let dir_id = kernel.fs.inode_id_for_path("/docs").unwrap();

        dispatch_rename(&mut kernel, pid, &mut io, "/docs", "/papers").unwrap();

        assert!(kernel.fs.inode_id_for_path("/docs").is_err());
        assert_eq!(kernel.fs.inode_id_for_path("/papers").unwrap(), dir_id);
        assert_eq!(kernel.fs.read_file("/papers/a.txt").unwrap(), b"child");
        assert!(kernel.phase_index.contains(dir_id));
    }

    #[test]
    fn fs_rename_refuses_missing_source() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        let mut io = IoBuffer::default();
        let err = dispatch_rename(&mut kernel, pid, &mut io, "/nope.txt", "/x.txt").unwrap_err();
        #[cfg(not(feature = "phasefs-journal"))]
        assert!(matches!(err, SyscallError::Fs(FsError::NotFound(_))));
        #[cfg(feature = "phasefs-journal")]
        assert!(matches!(
            err,
            SyscallError::Journal(crate::fs::journal::JournalError::Fs)
        ));
    }

    #[test]
    fn fs_rename_refuses_destination_collision() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_WRITE_FILE,
            &SyscallArgs {
                path: "/a.txt".into(),
                text: "a".into(),
                ..Default::default()
            },
        )
        .unwrap();
        running(&mut kernel, pid);
        dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_WRITE_FILE,
            &SyscallArgs {
                path: "/b.txt".into(),
                text: "b".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let err = dispatch_rename(&mut kernel, pid, &mut io, "/a.txt", "/b.txt").unwrap_err();
        #[cfg(not(feature = "phasefs-journal"))]
        assert!(matches!(err, SyscallError::Fs(FsError::AlreadyExists(_))));
        #[cfg(feature = "phasefs-journal")]
        assert!(matches!(
            err,
            SyscallError::Journal(crate::fs::journal::JournalError::Fs)
        ));
        assert_eq!(kernel.fs.read_file("/a.txt").unwrap(), b"a");
        assert_eq!(kernel.fs.read_file("/b.txt").unwrap(), b"b");
    }

    #[test]
    fn fs_rename_refuses_invalid_path() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_WRITE_FILE,
            &SyscallArgs {
                path: "/ok.txt".into(),
                text: "x".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let err = dispatch_rename(&mut kernel, pid, &mut io, "relative.txt", "/x.txt").unwrap_err();
        #[cfg(not(feature = "phasefs-journal"))]
        assert!(matches!(err, SyscallError::Fs(FsError::Path(_))));
        #[cfg(feature = "phasefs-journal")]
        assert!(matches!(
            err,
            SyscallError::Journal(crate::fs::journal::JournalError::Fs)
        ));
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    #[test]
    fn fs_rename_invalidates_old_cache_and_index_resolves_new_path() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_WRITE_FILE,
            &SyscallArgs {
                path: "/cached.txt".into(),
                text: "hi".into(),
                ..Default::default()
            },
        )
        .unwrap();
        running(&mut kernel, pid);
        dispatch(
            &mut kernel,
            pid,
            &mut io,
            FS_STAT,
            &SyscallArgs {
                path: "/cached.txt".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(kernel.phase_cache.stats().misses, 1);

        running(&mut kernel, pid);
        dispatch(
            &mut kernel,
            pid,
            &mut io,
            FS_STAT,
            &SyscallArgs {
                path: "/cached.txt".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(kernel.phase_cache.stats().hits, 1);

        dispatch_rename(&mut kernel, pid, &mut io, "/cached.txt", "/moved.txt").unwrap();

        running(&mut kernel, pid);
        assert!(matches!(
            dispatch(
                &mut kernel,
                pid,
                &mut io,
                FS_STAT,
                &SyscallArgs {
                    path: "/cached.txt".into(),
                    ..Default::default()
                },
            ),
            Err(SyscallError::Fs(FsError::NotFound(_)))
        ));
        running(&mut kernel, pid);
        let tag = dispatch(
            &mut kernel,
            pid,
            &mut io,
            FS_STAT,
            &SyscallArgs {
                path: "/moved.txt".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(tag, SyscallResult::PhaseState(PhaseState::Phi1));
        let inode_id = kernel.fs.inode_id_for_path("/moved.txt").unwrap();
        assert!(kernel.phase_index.contains(inode_id));
        let sector = kernel.clock.sector_id;
        let hits = kernel
            .phase_index
            .query_range(PhaseState::Phi1, sector, 0, u64::MAX);
        assert!(hits.contains(&inode_id));
    }

    #[cfg(feature = "std")]
    #[test]
    fn fs_rename_records_statechain_audit_for_both_paths() {
        use crate::kernel::state_proof::{hash_payload, ObjectKind};

        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_WRITE_FILE,
            &SyscallArgs {
                path: "/from.txt".into(),
                text: "x".into(),
                ..Default::default()
            },
        )
        .unwrap();
        dispatch_rename(&mut kernel, pid, &mut io, "/from.txt", "/to.txt").unwrap();
        kernel.statechain.verify_all().unwrap();

        let from_id = u16::from_le_bytes({
            let d = hash_payload(b"/from.txt");
            [d[0], d[1]]
        }) as u32;
        let to_id = u16::from_le_bytes({
            let d = hash_payload(b"/to.txt");
            [d[0], d[1]]
        }) as u32;
        let from_ops: Vec<&str> = kernel
            .statechain
            .chain(ObjectKind::File, from_id)
            .unwrap()
            .events
            .iter()
            .map(|e| e.proof.operation_name.as_str())
            .collect();
        let to_ops: Vec<&str> = kernel
            .statechain
            .chain(ObjectKind::File, to_id)
            .unwrap()
            .events
            .iter()
            .map(|e| e.proof.operation_name.as_str())
            .collect();
        assert!(from_ops.contains(&"fs_rename"));
        assert!(to_ops.contains(&"fs_rename"));
    }

    #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
    #[test]
    fn fs_open_mints_capability_and_fs_close_revokes_it() {
        use crate::kernel::captable::{CapError, RIGHT_READ};

        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let create = SyscallArgs {
            path: "/opened.txt".into(),
            text: "hi".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_WRITE_FILE, &create).unwrap();

        running(&mut kernel, pid);
        let open_args = SyscallArgs {
            path: "/opened.txt".into(),
            rights: RIGHT_READ,
            ..Default::default()
        };
        let (slot, generation) =
            match dispatch(&mut kernel, pid, &mut io, FS_OPEN, &open_args).unwrap() {
                SyscallResult::Handle(slot, generation) => (slot, generation),
                other => panic!("expected a minted handle, got {other:?}"),
            };
        assert!(kernel.captable.stats().opens >= 1);
        assert!(kernel
            .captable
            .authorize(
                crate::kernel::captable::GenerationalHandle { slot, generation },
                pid,
                RIGHT_READ,
                kernel.clock.chronos_tick,
            )
            .is_ok());

        // FS_OPEN on a path with no inode must fail, not silently mint a
        // capability for nothing.
        running(&mut kernel, pid);
        let missing = SyscallArgs {
            path: "/nope.txt".into(),
            rights: RIGHT_READ,
            ..Default::default()
        };
        assert!(matches!(
            dispatch(&mut kernel, pid, &mut io, FS_OPEN, &missing),
            Err(SyscallError::Fs(_))
        ));

        running(&mut kernel, pid);
        let close_args = SyscallArgs {
            cap_slot: slot,
            cap_generation: generation,
            ..Default::default()
        };
        dispatch(&mut kernel, pid, &mut io, FS_CLOSE, &close_args).unwrap();
        assert_eq!(kernel.captable.stats().closes, 1);
        assert_eq!(
            kernel.captable.authorize(
                crate::kernel::captable::GenerationalHandle { slot, generation },
                pid,
                RIGHT_READ,
                kernel.clock.chronos_tick,
            ),
            Err(CapError::InvalidHandle)
        );

        // Closing the same handle twice must fail, not silently succeed.
        running(&mut kernel, pid);
        assert!(matches!(
            dispatch(&mut kernel, pid, &mut io, FS_CLOSE, &close_args),
            Err(SyscallError::Captable(_))
        ));
    }

    #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
    #[test]
    fn fs_rm_invalidates_open_capabilities_via_object_destruction() {
        use crate::kernel::captable::{CapError, GenerationalHandle, RIGHT_READ};

        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let create = SyscallArgs {
            path: "/doomed.txt".into(),
            text: "hi".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_WRITE_FILE, &create).unwrap();

        running(&mut kernel, pid);
        let open_args = SyscallArgs {
            path: "/doomed.txt".into(),
            rights: RIGHT_READ,
            ..Default::default()
        };
        let (slot, generation) =
            match dispatch(&mut kernel, pid, &mut io, FS_OPEN, &open_args).unwrap() {
                SyscallResult::Handle(slot, generation) => (slot, generation),
                other => panic!("expected a minted handle, got {other:?}"),
            };

        running(&mut kernel, pid);
        let rm = SyscallArgs {
            path: "/doomed.txt".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_RM, &rm).unwrap();

        assert_eq!(
            kernel.captable.authorize(
                GenerationalHandle { slot, generation },
                pid,
                RIGHT_READ,
                kernel.clock.chronos_tick,
            ),
            Err(CapError::ObjectGone)
        );
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    #[test]
    fn fs_stat_and_read_serve_from_phase_cache_and_invalidate_on_write() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let create = SyscallArgs {
            path: "/cached.txt".into(),
            text: "hi".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_WRITE_FILE, &create).unwrap();

        let stat = SyscallArgs {
            path: "/cached.txt".into(),
            ..Default::default()
        };
        running(&mut kernel, pid);
        dispatch(&mut kernel, pid, &mut io, FS_STAT, &stat).unwrap();
        assert_eq!(kernel.phase_cache.stats().misses, 1);
        assert_eq!(kernel.phase_cache.stats().hits, 0);

        // Second stat of the same path must be a real cache hit.
        running(&mut kernel, pid);
        dispatch(&mut kernel, pid, &mut io, FS_STAT, &stat).unwrap();
        assert_eq!(kernel.phase_cache.stats().hits, 1);

        // FS_READ_FILE on the same path also hits, and returns real content
        // via `read_file_by_inode` rather than a fresh path resolution.
        let read = SyscallArgs {
            path: "/cached.txt".into(),
            ..Default::default()
        };
        running(&mut kernel, pid);
        let result = dispatch_fs(&mut kernel, pid, &mut io, FS_READ_FILE, &read).unwrap();
        assert_eq!(result, SyscallResult::Bytes(b"hi".to_vec()));
        assert_eq!(kernel.phase_cache.stats().hits, 2);

        // A write must invalidate the cached entry: the next stat is a real
        // trust rejection, not a stale hit.
        running(&mut kernel, pid);
        let rewrite = SyscallArgs {
            path: "/cached.txt".into(),
            text: "bye".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_WRITE_FILE, &rewrite).unwrap();

        running(&mut kernel, pid);
        dispatch(&mut kernel, pid, &mut io, FS_STAT, &stat).unwrap();
        assert_eq!(kernel.phase_cache.stats().trust_rejections, 1);
        assert_eq!(kernel.phase_cache.stats().hits, 2);
    }

    #[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
    #[test]
    fn proc_sleep_blocks_the_caller_in_sched2_and_advance_wakes_it() {
        use crate::kernel::sched2::RunState;

        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let args = SyscallArgs {
            sleep_ticks: 10,
            ..Default::default()
        };
        dispatch(&mut kernel, pid, &mut io, PROC_SLEEP, &args).unwrap();

        let handle = kernel.sched2.handle_for_pid(pid).expect("registered");
        assert_eq!(
            kernel.sched2.run_state(handle),
            Ok(RunState::Blocked(crate::kernel::sched2::WaitReason::Timer))
        );
        let deadline = kernel.sched2.elapsed() + 10;

        let mut woken = crate::platform::Vec::new();
        let fired = kernel.sched2.advance(deadline - 1, &mut woken);
        assert_eq!(fired, 0, "must not wake before the real deadline");
        assert_eq!(
            kernel.sched2.run_state(handle),
            Ok(RunState::Blocked(crate::kernel::sched2::WaitReason::Timer))
        );

        let fired = kernel.sched2.advance(deadline, &mut woken);
        assert_eq!(fired, 1, "must wake exactly at the real deadline");
        assert_eq!(woken.as_slice(), &[pid]);
        assert_eq!(kernel.sched2.run_state(handle), Ok(RunState::Ready));
    }

    #[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
    #[test]
    fn ipc_create_send_recv_round_trip_through_the_full_syscall_path() {
        use crate::kernel::captable::{RIGHT_GRANT, RIGHT_READ, RIGHT_WRITE};

        let mut kernel = Kernel::new();
        let sender = kernel.spawn();
        running(&mut kernel, sender);
        let mut io = IoBuffer::default();

        let create_args = SyscallArgs {
            ipc_capacity: 4,
            rights: RIGHT_READ | RIGHT_WRITE | RIGHT_GRANT,
            ..Default::default()
        };
        let (slot, generation) =
            match dispatch(&mut kernel, sender, &mut io, IPC_CREATE_QUEUE, &create_args).unwrap() {
                SyscallResult::Handle(slot, generation) => (slot, generation),
                other => panic!("expected a minted handle, got {other:?}"),
            };

        running(&mut kernel, sender);
        let send_args = SyscallArgs {
            cap_slot: slot,
            cap_generation: generation,
            text: "hello ipc".into(),
            ..Default::default()
        };
        dispatch(&mut kernel, sender, &mut io, IPC_SEND, &send_args).unwrap();

        // A different real process receives the exact bytes and the real
        // sender's pid -- not the sender reading back its own send. It
        // needs its own capability on the same queue object: capabilities
        // are per-subject, so `share_queue` (mirroring `delegate`) mints
        // one for it rather than letting it reuse the sender's own handle.
        let receiver = kernel.spawn();
        let owner_cap = crate::kernel::captable::GenerationalHandle { slot, generation };
        let receiver_cap = kernel
            .captable
            .share_queue(
                owner_cap,
                sender,
                receiver,
                RIGHT_READ,
                None,
                kernel.clock.chronos_tick,
            )
            .expect("share_queue");
        running(&mut kernel, receiver);
        let recv_args = SyscallArgs {
            cap_slot: receiver_cap.slot,
            cap_generation: receiver_cap.generation,
            ..Default::default()
        };
        match dispatch(&mut kernel, receiver, &mut io, IPC_RECV, &recv_args).unwrap() {
            SyscallResult::Message(from, data) => {
                assert_eq!(from, sender);
                assert_eq!(data, b"hello ipc");
            }
            other => panic!("expected a received message, got {other:?}"),
        }

        // Required proof: recv on an empty queue really blocks the caller
        // in sched2 (not a silent no-op), and a later send really wakes it.
        running(&mut kernel, receiver);
        let err = dispatch(&mut kernel, receiver, &mut io, IPC_RECV, &recv_args).unwrap_err();
        assert_eq!(
            err,
            SyscallError::Ipc(crate::kernel::ipc::IpcError::QueueEmpty)
        );
        let sched2_handle = kernel.sched2.handle_for_pid(receiver).expect("registered");
        assert!(
            matches!(
                kernel.sched2.run_state(sched2_handle),
                Ok(crate::kernel::sched2::RunState::Blocked(
                    crate::kernel::sched2::WaitReason::Ipc(_)
                ))
            ),
            "recv on an empty queue must really block the caller in sched2"
        );

        running(&mut kernel, sender);
        let send_again = SyscallArgs {
            cap_slot: slot,
            cap_generation: generation,
            text: "second message".into(),
            ..Default::default()
        };
        dispatch(&mut kernel, sender, &mut io, IPC_SEND, &send_again).unwrap();
        assert_eq!(
            kernel.sched2.run_state(sched2_handle),
            Ok(crate::kernel::sched2::RunState::Ready),
            "the real send must have woken the real blocked receiver"
        );
    }

    /// Required proof (extmap, subsystem 6): a multi-block file round-trips
    /// exactly through the real `FS_WRITE_FILE`/`FS_READ_FILE` syscalls, and
    /// `FS_RM` really frees its extmap blocks (recursive deletion updates
    /// all references — checked here via `kernel.extmap.stats()`).
    #[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
    #[test]
    fn extmap_multi_block_file_round_trips_through_fs_write_read_rm() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();

        // 3 blocks' worth at extmap's 512-byte block size, deliberately not
        // block-aligned to also exercise the trailing partial block.
        let payload: String = (0..1400u32)
            .map(|i| ((i % 26) as u8 + b'a') as char)
            .collect();
        let write_args = SyscallArgs {
            path: "/big.txt".into(),
            text: payload.clone(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_WRITE_FILE, &write_args).unwrap();
        assert_eq!(kernel.extmap.stats().blocks_live, 3);

        running(&mut kernel, pid);
        let read_args = SyscallArgs {
            path: "/big.txt".into(),
            ..Default::default()
        };
        match dispatch_fs(&mut kernel, pid, &mut io, FS_READ_FILE, &read_args).unwrap() {
            SyscallResult::Bytes(data) => assert_eq!(data, payload.as_bytes()),
            other => panic!("expected file bytes, got {other:?}"),
        }

        running(&mut kernel, pid);
        let rm_args = SyscallArgs {
            path: "/big.txt".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_RM, &rm_args).unwrap();
        assert_eq!(
            kernel.extmap.stats().blocks_live,
            0,
            "FS_RM must free every extmap block the deleted file held"
        );
    }

    #[test]
    fn fs_lock_and_unlock_relocate_the_phase_index_entry() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let create = SyscallArgs {
            path: "/locked.txt".into(),
            text: "secret".into(),
            ..Default::default()
        };
        dispatch_fs(&mut kernel, pid, &mut io, FS_WRITE_FILE, &create).unwrap();
        let inode_id = kernel.fs.inode_id_for_path("/locked.txt").unwrap();
        let sector = kernel.clock.sector_id;

        running(&mut kernel, pid);
        let lock = SyscallArgs {
            path: "/locked.txt".into(),
            ..Default::default()
        };
        dispatch(&mut kernel, pid, &mut io, FS_LOCK, &lock).unwrap();
        assert!(kernel
            .phase_index
            .query_range(PhaseState::Phi3, sector, 0, u64::MAX)
            .contains(&inode_id));
        assert!(!kernel
            .phase_index
            .query_range(PhaseState::Phi1, sector, 0, u64::MAX)
            .contains(&inode_id));

        running(&mut kernel, pid);
        let unlock = SyscallArgs {
            path: "/locked.txt".into(),
            ..Default::default()
        };
        dispatch(&mut kernel, pid, &mut io, FS_UNLOCK, &unlock).unwrap();
        assert!(kernel
            .phase_index
            .query_range(PhaseState::Phi1, sector, 0, u64::MAX)
            .contains(&inode_id));
        assert!(!kernel
            .phase_index
            .query_range(PhaseState::Phi3, sector, 0, u64::MAX)
            .contains(&inode_id));
    }

    #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
    #[test]
    fn fs_read_write_mkdir_rm_fail_closed_without_capability() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        let write = SyscallArgs {
            path: "/a19.txt".into(),
            text: "x".into(),
            ..Default::default()
        };
        assert!(matches!(
            dispatch(&mut kernel, pid, &mut io, FS_WRITE_FILE, &write),
            Err(SyscallError::Captable(_))
        ));
        running(&mut kernel, pid);
        let mkdir = SyscallArgs {
            path: "/a19dir".into(),
            ..Default::default()
        };
        assert!(matches!(
            dispatch(&mut kernel, pid, &mut io, FS_MKDIR, &mkdir),
            Err(SyscallError::Captable(_))
        ));
        dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_WRITE_FILE,
            &write.clone(),
        ).unwrap();
        running(&mut kernel, pid);
        let read = SyscallArgs {
            path: "/a19.txt".into(),
            ..Default::default()
        };
        assert!(matches!(
            dispatch(&mut kernel, pid, &mut io, FS_READ_FILE, &read),
            Err(SyscallError::Captable(_))
        ));
        running(&mut kernel, pid);
        let rm = SyscallArgs {
            path: "/a19.txt".into(),
            ..Default::default()
        };
        assert!(matches!(
            dispatch(&mut kernel, pid, &mut io, FS_RM, &rm),
            Err(SyscallError::Captable(_))
        ));
    }

    #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
    #[test]
    fn fs_read_write_mkdir_rm_succeed_with_matching_file_cap() {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_MKDIR,
            &SyscallArgs {
                path: "/a19ok".into(),
                ..Default::default()
            },
        )
        .unwrap();
        dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_WRITE_FILE,
            &SyscallArgs {
                path: "/a19ok/file.txt".into(),
                text: "ok".into(),
                ..Default::default()
            },
        )
        .unwrap();
        match dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_READ_FILE,
            &SyscallArgs {
                path: "/a19ok/file.txt".into(),
                ..Default::default()
            },
        )
        .unwrap()
        {
            SyscallResult::Bytes(data) => assert_eq!(data, b"ok"),
            other => panic!("expected bytes, got {other:?}"),
        }
        dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_RM,
            &SyscallArgs {
                path: "/a19ok/file.txt".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(kernel.fs.inode_id_for_path("/a19ok/file.txt").is_err());
    }

    #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
    #[test]
    fn fs_write_rejects_read_only_and_queue_caps() {
        use crate::kernel::captable::{RIGHT_READ, RIGHT_WRITE};
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        running(&mut kernel, pid);
        let mut io = IoBuffer::default();
        dispatch_fs(
            &mut kernel,
            pid,
            &mut io,
            FS_WRITE_FILE,
            &SyscallArgs {
                path: "/a19rw.txt".into(),
                text: "n".into(),
                ..Default::default()
            },
        )
        .unwrap();
        running(&mut kernel, pid);
        let open_r = SyscallArgs {
            path: "/a19rw.txt".into(),
            rights: RIGHT_READ,
            ..Default::default()
        };
        let (slot, generation) =
            match dispatch(&mut kernel, pid, &mut io, FS_OPEN, &open_r).unwrap() {
                SyscallResult::Handle(slot, generation) => (slot, generation),
                other => panic!("{other:?}"),
            };
        running(&mut kernel, pid);
        let write = SyscallArgs {
            path: "/a19rw.txt".into(),
            text: "nope".into(),
            cap_slot: slot,
            cap_generation: generation,
            ..Default::default()
        };
        assert!(matches!(
            dispatch(&mut kernel, pid, &mut io, FS_WRITE_FILE, &write),
            Err(SyscallError::Captable(CapError::RightsEscalation))
        ));
        running(&mut kernel, pid);
        let queue = kernel
            .captable
            .open_queue(1, pid, RIGHT_READ | RIGHT_WRITE, None);
        let wrong = SyscallArgs {
            path: "/a19rw.txt".into(),
            text: "nope".into(),
            cap_slot: queue.slot,
            cap_generation: queue.generation,
            ..Default::default()
        };
        assert!(matches!(
            dispatch(&mut kernel, pid, &mut io, FS_WRITE_FILE, &wrong),
            Err(SyscallError::Captable(CapError::WrongObjectKind))
        ));
    }
}
