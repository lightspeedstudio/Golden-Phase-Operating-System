//! Remote shell session — reuses parser and dispatcher.

use crate::kernel::event_log::ProcessId;
use crate::kernel::remote_shell::error::RemoteShellError;
use crate::kernel::remote_shell::policy::{authorize_command, command_category};
use crate::kernel::remote_shell::types::{RemoteAuthority, RemoteCommandResult};
use crate::platform::String;
use crate::stateclock::KairosEventKind;
use crate::userland::hosted::Hosted;
use crate::userland::parser::{parse, Command};
use crate::userland::shell::{ShellAction, ShellError};

pub fn execute_remote_line(
    hosted: &mut Hosted,
    caller: ProcessId,
    authority: RemoteAuthority,
    line: &str,
    max_command_length: usize,
) -> Result<RemoteCommandResult, RemoteShellError> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(RemoteCommandResult {
            output: String::new(),
            exit: false,
        });
    }
    if line.len() > max_command_length {
        return Err(RemoteShellError::CommandTooLong);
    }
    if std::str::from_utf8(line.as_bytes()).is_err() {
        return Err(RemoteShellError::InvalidUtf8);
    }

    let cmd = parse(line);
    if let Command::Unknown(_) = cmd {
        return Ok(RemoteCommandResult {
            output: format!("unknown command: {line}\n"),
            exit: false,
        });
    }
    authorize_command(authority, &cmd)?;
    record_command_received(hosted, &cmd);

    let output_start = hosted.io.output.len();
    let shell = &mut hosted.shell;
    let state = &mut hosted.session_state;
    let mut session = crate::userland::shell::ShellSession {
        kernel: &mut hosted.kernel,
        caller,
        io: &mut hosted.io,
        loader: &hosted.programs,
        vms: &mut hosted.vms,
        state,
    };

    let result = match shell.execute_session(&mut session, line) {
        Ok(ShellAction::Continue)
        | Ok(ShellAction::RunSshServer { .. })
        | Ok(ShellAction::RunAiPhaseLink { .. }) => RemoteCommandResult {
            output: drain_new_output(hosted, output_start),
            exit: false,
        },
        Ok(ShellAction::Exit) => RemoteCommandResult {
            output: format!(
                "{}\nsession closed\n",
                drain_new_output(hosted, output_start)
            ),
            exit: true,
        },
        Err(ShellError::Syscall(err)) => {
            return Err(RemoteShellError::Shell(format!("{err}")));
        }
        Err(ShellError::Run(err)) => {
            return Err(RemoteShellError::Shell(format!("{err:?}")));
        }
        Err(ShellError::Unknown(msg)) | Err(ShellError::Fs(msg)) => RemoteCommandResult {
            output: format!("{msg}\n"),
            exit: false,
        },
    };

    record_command_completed(hosted, &cmd, "ok");
    let _ = command_category(&cmd);
    let _ = authority;
    Ok(result)
}

fn drain_new_output(hosted: &Hosted, start: usize) -> String {
    hosted.io.output[start..].join("")
}

fn record_command_received(hosted: &mut Hosted, cmd: &Command) {
    let category = command_category(cmd);
    crate::stateclock::record_kairos_on_kernel(
        &mut hosted.kernel,
        KairosEventKind::Command,
        format!("remote:{category}"),
    );
}

fn record_command_completed(hosted: &mut Hosted, cmd: &Command, status: &str) {
    let category = command_category(cmd);
    #[cfg(feature = "std")]
    {
        let _ = hosted.kernel.statechain.record_transition(
            crate::kernel::state_proof::ObjectKind::Driver,
            crate::kernel::remote_shell::types::SSH_STATECHAIN_OBJECT_ID,
            crate::kernel::state_root::SubsystemKind::Driver,
            crate::core::gphase_core::PhaseState::Phi2,
            crate::core::gphase_core::PhaseState::Phi0,
            "ssh_command_executed",
            None,
            format!("{category}:{status}").as_bytes(),
        );
    }
    let _ = status;
    let _ = cmd;
}

pub fn render_session_banner(
    authority: RemoteAuthority,
    session_id: u64,
    visibility: &str,
) -> String {
    format!(
        "Golden Phase Operating System Secure Operator Shell\nauthority: {}\nvisibility: {}\nsession: {}\n\ntype help for commands\n\ngpos> ",
        authority.label(),
        visibility,
        session_id
    )
}

pub fn visibility_label(kernel: &crate::kernel::Kernel) -> &'static str {
    if kernel.authority.is_anom_protected() {
        "protected"
    } else {
        "transparent"
    }
}
