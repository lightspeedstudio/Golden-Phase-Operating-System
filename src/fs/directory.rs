//! Directory entries and path helpers.

use crate::fs::inode::InodeId;
use crate::platform::{String, ToString, Vec};
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub name: String,
    pub inode: InodeId,
}

pub fn normalize_path(path: &str) -> Result<String, PathError> {
    let path = path.trim();
    if !path.starts_with('/') {
        return Err(PathError::NotAbsolute(path.to_string()));
    }
    if path == "/" {
        return Ok(path.to_string());
    }
    let mut parts = Vec::new();
    for part in path.split('/').filter(|p| !p.is_empty()) {
        if part == "." {
            continue;
        }
        if part == ".." {
            return Err(PathError::ParentReference);
        }
        if part.is_empty() {
            return Err(PathError::EmptyComponent);
        }
        parts.push(part.to_string());
    }
    Ok(format!("/{}", parts.join("/")))
}

pub fn split_parent_name(path: &str) -> Result<(String, String), PathError> {
    let normalized = normalize_path(path)?;
    if normalized == "/" {
        return Err(PathError::IsRoot);
    }
    let slash = normalized.rfind('/').expect("non-root path");
    let parent = if slash == 0 {
        "/".to_string()
    } else {
        normalized[..slash].to_string()
    };
    let name = normalized[slash + 1..].to_string();
    Ok((parent, name))
}

pub fn path_components(path: &str) -> Result<Vec<String>, PathError> {
    let normalized = normalize_path(path)?;
    if normalized == "/" {
        return Ok(Vec::new());
    }
    Ok(normalized
        .trim_start_matches('/')
        .split('/')
        .map(str::to_string)
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    NotAbsolute(String),
    EmptyComponent,
    ParentReference,
    IsRoot,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::NotAbsolute(p) => write!(f, "path not absolute: {p}"),
            PathError::EmptyComponent => write!(f, "empty path component"),
            PathError::ParentReference => write!(f, "parent references not supported"),
            PathError::IsRoot => write!(f, "operation invalid on root"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PathError {}
