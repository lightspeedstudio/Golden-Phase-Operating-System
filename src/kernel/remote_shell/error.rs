//! Remote shell errors.

use crate::platform::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteShellError {
    HostedOnly,
    InvalidUtf8,
    CommandTooLong,
    AuthorityDenied,
    Shell(String),
    Internal(String),
}

impl RemoteShellError {
    pub fn reason(&self) -> String {
        match self {
            Self::HostedOnly => "remote shell requires hosted build".into(),
            Self::InvalidUtf8 => "invalid UTF-8 in command".into(),
            Self::CommandTooLong => "command exceeds maximum length".into(),
            Self::AuthorityDenied => "authority denied for command".into(),
            Self::Shell(msg) => msg.clone(),
            Self::Internal(msg) => msg.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshError {
    HostedOnly,
    AlreadyRunning,
    NotRunning,
    BindFailed(String),
    HostKeyMissing,
    HostKeyInvalid,
    HostKeyGenerationFailed(String),
    AuthorizedKeysInvalid(String),
    AuthenticationFailed,
    AuthorityDenied,
    TooManySessions,
    RateLimited,
    SessionNotFound,
    ChannelRejected,
    CommandTooLong,
    InvalidUtf8,
    IoFailure(String),
    InternalFailure(String),
    NotPrivileged,
    KeyNotFound,
    KeyAlreadyExists,
}

impl SshError {
    pub fn reason(&self) -> String {
        match self {
            Self::HostedOnly => "SSH server is hosted-only".into(),
            Self::AlreadyRunning => "SSH server already running".into(),
            Self::NotRunning => "SSH server not running".into(),
            Self::BindFailed(msg) => format!("SSH bind failed: {msg}"),
            Self::HostKeyMissing => "SSH host key missing".into(),
            Self::HostKeyInvalid => "SSH host key invalid".into(),
            Self::HostKeyGenerationFailed(msg) => format!("host key generation failed: {msg}"),
            Self::AuthorizedKeysInvalid(msg) => format!("authorized keys invalid: {msg}"),
            Self::AuthenticationFailed => "SSH authentication failed".into(),
            Self::AuthorityDenied => "SSH authority denied".into(),
            Self::TooManySessions => "too many SSH sessions".into(),
            Self::RateLimited => "SSH authentication rate limited".into(),
            Self::SessionNotFound => "SSH session not found".into(),
            Self::ChannelRejected => "SSH channel rejected".into(),
            Self::CommandTooLong => "SSH command too long".into(),
            Self::InvalidUtf8 => "invalid UTF-8".into(),
            Self::IoFailure(msg) => format!("I/O failure: {msg}"),
            Self::InternalFailure(msg) => format!("internal failure: {msg}"),
            Self::NotPrivileged => "insufficient privilege".into(),
            Self::KeyNotFound => "SSH key not found".into(),
            Self::KeyAlreadyExists => "SSH key already registered".into(),
        }
    }
}
