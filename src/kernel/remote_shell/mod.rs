//! Remote shell module.

pub mod error;
pub mod output;
pub mod policy;
pub mod session;
pub mod transport;
pub mod types;

pub use error::{RemoteShellError, SshError};
pub use output::{ConsoleOutput, ShellOutput, TestBufferOutput};
pub use session::{execute_remote_line, render_session_banner, visibility_label};
pub use transport::{
    available_transports, baremetal_operator_transport, can_host_ssh, transport_label,
};
pub use types::{
    AuthorizedKeyRecord, RemoteAuthority, RemoteCommandResult, RemoteSessionInfo,
    RemoteTransportKind, SshController, SshServerConfig, DEFAULT_SSH_PORT, DEFAULT_SSH_USER,
};

#[cfg(feature = "ssh")]
pub mod audit;
