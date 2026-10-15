//! SSH host key load, generate, fingerprint.

use crate::hosted::ssh::config::SshServerConfig;
use crate::kernel::remote_shell::error::SshError;
use russh_keys::key::KeyPair;
use russh_keys::{decode_secret_key, encode_pkcs8_pem};
use std::fs;
use std::io::Cursor;
use std::path::Path;

pub struct HostKeyMaterial {
    pub key_pair: KeyPair,
    pub fingerprint_sha256: String,
}

pub fn ensure_host_key(config: &SshServerConfig) -> Result<HostKeyMaterial, SshError> {
    if let Some(parent) = Path::new(&config.host_key_path).parent() {
        fs::create_dir_all(parent).map_err(|e| SshError::IoFailure(e.to_string()))?;
    }
    if Path::new(&config.host_key_path).exists() {
        return load_host_key(&config.host_key_path);
    }
    generate_host_key(&config.host_key_path)
}

pub fn load_host_key(path: &str) -> Result<HostKeyMaterial, SshError> {
    let pem = fs::read_to_string(path).map_err(|_| SshError::HostKeyMissing)?;
    let key_pair = decode_secret_key(&pem, None).map_err(|_| SshError::HostKeyInvalid)?;
    Ok(HostKeyMaterial {
        fingerprint_sha256: fingerprint_for_key(&key_pair)?,
        key_pair,
    })
}

pub fn generate_host_key(path: &str) -> Result<HostKeyMaterial, SshError> {
    let key_pair = KeyPair::generate_ed25519()
        .ok_or_else(|| SshError::HostKeyGenerationFailed("ed25519".into()))?;
    let mut buf = Cursor::new(Vec::new());
    encode_pkcs8_pem(&key_pair, &mut buf)
        .map_err(|e| SshError::HostKeyGenerationFailed(e.to_string()))?;
    fs::write(path, buf.into_inner()).map_err(|e| SshError::IoFailure(e.to_string()))?;
    Ok(HostKeyMaterial {
        fingerprint_sha256: fingerprint_for_key(&key_pair)?,
        key_pair,
    })
}

pub fn fingerprint_for_key(key_pair: &KeyPair) -> Result<String, SshError> {
    let public = key_pair
        .clone_public_key()
        .map_err(|_| SshError::HostKeyInvalid)?;
    Ok(public.fingerprint())
}
