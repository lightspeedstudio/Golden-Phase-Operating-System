//! Filesystem shell commands via syscalls.

use crate::core::gphase_core::PhaseState;
use crate::kernel::event_log::ProcessId;
use crate::kernel::syscall::{
    IoBuffer, SyscallArgs, SyscallResult, FS_CLOSE, FS_LOCK, FS_LS, FS_MKDIR, FS_OPEN,
    FS_READ_FILE, FS_RENAME, FS_RM, FS_STAT, FS_UNLOCK, FS_WRITE_FILE,
};
use crate::kernel::Kernel;
use crate::platform::{String, ToString};
use crate::userland::syscall::{self, SyscallError};

pub fn phase_label(state: PhaseState) -> &'static str {
    state.kairos_label()
}

fn path_args(path: &str) -> SyscallArgs {
    SyscallArgs {
        path: path.to_string(),
        ..Default::default()
    }
}

pub fn ls(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    path: &str,
) -> Result<String, SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let args = path_args(path);
    let listing = match syscall::dispatch(kernel, caller, io, FS_LS, &args)? {
        SyscallResult::Text(text) => text,
        _ => return Err(SyscallError::BadSyscall),
    };
    syscall::ensure_running(kernel, caller)?;
    syscall::write(kernel, caller, io, &listing)?;
    Ok(listing)
}

fn parent_path(path: &str) -> Result<String, SyscallError> {
    crate::fs::directory::split_parent_name(path)
        .map(|(parent, _)| parent)
        .map_err(|e| SyscallError::Fs(crate::fs::FsError::Path(e)))
}

fn mint_file_cap(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    path: &str,
    mode: &str,
) -> Result<Option<(u32, u32)>, SyscallError> {
    match open(kernel, caller, io, path, mode)? {
        SyscallResult::Handle(slot, generation) => Ok(Some((slot, generation))),
        SyscallResult::Ok => Ok(None),
        _ => Err(SyscallError::BadSyscall),
    }
}

fn drop_file_cap(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    cap: Option<(u32, u32)>,
) {
    if let Some((slot, generation)) = cap {
        let _ = close(kernel, caller, io, slot, generation);
    }
}

fn cap_fields(cap: Option<(u32, u32)>) -> (u32, u32) {
    cap.unwrap_or((0, 0))
}

pub fn cat(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    path: &str,
) -> Result<String, SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let cap = mint_file_cap(kernel, caller, io, path, "r")?;
    let (slot, generation) = cap_fields(cap);
    syscall::ensure_running(kernel, caller)?;
    let args = SyscallArgs {
        path: path.to_string(),
        cap_slot: slot,
        cap_generation: generation,
        ..Default::default()
    };
    let data = match syscall::dispatch(kernel, caller, io, FS_READ_FILE, &args) {
        Ok(SyscallResult::Bytes(bytes)) => bytes,
        Ok(_) => {
            drop_file_cap(kernel, caller, io, cap);
            return Err(SyscallError::BadSyscall);
        }
        Err(err) => {
            drop_file_cap(kernel, caller, io, cap);
            return Err(err);
        }
    };
    drop_file_cap(kernel, caller, io, cap);
    let text = core::str::from_utf8(&data)
        .map(|s| s.to_string())
        .unwrap_or_default();
    syscall::ensure_running(kernel, caller)?;
    syscall::write(kernel, caller, io, &text)?;
    Ok(text)
}

pub fn write_path(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    path: &str,
    text: &str,
) -> Result<(), SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let cap_path = if kernel.fs.inode_id_for_path(path).is_ok() {
        path.to_string()
    } else {
        parent_path(path)?
    };
    let cap = mint_file_cap(kernel, caller, io, &cap_path, "w")?;
    let (slot, generation) = cap_fields(cap);
    syscall::ensure_running(kernel, caller)?;
    let args = SyscallArgs {
        path: path.to_string(),
        text: text.to_string(),
        cap_slot: slot,
        cap_generation: generation,
        ..Default::default()
    };
    let result = syscall::dispatch(kernel, caller, io, FS_WRITE_FILE, &args);
    drop_file_cap(kernel, caller, io, cap);
    match result? {
        SyscallResult::Ok => {}
        _ => return Err(SyscallError::BadSyscall),
    };
    Ok(())
}

pub fn mkdir(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    path: &str,
) -> Result<(), SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let cap_path = parent_path(path)?;
    let cap = mint_file_cap(kernel, caller, io, &cap_path, "w")?;
    let (slot, generation) = cap_fields(cap);
    syscall::ensure_running(kernel, caller)?;
    let args = SyscallArgs {
        path: path.to_string(),
        cap_slot: slot,
        cap_generation: generation,
        ..Default::default()
    };
    let result = syscall::dispatch(kernel, caller, io, FS_MKDIR, &args);
    drop_file_cap(kernel, caller, io, cap);
    match result? {
        SyscallResult::Ok => {}
        _ => return Err(SyscallError::BadSyscall),
    };
    Ok(())
}

pub fn rename(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    from: &str,
    to: &str,
) -> Result<(), SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let args = SyscallArgs {
        path: from.to_string(),
        path_b: to.to_string(),
        ..Default::default()
    };
    match syscall::dispatch(kernel, caller, io, FS_RENAME, &args)? {
        SyscallResult::Ok => {}
        _ => return Err(SyscallError::BadSyscall),
    };
    Ok(())
}

pub fn rm(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    path: &str,
) -> Result<(), SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let cap = mint_file_cap(kernel, caller, io, path, "d")?;
    let (slot, generation) = cap_fields(cap);
    syscall::ensure_running(kernel, caller)?;
    let args = SyscallArgs {
        path: path.to_string(),
        cap_slot: slot,
        cap_generation: generation,
        ..Default::default()
    };
    let result = syscall::dispatch(kernel, caller, io, FS_RM, &args);
    drop_file_cap(kernel, caller, io, cap);
    match result? {
        SyscallResult::Ok => {}
        _ => return Err(SyscallError::BadSyscall),
    };
    Ok(())
}

pub fn stat(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    path: &str,
) -> Result<PhaseState, SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let args = path_args(path);
    let state = match syscall::dispatch(kernel, caller, io, FS_STAT, &args)? {
        SyscallResult::PhaseState(state) => state,
        _ => return Err(SyscallError::BadSyscall),
    };
    let label = phase_label(state);
    syscall::ensure_running(kernel, caller)?;
    syscall::write(kernel, caller, io, label)?;
    Ok(state)
}

pub fn lock(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    path: &str,
) -> Result<(), SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let args = path_args(path);
    match syscall::dispatch(kernel, caller, io, FS_LOCK, &args)? {
        SyscallResult::Ok => {}
        _ => return Err(SyscallError::BadSyscall),
    };
    Ok(())
}

pub fn unlock(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    path: &str,
) -> Result<(), SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let args = path_args(path);
    match syscall::dispatch(kernel, caller, io, FS_UNLOCK, &args)? {
        SyscallResult::Ok => {}
        _ => return Err(SyscallError::BadSyscall),
    };
    Ok(())
}

/// Maps an `open` mode string (e.g. `"r"`, `"rw"`, `"rwd"`) to a
/// `kernel::captable::Rights` bitset, without requiring the `captable`
/// feature at this call site: `r`=read (bit 0), `w`=write (bit 1),
/// `d`=delete (bit 2); unrecognized characters are ignored. Meaningless
/// (rights are never enforced) when the `captable` feature is disabled.
fn rights_from_mode(mode: &str) -> u32 {
    let mut rights = 0u32;
    for ch in mode.chars() {
        rights |= match ch {
            'r' => 1,
            'w' => 2,
            'd' => 4,
            _ => 0,
        };
    }
    rights
}

/// `FS_OPEN`: mint a capability for `path` under `mode`'s requested rights.
/// Returns `SyscallResult::Handle(slot, generation)` when the `captable`
/// feature is enabled, or `SyscallResult::Ok` (no capability minted) when it
/// is disabled — the caller must branch on the result.
pub fn open(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    path: &str,
    mode: &str,
) -> Result<SyscallResult, SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let args = SyscallArgs {
        path: path.to_string(),
        rights: rights_from_mode(mode),
        ..Default::default()
    };
    syscall::dispatch(kernel, caller, io, FS_OPEN, &args)
}

/// `FS_CLOSE`: revoke the capability named `(slot, generation)`.
pub fn close(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    slot: u32,
    generation: u32,
) -> Result<(), SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let args = SyscallArgs {
        cap_slot: slot,
        cap_generation: generation,
        ..Default::default()
    };
    match syscall::dispatch(kernel, caller, io, FS_CLOSE, &args)? {
        SyscallResult::Ok => Ok(()),
        _ => Err(SyscallError::BadSyscall),
    }
}

pub fn format_fs_error(err: &SyscallError) -> String {
    match err {
        SyscallError::Fs(crate::fs::FsError::Locked(_)) => {
            "error: cannot remove locked path".to_string()
        }
        // `FS_RM` on a locked path routes through `kernel::txlog`'s
        // `run_transaction` when the `txlog` feature is enabled (see
        // `kernel::syscall::txlog_rm`), which wraps the exact same
        // `FsError::Locked` one layer deeper as `TxlogError::Fs` instead of
        // returning it bare — unwrap that one layer so this message stays
        // identical regardless of whether `txlog` is on.
        #[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
        SyscallError::Txlog(crate::kernel::txlog::TxlogError::Fs(crate::fs::FsError::Locked(
            _,
        ))) => "error: cannot remove locked path".to_string(),
        SyscallError::Fs(err) => format!("error: {err}"),
        other => format!("error: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::syscall::IoBuffer;
    use crate::kernel::Kernel;

    fn live() -> (Kernel, ProcessId, IoBuffer) {
        let mut kernel = Kernel::new();
        let pid = kernel.spawn();
        kernel.wake(pid).unwrap();
        kernel.schedule(pid).unwrap();
        (kernel, pid, IoBuffer::default())
    }

    #[test]
    fn write_cat_mkdir_rm_reenter_after_open() {
        let (mut kernel, pid, mut io) = live();
        write_path(&mut kernel, pid, &mut io, "/a19.txt", "a19-file-cap-ok").unwrap();
        assert_eq!(
            cat(&mut kernel, pid, &mut io, "/a19.txt").unwrap(),
            "a19-file-cap-ok"
        );
        mkdir(&mut kernel, pid, &mut io, "/a19dir").unwrap();
        write_path(
            &mut kernel,
            pid,
            &mut io,
            "/a19dir/nested.txt",
            "nested-ok",
        )
        .unwrap();
        assert_eq!(
            cat(&mut kernel, pid, &mut io, "/a19dir/nested.txt").unwrap(),
            "nested-ok"
        );
        rm(&mut kernel, pid, &mut io, "/a19.txt").unwrap();
        assert!(cat(&mut kernel, pid, &mut io, "/a19.txt").is_err());
        #[cfg(feature = "captable")]
        {
            assert!(kernel.captable.stats().authorize_ok >= 6);
            assert_eq!(kernel.captable.capability_count(), 0);
        }
    }
}
