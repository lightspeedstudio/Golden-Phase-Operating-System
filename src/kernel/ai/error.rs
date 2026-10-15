//! AI control layer errors.

use crate::platform::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiError {
    NotConnected,
    AlreadyConnected,
    ConnectionFailed(String),
    JobNotFound,
    JobTableFull,
    QueueFull,
    PromptTooLarge,
    ResponseTooLarge,
    RequestTimeout,
    ConnectionTimeout,
    MalformedResponse,
    RequestIdMismatch,
    ServerUnavailable,
    DisconnectedDuringJob,
    ToolNotFound,
    ToolDenied,
    Cancelled,
    NotPrivileged,
    InvalidPhase,
}

impl AiError {
    pub fn code(&self) -> u16 {
        match self {
            Self::NotConnected => 1,
            Self::AlreadyConnected => 2,
            Self::ConnectionFailed(_) => 3,
            Self::JobNotFound => 4,
            Self::JobTableFull => 5,
            Self::QueueFull => 6,
            Self::PromptTooLarge => 7,
            Self::ResponseTooLarge => 8,
            Self::RequestTimeout => 9,
            Self::ConnectionTimeout => 10,
            Self::MalformedResponse => 11,
            Self::RequestIdMismatch => 12,
            Self::ServerUnavailable => 13,
            Self::DisconnectedDuringJob => 14,
            Self::ToolNotFound => 15,
            Self::ToolDenied => 16,
            Self::Cancelled => 17,
            Self::NotPrivileged => 18,
            Self::InvalidPhase => 19,
        }
    }

    pub fn reason(&self) -> String {
        match self {
            Self::NotConnected => "AI bridge not connected".into(),
            Self::AlreadyConnected => "AI bridge already connected".into(),
            Self::ConnectionFailed(msg) => format!("AI connection failed: {msg}"),
            Self::JobNotFound => "AI job not found".into(),
            Self::JobTableFull => "AI job table full".into(),
            Self::QueueFull => "AI job queue full".into(),
            Self::PromptTooLarge => "AI prompt exceeds size limit".into(),
            Self::ResponseTooLarge => "AI response exceeds size limit".into(),
            Self::RequestTimeout => "AI request timed out".into(),
            Self::ConnectionTimeout => "AI connection timed out".into(),
            Self::MalformedResponse => "AI server returned malformed response".into(),
            Self::RequestIdMismatch => "AI response request_id mismatch".into(),
            Self::ServerUnavailable => "AI server unavailable".into(),
            Self::DisconnectedDuringJob => "AI server disconnected during job".into(),
            Self::ToolNotFound => "AI tool not found".into(),
            Self::ToolDenied => "AI tool denied".into(),
            Self::Cancelled => "AI job cancelled".into(),
            Self::NotPrivileged => "insufficient privilege for AI permission change".into(),
            Self::InvalidPhase => "invalid AI job phase transition".into(),
        }
    }
}
