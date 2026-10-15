//! SSH authorized key registry and authentication.

use crate::kernel::remote_shell::audit::record_ssh_event;
use crate::kernel::remote_shell::error::SshError;
use crate::kernel::remote_shell::types::{AuthorizedKeyRecord, RemoteAuthority, SshServerConfig};
use crate::kernel::Kernel;
use crate::platform::{String, Vec};
use russh_keys::key::PublicKey;
use russh_keys::parse_public_key_base64;
use std::fs;
use std::path::Path;

pub fn load_key_registry(config: &SshServerConfig) -> Result<Vec<AuthorizedKeyRecord>, SshError> {
    if !Path::new(&config.key_registry_path).exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&config.key_registry_path)
        .map_err(|e| SshError::AuthorizedKeysInvalid(e.to_string()))?;
    parse_registry(&text)
}

pub fn save_key_registry(
    config: &SshServerConfig,
    keys: &[AuthorizedKeyRecord],
) -> Result<(), SshError> {
    if let Some(parent) = Path::new(&config.key_registry_path).parent() {
        fs::create_dir_all(parent).map_err(|e| SshError::IoFailure(e.to_string()))?;
    }
    let mut out = String::new();
    for key in keys {
        out.push_str(&format!(
            "fingerprint={};identity={};authority={};enabled={};created={};last_used={};expires={};line={}\n",
            key.fingerprint,
            key.operator_identity,
            key.authority.label(),
            key.enabled,
            key.created_tick,
            key.last_used_tick.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            key.expires_tick.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            key.public_key_line
        ));
    }
    fs::write(&config.key_registry_path, out).map_err(|e| SshError::IoFailure(e.to_string()))?;
    sync_authorized_keys_file(config, keys)
}

fn sync_authorized_keys_file(
    config: &SshServerConfig,
    keys: &[AuthorizedKeyRecord],
) -> Result<(), SshError> {
    if let Some(parent) = Path::new(&config.authorized_keys_path).parent() {
        fs::create_dir_all(parent).map_err(|e| SshError::IoFailure(e.to_string()))?;
    }
    let mut out = String::new();
    for key in keys.iter().filter(|k| k.enabled) {
        out.push_str(&key.public_key_line);
        out.push('\n');
    }
    fs::write(&config.authorized_keys_path, out).map_err(|e| SshError::IoFailure(e.to_string()))?;
    Ok(())
}

pub fn parse_registry(text: &str) -> Result<Vec<AuthorizedKeyRecord>, SshError> {
    let mut keys = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        keys.push(parse_registry_line(line)?);
    }
    Ok(keys)
}

fn parse_registry_line(line: &str) -> Result<AuthorizedKeyRecord, SshError> {
    let mut fingerprint = String::new();
    let mut operator_identity = String::new();
    let mut authority = RemoteAuthority::Operator;
    let mut enabled = true;
    let mut created_tick = 0;
    let mut last_used_tick = None;
    let mut expires_tick = None;
    let mut public_key_line = String::new();
    for part in line.split(';') {
        if let Some((k, v)) = part.split_once('=') {
            match k {
                "fingerprint" => fingerprint = v.into(),
                "identity" => operator_identity = v.into(),
                "authority" => {
                    authority = match v {
                        "viewer" => RemoteAuthority::Viewer,
                        "root" => RemoteAuthority::Root,
                        "anom-administrator" => RemoteAuthority::AnomAdministrator,
                        _ => RemoteAuthority::Operator,
                    };
                }
                "enabled" => enabled = v == "true",
                "created" => created_tick = v.parse().unwrap_or(0),
                "last_used" => {
                    if v != "-" {
                        last_used_tick = v.parse().ok();
                    }
                }
                "expires" => {
                    if v != "-" {
                        expires_tick = v.parse().ok();
                    }
                }
                "line" => public_key_line = v.into(),
                _ => {}
            }
        }
    }
    if fingerprint.is_empty() || public_key_line.is_empty() {
        return Err(SshError::AuthorizedKeysInvalid(
            "incomplete key record".into(),
        ));
    }
    Ok(AuthorizedKeyRecord {
        fingerprint,
        public_key_line,
        operator_identity,
        authority,
        enabled,
        created_tick,
        last_used_tick,
        expires_tick,
    })
}

pub fn fingerprint_public_key(key: &PublicKey) -> String {
    key.fingerprint()
}

pub fn parse_authorized_line(line: &str) -> Result<PublicKey, SshError> {
    let mut parts = line.split_whitespace();
    let _alg = parts.next();
    let key_b64 = parts
        .next()
        .ok_or_else(|| SshError::AuthorizedKeysInvalid("missing key blob".into()))?;
    parse_public_key_base64(key_b64).map_err(|e| SshError::AuthorizedKeysInvalid(e.to_string()))
}

pub fn authenticate_publickey(
    kernel: &mut Kernel,
    config: &SshServerConfig,
    username: &str,
    key: &PublicKey,
    tick: u64,
) -> Result<AuthorizedKeyRecord, SshError> {
    if username != crate::kernel::remote_shell::types::DEFAULT_SSH_USER {
        record_ssh_event(kernel, "ssh_auth_failure", b"bad-user");
        return Err(SshError::AuthenticationFailed);
    }
    let fp = fingerprint_public_key(key);
    let mut keys = load_key_registry(config)?;
    let Some(record) = keys.iter_mut().find(|k| k.fingerprint == fp) else {
        record_ssh_event(kernel, "ssh_auth_failure", b"unknown-key");
        return Err(SshError::AuthenticationFailed);
    };
    if !record.enabled {
        record_ssh_event(kernel, "ssh_auth_failure", b"disabled-key");
        return Err(SshError::AuthenticationFailed);
    }
    if let Some(exp) = record.expires_tick {
        if tick > exp {
            record_ssh_event(kernel, "ssh_auth_failure", b"expired-key");
            return Err(SshError::AuthenticationFailed);
        }
    }
    record.last_used_tick = Some(tick);
    let out = record.clone();
    save_key_registry(config, &keys)?;
    kernel.ssh.keys = keys;
    record_ssh_event(kernel, "ssh_auth_success", fp.as_bytes());
    Ok(out)
}

pub fn authenticate_password(
    kernel: &mut Kernel,
    config: &SshServerConfig,
    password: &str,
) -> Result<RemoteAuthority, SshError> {
    if !config.allow_password {
        record_ssh_event(kernel, "ssh_auth_failure", b"password-disabled");
        return Err(SshError::AuthenticationFailed);
    }
    let expected =
        std::env::var("KAIROS_SSH_PASSWORD").map_err(|_| SshError::AuthenticationFailed)?;
    if expected != password {
        record_ssh_event(kernel, "ssh_auth_failure", b"bad-password");
        return Err(SshError::AuthenticationFailed);
    }
    record_ssh_event(kernel, "ssh_auth_success", b"password-dev");
    Ok(RemoteAuthority::Operator)
}

pub fn add_key(
    config: &SshServerConfig,
    kernel: &mut Kernel,
    public_key_line: &str,
    authority: RemoteAuthority,
    identity: &str,
    tick: u64,
) -> Result<String, SshError> {
    let public = parse_authorized_line(public_key_line)?;
    let fingerprint = fingerprint_public_key(&public);
    let mut keys = load_key_registry(config)?;
    if keys.iter().any(|k| k.fingerprint == fingerprint) {
        return Err(SshError::KeyAlreadyExists);
    }
    keys.push(AuthorizedKeyRecord {
        fingerprint: fingerprint.clone(),
        public_key_line: public_key_line.into(),
        operator_identity: identity.into(),
        authority,
        enabled: true,
        created_tick: tick,
        last_used_tick: None,
        expires_tick: None,
    });
    save_key_registry(config, &keys)?;
    kernel.ssh.keys = keys;
    record_ssh_event(kernel, "ssh_key_added", fingerprint.as_bytes());
    Ok(fingerprint)
}
