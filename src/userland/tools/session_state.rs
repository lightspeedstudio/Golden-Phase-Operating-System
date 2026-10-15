//! Per-shell session state.

use crate::platform::{HashMap, String, Vec};
use crate::userland::tools::limits::MAX_HISTORY;

#[derive(Debug, Default, Clone)]
pub struct ShellSessionState {
    pub session_id: u64,
    pub cwd: String,
    pub history: Vec<String>,
    pub env: HashMap<String, String>,
    pub last_status: i32,
    pub last_command: String,
    pub output_limit: usize,
    pub edit_buffer: Option<EditSession>,
}

#[derive(Debug, Clone)]
pub struct EditSession {
    pub path: String,
    pub lines: Vec<String>,
    pub dirty: bool,
}

impl ShellSessionState {
    pub fn new() -> Self {
        Self {
            cwd: "/".into(),
            output_limit: crate::userland::tools::limits::MAX_OUTPUT_BYTES,
            ..Self::default()
        }
    }

    pub fn record_history(&mut self, line: &str) {
        if line.trim().is_empty() {
            return;
        }
        let stored = redact_history_line(line);
        if stored.is_empty() {
            return;
        }
        if self.history.last().map(|s| s.as_str()) == Some(stored.as_str()) {
            return;
        }
        self.history.push(stored);
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

pub fn redact_history_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if lower.contains("anom") && (lower.contains("-p") || lower.contains("passphrase")) {
        return String::new();
    }
    if lower.contains("ssh key add") || lower.contains("private") || lower.contains("password") {
        return String::new();
    }
    if lower.starts_with("ai ask ") {
        return "ai ask <redacted>".into();
    }
    if line.len() > 256 {
        format!("{}...", &line[..253])
    } else {
        line.into()
    }
}
