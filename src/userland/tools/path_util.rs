//! Path resolution relative to shell cwd.

use crate::fs::directory::normalize_path;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::limits::MAX_PATH_LEN;
use crate::userland::tools::prelude::*;

pub fn resolve_path(cwd: &str, path: &str) -> Result<String, ToolError> {
    if path.is_empty() {
        return Err(ToolError::InvalidPath);
    }
    let joined = if path.starts_with('/') {
        path.to_string()
    } else if cwd == "/" {
        format!("/{path}")
    } else {
        format!("{cwd}/{path}")
    };
    if joined.len() > MAX_PATH_LEN {
        return Err(ToolError::InvalidPath);
    }
    normalize_path(&joined).map_err(|_| ToolError::InvalidPath)
}

pub fn parent_path(path: &str) -> Option<String> {
    if path == "/" {
        return None;
    }
    let trimmed = path.trim_end_matches('/');
    let Some((parent, _)) = trimmed.rsplit_once('/') else {
        return Some("/".into());
    };
    if parent.is_empty() {
        Some("/".into())
    } else {
        Some(parent.into())
    }
}
