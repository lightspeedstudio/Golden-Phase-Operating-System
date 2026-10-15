//! Remote shell types and configuration.

use crate::platform::{String, Vec};

pub const DEFAULT_SSH_PORT: u16 = 2222;
pub const DEFAULT_SSH_USER: &str = "kairos";
pub const DEFAULT_MAX_SESSIONS: usize = 8;
pub const DEFAULT_MAX_AUTH_ATTEMPTS: u8 = 5;
pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 900;
pub const DEFAULT_MAX_COMMAND_LENGTH: usize = 4096;
pub const SSH_STATECHAIN_OBJECT_ID: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteAuthority {
    Viewer,
    Operator,
    Root,
    AnomAdministrator,
}

impl RemoteAuthority {
    pub fn label(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Operator => "operator",
            Self::Root => "root",
            Self::AnomAdministrator => "anom-administrator",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteTransportKind {
    #[cfg(feature = "ssh")]
    Ssh,
    PhaseLinkGateway,
    UartConsole,
    RawTcp,
}

#[derive(Debug, Clone)]
pub struct SshServerConfig {
    pub listen_address: String,
    pub port: u16,
    pub host_key_path: String,
    pub authorized_keys_path: String,
    pub key_registry_path: String,
    pub allow_password: bool,
    pub max_sessions: usize,
    pub max_auth_attempts: u8,
    pub idle_timeout_seconds: u64,
    pub max_command_length: usize,
    pub banner: String,
}

impl Default for SshServerConfig {
    fn default() -> Self {
        let base = default_data_dir();
        Self {
            listen_address: "127.0.0.1".into(),
            port: DEFAULT_SSH_PORT,
            host_key_path: format!("{base}/ssh/ssh_host_ed25519_key"),
            authorized_keys_path: format!("{base}/ssh/authorized_keys"),
            key_registry_path: format!("{base}/ssh/key_registry.json"),
            allow_password: false,
            max_sessions: DEFAULT_MAX_SESSIONS,
            max_auth_attempts: DEFAULT_MAX_AUTH_ATTEMPTS,
            idle_timeout_seconds: DEFAULT_IDLE_TIMEOUT_SECS,
            max_command_length: DEFAULT_MAX_COMMAND_LENGTH,
            banner: "KairOS Secure Operator Shell".into(),
        }
    }
}

pub fn default_data_dir() -> String {
    #[cfg(feature = "std")]
    {
        if let Ok(home) = std::env::var("KAIROS_DATA_DIR") {
            return home;
        }
        if let Ok(home) = std::env::var("USERPROFILE") {
            return format!("{home}/.kairos");
        }
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/.kairos");
        }
    }
    ".kairos".into()
}

#[derive(Debug, Clone)]
pub struct AuthorizedKeyRecord {
    pub fingerprint: String,
    pub public_key_line: String,
    pub operator_identity: String,
    pub authority: RemoteAuthority,
    pub enabled: bool,
    pub created_tick: u64,
    pub last_used_tick: Option<u64>,
    pub expires_tick: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct RemoteSessionInfo {
    pub session_id: u64,
    pub key_fingerprint: String,
    pub authority: RemoteAuthority,
    pub remote_address: String,
    pub connected_tick: u64,
    pub last_activity_tick: u64,
    pub command_count: u64,
    pub failed_command_count: u64,
    pub interactive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCommandResult {
    pub output: String,
    pub exit: bool,
}

#[derive(Debug, Default, Clone)]
pub struct SshController {
    pub running: bool,
    pub listen_address: String,
    pub port: u16,
    pub host_key_fingerprint: String,
    pub allow_password: bool,
    pub sessions: Vec<RemoteSessionInfo>,
    pub next_session_id: u64,
    pub auth_failures_by_addr: Vec<(String, u8)>,
    pub last_error: Option<String>,
    pub keys: Vec<AuthorizedKeyRecord>,
}

impl SshController {
    pub fn new() -> Self {
        Self {
            next_session_id: 1,
            ..Self::default()
        }
    }

    pub fn transport_status(&self) -> String {
        #[cfg(feature = "ssh")]
        if self.running {
            return format!(
                "SSH server running on {}:{}",
                self.listen_address, self.port
            );
        }
        #[cfg(not(feature = "ssh"))]
        let _ = &self.listen_address;
        "hosted-only SSH server unavailable on this target".into()
    }
}
