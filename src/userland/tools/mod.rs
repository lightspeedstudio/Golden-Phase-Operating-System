//! Engineering terminal toolset — multicall dispatch.

pub mod error;
pub mod limits;
pub mod session_state;
pub mod types;

#[cfg(feature = "bm-engineering-tools")]
pub(crate) mod prelude;

#[cfg(feature = "bm-engineering-tools")]
pub mod benchmark;
#[cfg(feature = "bm-engineering-tools")]
pub mod build;
#[cfg(feature = "bm-engineering-tools")]
pub mod context;
#[cfg(feature = "bm-engineering-tools")]
pub mod debug;
#[cfg(feature = "bm-engineering-tools")]
pub mod device;
#[cfg(feature = "bm-engineering-tools")]
pub mod file;
#[cfg(feature = "bm-engineering-tools")]
pub mod network;
#[cfg(feature = "bm-engineering-tools")]
pub mod output;
#[cfg(feature = "bm-engineering-tools")]
pub mod path_util;
#[cfg(feature = "bm-engineering-tools")]
pub mod phase;
#[cfg(feature = "bm-engineering-tools")]
pub mod process;
#[cfg(feature = "bm-engineering-tools")]
pub mod registry;
#[cfg(feature = "bm-engineering-tools")]
pub mod script;
#[cfg(feature = "bm-engineering-tools")]
pub mod security;
#[cfg(feature = "bm-engineering-tools")]
pub mod shell;
#[cfg(feature = "bm-engineering-tools")]
pub mod system;
#[cfg(feature = "bm-engineering-tools")]
pub mod text;

use crate::userland::shell::{ShellAction, ShellError, ShellSession};

pub use session_state::ShellSessionState;
pub use types::{AuthorityRequirement, CommandGroup, TargetSupport};

#[cfg(feature = "bm-engineering-tools")]
pub use registry::{lookup_descriptor, registry_entries};

#[cfg(feature = "bm-engineering-tools")]
use crate::userland::tools::context::CommandContext;
#[cfg(feature = "bm-engineering-tools")]
use crate::userland::tools::error::ToolError;
#[cfg(feature = "bm-engineering-tools")]
use crate::userland::tools::prelude::{String, Vec};
#[cfg(feature = "bm-engineering-tools")]
use crate::userland::tools::registry::resolve_command;

/// Try to execute a line via the engineering tool registry.
#[cfg(feature = "bm-engineering-tools")]
pub fn try_execute(
    session: &mut ShellSession<'_>,
    line: &str,
) -> Option<Result<ShellAction, ShellError>> {
    let (desc, args) = resolve_command(line)?;
    // Parser/dispatch owns these verbs; the tool registry must not steal them.
    if desc.name == "run" {
        match args.first().map(|s| s.as_str()) {
            None | Some("file") => return None,
            _ => {}
        }
    }
    if matches!(desc.name, "cat" | "mkdir" | "rm") {
        return None;
    }
    if matches!(
        desc.name,
        "phaseperf status" | "phi-stability status" | "ssh status"
    ) {
        return None;
    }
    let status = {
        let mut ctx = CommandContext::new(
            session.kernel,
            session.caller,
            session.io,
            session.loader,
            session.vms,
            &mut session.state,
        );
        match (desc.handler)(&mut ctx, &args) {
            Ok(code) => code,
            Err(err) => {
                let msg = err.message();
                let _ = ctx.writeln_line(&msg);
                1
            }
        }
    };
    session.state.last_status = status;
    session.state.last_command = line.into();
    if desc.name == "exit"
        || shell::is_exit_command(
            &line
                .split_whitespace()
                .map(String::from)
                .collect::<Vec<_>>(),
        )
    {
        return Some(Ok(ShellAction::Exit));
    }
    let _ = status;
    Some(Ok(ShellAction::Continue))
}

#[cfg(not(feature = "bm-engineering-tools"))]
pub fn try_execute(
    _session: &mut ShellSession<'_>,
    _line: &str,
) -> Option<Result<ShellAction, ShellError>> {
    None
}

#[cfg(feature = "bm-engineering-tools")]
pub fn execute_line_inner(
    ctx: &mut CommandContext<'_>,
    line: &str,
    record_history: bool,
) -> Result<i32, ToolError> {
    if record_history {
        ctx.session.record_history(line);
    }
    let (desc, args) = resolve_command(line).ok_or(ToolError::UnknownCommand)?;
    (desc.handler)(ctx, &args)
}
