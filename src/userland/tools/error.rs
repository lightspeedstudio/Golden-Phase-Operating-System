//! Engineering tool errors.

use crate::platform::String;
use crate::userland::shell::ShellError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    UnknownCommand,
    InvalidArguments,
    UnsupportedTarget,
    AuthorityDenied,
    ObjectNotFound,
    AlreadyExists,
    InvalidPath,
    InvalidState,
    ReadFailed,
    WriteFailed,
    OutputLimitExceeded,
    InputTooLarge,
    QueueFull,
    Timeout,
    Busy,
    NotImplemented,
    VerificationFailed,
    InternalError(String),
}

impl ToolError {
    pub fn message(&self) -> String {
        match self {
            Self::UnknownCommand => "unknown command".into(),
            Self::InvalidArguments => "invalid arguments".into(),
            Self::UnsupportedTarget => "command unavailable on this target".into(),
            Self::AuthorityDenied => "authority denied".into(),
            Self::ObjectNotFound => "object not found".into(),
            Self::AlreadyExists => "already exists".into(),
            Self::InvalidPath => "invalid path".into(),
            Self::InvalidState => "invalid state".into(),
            Self::ReadFailed => "read failed".into(),
            Self::WriteFailed => "write failed".into(),
            Self::OutputLimitExceeded => "output limit exceeded".into(),
            Self::InputTooLarge => "input too large".into(),
            Self::QueueFull => "queue full".into(),
            Self::Timeout => "timeout".into(),
            Self::Busy => "busy".into(),
            Self::NotImplemented => "not implemented".into(),
            Self::VerificationFailed => "verification failed".into(),
            Self::InternalError(msg) => msg.clone(),
        }
    }
}

impl From<ToolError> for ShellError {
    fn from(err: ToolError) -> Self {
        ShellError::Unknown(err.message())
    }
}
