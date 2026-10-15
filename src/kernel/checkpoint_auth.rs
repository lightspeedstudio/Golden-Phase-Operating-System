//! Secret-backed GPC1 checkpoint authentication.
//!
//! V1/V2 HMAC in [`crate::kernel::statechain_checkpoint`] stays
//! integrity-only and is never used to create authenticated checkpoints.
//! PSC1 and identity certificates stay Ed25519. This HMAC tag is 32 bytes
//! and is not an Ed25519 signature.
//!
//! `HardwareBoundSecret` is unavailable on PZ7020: no eFUSE, BBRAM, PUF,
//! or DNA keystore is configured.

use crate::kernel::crypto::keyring::hkdf_sha256;
use crate::kernel::hash::{tagged_hash, StateHash};
use crate::kernel::statechain_checkpoint::prime_log_for_equation;
use crate::platform::{String, ToString, Vec};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub const HKDF_SALT: &[u8] = b"GPOS/checkpoint/v1";
pub const PAYLOAD_TAG: &str = "GPOS/checkpoint/payload/v1";
pub const PREV_ROOT_TAG: &str = "GPOS/checkpoint/prev/v1";
pub const MAGIC: &[u8; 4] = b"GPC1";
pub const FORMAT_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointKeyBackend {
    AuthUnavailable,
    EphemeralProvisionedSecret,
    HardwareBoundSecret,
    LegacyIntegrityOnly,
}

impl CheckpointKeyBackend {
    pub fn status_label(self) -> &'static str {
        match self {
            Self::AuthUnavailable => "AUTH_UNAVAILABLE",
            Self::EphemeralProvisionedSecret => "EPHEMERAL_PROVISIONED_SECRET",
            Self::HardwareBoundSecret => "HARDWARE_BOUND_SECRET",
            Self::LegacyIntegrityOnly => "LEGACY_INTEGRITY_ONLY",
        }
    }

    pub fn allows_authenticated_seal(self) -> bool {
        matches!(self, Self::EphemeralProvisionedSecret)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyError {
    AuthUnavailable,
    Unavailable,
    NotProvisioned,
    BadSecret,
    ArgvSecretRefused,
    Rollback,
    Empty,
    BadSignature,
    Truncated,
    Extended,
    UnknownVersion,
}

impl KeyError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthUnavailable | Self::Unavailable | Self::NotProvisioned => "AUTH_UNAVAILABLE",
            Self::BadSecret => "BAD_SECRET",
            Self::ArgvSecretRefused => "ARGV_SECRET_REFUSED",
            Self::Rollback => "ROLLBACK",
            Self::Empty => "EMPTY",
            Self::BadSignature => "BAD_SIGNATURE",
            Self::Truncated => "TRUNCATED",
            Self::Extended => "EXTENDED",
            Self::UnknownVersion => "UNKNOWN_VERSION",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckpointContext {
    pub device_id: StateHash,
    pub boot_session_root: StateHash,
    pub key_epoch: u32,
    pub key_backend: CheckpointKeyBackend,
}

pub trait CheckpointKeySource {
    fn backend_id(&self) -> CheckpointKeyBackend;
    fn derive_checkpoint_key(
        &self,
        context: &CheckpointContext,
        output: &mut [u8; 32],
    ) -> Result<(), KeyError>;
}

pub struct LegacyIntegrityOnly {
    pub equation_root: StateHash,
}

impl CheckpointKeySource for LegacyIntegrityOnly {
    fn backend_id(&self) -> CheckpointKeyBackend {
        CheckpointKeyBackend::LegacyIntegrityOnly
    }

    fn derive_checkpoint_key(
        &self,
        _context: &CheckpointContext,
        output: &mut [u8; 32],
    ) -> Result<(), KeyError> {
        let key = crate::kernel::statechain_checkpoint::derive_signing_key(&self.equation_root);
        output.copy_from_slice(&key);
        Ok(())
    }
}

pub struct HardwareBoundSecret;

impl CheckpointKeySource for HardwareBoundSecret {
    fn backend_id(&self) -> CheckpointKeyBackend {
        CheckpointKeyBackend::HardwareBoundSecret
    }

    fn derive_checkpoint_key(
        &self,
        _context: &CheckpointContext,
        _output: &mut [u8; 32],
    ) -> Result<(), KeyError> {
        Err(KeyError::Unavailable)
    }
}

pub struct EphemeralProvisionedSecret<'a> {
    secret: &'a [u8; 32],
}

impl<'a> EphemeralProvisionedSecret<'a> {
    pub fn new(secret: &'a [u8; 32]) -> Self {
        Self { secret }
    }
}

impl CheckpointKeySource for EphemeralProvisionedSecret<'_> {
    fn backend_id(&self) -> CheckpointKeyBackend {
        CheckpointKeyBackend::EphemeralProvisionedSecret
    }

    fn derive_checkpoint_key(
        &self,
        context: &CheckpointContext,
        output: &mut [u8; 32],
    ) -> Result<(), KeyError> {
        derive_k_checkpoint(self.secret, context, output);
        Ok(())
    }
}

pub fn derive_k_checkpoint(
    device_secret: &[u8; 32],
    context: &CheckpointContext,
    output: &mut [u8; 32],
) {
    let mut info = Vec::new();
    push_lp(&mut info, &context.device_id);
    push_lp(&mut info, &context.boot_session_root);
    push_lp(&mut info, &context.key_epoch.to_le_bytes());
    push_lp(&mut info, context.key_backend.status_label().as_bytes());
    let mut key = hkdf_sha256(device_secret, HKDF_SALT, &info);
    output.copy_from_slice(&key);
    zeroize(&mut key);
}

pub fn hmac_checkpoint(key: &[u8; 32], body: &[u8]) -> StateHash {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts 32-byte keys");
    mac.update(body);
    mac.finalize().into_bytes().into()
}

pub fn payload_root(prime_log: &StateHash, app_registry_root: &StateHash) -> StateHash {
    tagged_hash(
        PAYLOAD_TAG,
        &[
            &(32u32).to_le_bytes(),
            prime_log,
            &(32u32).to_le_bytes(),
            app_registry_root,
        ],
    )
}

pub fn previous_checkpoint_root(mac_body: &[u8]) -> StateHash {
    tagged_hash(PREV_ROOT_TAG, &[mac_body])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedCheckpointFields {
    pub version: u16,
    pub sequence: u64,
    pub previous_root: StateHash,
    pub kernel_root: StateHash,
    pub equation_root: StateHash,
    pub boot_session_root: StateHash,
    pub chronos_tick: u64,
    pub kairos_tick: u64,
    pub key_epoch: u32,
    pub key_backend: CheckpointKeyBackend,
    pub payload_root: StateHash,
    pub device_id: StateHash,
}

pub fn encode_mac_body(fields: &AuthenticatedCheckpointFields) -> Vec<u8> {
    let mut body = Vec::new();
    push_lp(&mut body, MAGIC);
    push_lp(&mut body, &fields.version.to_le_bytes());
    push_lp(&mut body, &fields.sequence.to_le_bytes());
    push_lp(&mut body, &fields.previous_root);
    push_lp(&mut body, &fields.kernel_root);
    push_lp(&mut body, &fields.equation_root);
    push_lp(&mut body, &fields.boot_session_root);
    push_lp(&mut body, &fields.chronos_tick.to_le_bytes());
    push_lp(&mut body, &fields.kairos_tick.to_le_bytes());
    push_lp(&mut body, &fields.key_epoch.to_le_bytes());
    push_lp(&mut body, fields.key_backend.status_label().as_bytes());
    push_lp(&mut body, &fields.payload_root);
    push_lp(&mut body, &fields.device_id);
    body
}

pub fn encode_record(fields: &AuthenticatedCheckpointFields, auth_tag: &StateHash) -> Vec<u8> {
    let mut record = encode_mac_body(fields);
    push_lp(&mut record, auth_tag);
    record
}

pub fn decode_record(bytes: &[u8]) -> Result<(AuthenticatedCheckpointFields, StateHash), KeyError> {
    let mut r = LpReader { src: bytes, off: 0 };
    let magic = r.lp()?;
    if magic != MAGIC {
        return Err(KeyError::UnknownVersion);
    }
    let version_bytes = r.lp()?;
    if version_bytes.len() != 2 {
        return Err(KeyError::Truncated);
    }
    let version = u16::from_le_bytes(version_bytes.try_into().map_err(|_| KeyError::Truncated)?);
    if version != FORMAT_VERSION {
        return Err(KeyError::UnknownVersion);
    }
    let fields = AuthenticatedCheckpointFields {
        version,
        sequence: r.u64_lp()?,
        previous_root: r.hash_lp()?,
        kernel_root: r.hash_lp()?,
        equation_root: r.hash_lp()?,
        boot_session_root: r.hash_lp()?,
        chronos_tick: r.u64_lp()?,
        kairos_tick: r.u64_lp()?,
        key_epoch: r.u32_lp()?,
        key_backend: parse_backend_label(r.lp()?)?,
        payload_root: r.hash_lp()?,
        device_id: r.hash_lp()?,
    };
    let tag_field = r.lp()?;
    if tag_field.len() != 32 {
        if tag_field.len() < 32 {
            return Err(KeyError::Truncated);
        }
        return Err(KeyError::Extended);
    }
    let mut tag = [0u8; 32];
    tag.copy_from_slice(tag_field);
    if r.off != bytes.len() {
        return Err(KeyError::Extended);
    }
    Ok((fields, tag))
}

fn parse_backend_label(label: &[u8]) -> Result<CheckpointKeyBackend, KeyError> {
    match label {
        b"AUTH_UNAVAILABLE" => Ok(CheckpointKeyBackend::AuthUnavailable),
        b"EPHEMERAL_PROVISIONED_SECRET" => Ok(CheckpointKeyBackend::EphemeralProvisionedSecret),
        b"HARDWARE_BOUND_SECRET" => Ok(CheckpointKeyBackend::HardwareBoundSecret),
        b"LEGACY_INTEGRITY_ONLY" => Ok(CheckpointKeyBackend::LegacyIntegrityOnly),
        _ => Err(KeyError::UnknownVersion),
    }
}

pub fn verify_record(
    source: &dyn CheckpointKeySource,
    context: &CheckpointContext,
    bytes: &[u8],
) -> Result<AuthenticatedCheckpointFields, KeyError> {
    let (fields, tag) = decode_record(bytes)?;
    if fields.key_backend != source.backend_id() || fields.key_backend != context.key_backend {
        return Err(KeyError::BadSignature);
    }
    if fields.device_id != context.device_id
        || fields.boot_session_root != context.boot_session_root
        || fields.key_epoch != context.key_epoch
    {
        return Err(KeyError::BadSignature);
    }
    let body = encode_mac_body(&fields);
    let mut key = [0u8; 32];
    source.derive_checkpoint_key(context, &mut key)?;
    let expected = hmac_checkpoint(&key, &body);
    zeroize(&mut key);
    if !ct_eq(&expected, &tag) {
        return Err(KeyError::BadSignature);
    }
    if fields.kernel_root == [0u8; 32] || fields.equation_root == [0u8; 32] {
        return Err(KeyError::Empty);
    }
    Ok(fields)
}

pub fn seal_record(
    source: &dyn CheckpointKeySource,
    context: &CheckpointContext,
    fields: &AuthenticatedCheckpointFields,
) -> Result<Vec<u8>, KeyError> {
    if !source.backend_id().allows_authenticated_seal()
        || !context.key_backend.allows_authenticated_seal()
        || fields.key_backend != source.backend_id()
    {
        return Err(KeyError::AuthUnavailable);
    }
    if fields.kernel_root == [0u8; 32] || fields.equation_root == [0u8; 32] {
        return Err(KeyError::Empty);
    }
    let body = encode_mac_body(fields);
    let mut key = [0u8; 32];
    source.derive_checkpoint_key(context, &mut key)?;
    let tag = hmac_checkpoint(&key, &body);
    zeroize(&mut key);
    Ok(encode_record(fields, &tag))
}

#[derive(Debug)]
pub struct CheckpointAuthController {
    backend: CheckpointKeyBackend,
    device_id: StateHash,
    key_epoch: u32,
    last_accepted: Option<u64>,
    last_prev_root: StateHash,
    last_record: Option<Vec<u8>>,
    next_sequence: u64,
    secret: Option<[u8; 32]>,
}

impl Default for CheckpointAuthController {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckpointAuthController {
    pub fn new() -> Self {
        Self {
            backend: CheckpointKeyBackend::AuthUnavailable,
            device_id: tagged_hash("GPOS/checkpoint/device-id/v1", &[b"SOFTWARE_FIXTURE"]),
            key_epoch: 0,
            last_accepted: None,
            last_prev_root: [0u8; 32],
            last_record: None,
            next_sequence: 0,
            secret: None,
        }
    }

    pub fn backend(&self) -> CheckpointKeyBackend {
        self.backend
    }

    pub fn device_id(&self) -> StateHash {
        self.device_id
    }

    pub fn key_epoch(&self) -> u32 {
        self.key_epoch
    }

    pub fn last_record(&self) -> Option<&[u8]> {
        self.last_record.as_deref()
    }

    pub fn set_device_id(&mut self, device_id: StateHash) {
        self.device_id = device_id;
    }

    pub fn context(&self, boot_session_root: StateHash) -> CheckpointContext {
        CheckpointContext {
            device_id: self.device_id,
            boot_session_root,
            key_epoch: self.key_epoch,
            key_backend: self.backend,
        }
    }

    pub fn status_text(&self) -> String {
        let mut out = String::new();
        out.push_str("KEY_BACKEND: ");
        out.push_str(self.backend.status_label());
        out.push('\n');
        out.push_str("HARDWARE_BOUND: NO\n");
        out.push_str("DEVICE_ID_SOURCE: SOFTWARE_FIXTURE\n");
        out.push_str("CLASSIFICATION: ");
        match self.backend {
            CheckpointKeyBackend::AuthUnavailable => out.push_str("unavailable\n"),
            CheckpointKeyBackend::EphemeralProvisionedSecret => {
                out.push_str("authenticated-ephemeral-ram-secret\n");
                out.push_str("NOTE: not device-bound; not persistent across reboot\n");
            }
            CheckpointKeyBackend::HardwareBoundSecret => out.push_str("unavailable\n"),
            CheckpointKeyBackend::LegacyIntegrityOnly => out.push_str("integrity-only\n"),
        }
        out.push_str("KEY_EPOCH: ");
        out.push_str(&self.key_epoch.to_string());
        out.push('\n');
        out.push_str("DEVICE_ID: ");
        out.push_str(&short_hex(&self.device_id));
        out.push('\n');
        match self.last_accepted {
            Some(seq) => {
                out.push_str("LAST_ACCEPTED_SEQUENCE: ");
                out.push_str(&seq.to_string());
                out.push('\n');
            }
            None => out.push_str("LAST_ACCEPTED_SEQUENCE: none\n"),
        }
        out.push_str("CROSS_REBOOT_ROLLBACK: unsupported\n");
        out
    }

    pub fn hardware_status_text(&self) -> String {
        let mut out = String::new();
        out.push_str("KEY_BACKEND: HARDWARE_BOUND_SECRET\n");
        out.push_str("HARDWARE_BOUND: NO\n");
        out.push_str("KEY_ERROR: UNAVAILABLE\n");
        out.push_str("AUTH_UNAVAILABLE\n");
        out.push_str(
            "NOTE: Adversarial checkpoint authentication requires custom-board key storage.\n",
        );
        out
    }

    pub fn provision_secret(&mut self, secret: &[u8; 32]) -> Result<(), KeyError> {
        if secret.iter().all(|b| *b == 0) {
            return Err(KeyError::BadSecret);
        }
        if self.secret.is_some() {
            self.key_epoch = self.key_epoch.saturating_add(1);
        }
        self.secret = Some(*secret);
        self.backend = CheckpointKeyBackend::EphemeralProvisionedSecret;
        Ok(())
    }

    pub fn provision_from_line(&mut self, line: &str) -> Result<(), KeyError> {
        let mut secret = parse_development_secret(line)?;
        let result = self.provision_secret(&secret);
        zeroize(&mut secret);
        result
    }

    pub fn seal(
        &mut self,
        boot_session_root: StateHash,
        kernel_root: StateHash,
        equation_root: StateHash,
        chronos_tick: u64,
        kairos_tick: u64,
        app_registry_root: StateHash,
    ) -> Result<Vec<u8>, KeyError> {
        let secret = self.secret.as_ref().ok_or(KeyError::AuthUnavailable)?;
        if self.backend != CheckpointKeyBackend::EphemeralProvisionedSecret {
            return Err(KeyError::AuthUnavailable);
        }
        let ctx = self.context(boot_session_root);
        let prime_log = prime_log_for_equation(&equation_root);
        let fields = AuthenticatedCheckpointFields {
            version: FORMAT_VERSION,
            sequence: self.next_sequence,
            previous_root: self.last_prev_root,
            kernel_root,
            equation_root,
            boot_session_root,
            chronos_tick,
            kairos_tick,
            key_epoch: self.key_epoch,
            key_backend: CheckpointKeyBackend::EphemeralProvisionedSecret,
            payload_root: payload_root(&prime_log, &app_registry_root),
            device_id: self.device_id,
        };
        let src = EphemeralProvisionedSecret { secret };
        let record = seal_record(&src, &ctx, &fields)?;
        let body = encode_mac_body(&fields);
        self.last_prev_root = previous_checkpoint_root(&body);
        self.last_accepted = Some(fields.sequence);
        self.next_sequence = fields.sequence.saturating_add(1);
        self.last_record = Some(record.clone());
        Ok(record)
    }

    pub fn verify_bytes(
        &mut self,
        boot_session_root: StateHash,
        bytes: &[u8],
    ) -> Result<AuthenticatedCheckpointFields, KeyError> {
        let secret = self.secret.as_ref().ok_or(KeyError::AuthUnavailable)?;
        if self.backend != CheckpointKeyBackend::EphemeralProvisionedSecret {
            return Err(KeyError::AuthUnavailable);
        }
        let ctx = self.context(boot_session_root);
        let src = EphemeralProvisionedSecret { secret };
        let fields = verify_record(&src, &ctx, bytes)?;
        if let Some(last) = self.last_accepted {
            if fields.sequence < last {
                return Err(KeyError::Rollback);
            }
        }
        if self
            .last_accepted
            .map(|n| fields.sequence > n)
            .unwrap_or(true)
        {
            self.last_accepted = Some(fields.sequence);
            self.next_sequence = fields.sequence.saturating_add(1);
            self.last_prev_root = previous_checkpoint_root(&encode_mac_body(&fields));
        }
        self.last_record = Some(bytes.to_vec());
        Ok(fields)
    }
}

impl Drop for CheckpointAuthController {
    fn drop(&mut self) {
        if let Some(ref mut secret) = self.secret {
            zeroize(secret);
        }
    }
}

pub fn parse_development_secret(line: &str) -> Result<[u8; 32], KeyError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(KeyError::BadSecret);
    }
    if trimmed.len() == 64 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        let mut out = [0u8; 32];
        for i in 0..32 {
            out[i] = u8::from_str_radix(&trimmed[i * 2..i * 2 + 2], 16)
                .map_err(|_| KeyError::BadSecret)?;
        }
        if out.iter().all(|b| *b == 0) {
            return Err(KeyError::BadSecret);
        }
        return Ok(out);
    }
    if trimmed.len() == 32 {
        let mut out = [0u8; 32];
        out.copy_from_slice(trimmed.as_bytes());
        if out.iter().all(|b| *b == 0) {
            return Err(KeyError::BadSecret);
        }
        return Ok(out);
    }
    Err(KeyError::BadSecret)
}

fn push_lp(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

struct LpReader<'a> {
    src: &'a [u8],
    off: usize,
}

impl<'a> LpReader<'a> {
    fn lp(&mut self) -> Result<&'a [u8], KeyError> {
        let end = self.off.checked_add(4).ok_or(KeyError::Truncated)?;
        if end > self.src.len() {
            return Err(KeyError::Truncated);
        }
        let n = u32::from_le_bytes(self.src[self.off..end].try_into().unwrap()) as usize;
        self.off = end;
        let data_end = self.off.checked_add(n).ok_or(KeyError::Truncated)?;
        if data_end > self.src.len() {
            return Err(KeyError::Truncated);
        }
        let slice = &self.src[self.off..data_end];
        self.off = data_end;
        Ok(slice)
    }

    fn u32_lp(&mut self) -> Result<u32, KeyError> {
        let b = self.lp()?;
        if b.len() != 4 {
            return Err(KeyError::Truncated);
        }
        Ok(u32::from_le_bytes(b.try_into().unwrap()))
    }

    fn u64_lp(&mut self) -> Result<u64, KeyError> {
        let b = self.lp()?;
        if b.len() != 8 {
            return Err(KeyError::Truncated);
        }
        Ok(u64::from_le_bytes(b.try_into().unwrap()))
    }

    fn hash_lp(&mut self) -> Result<StateHash, KeyError> {
        let b = self.lp()?;
        if b.len() != 32 {
            return Err(KeyError::Truncated);
        }
        let mut h = [0u8; 32];
        h.copy_from_slice(b);
        Ok(h)
    }
}

pub fn zeroize(bytes: &mut [u8]) {
    for b in bytes.iter_mut() {
        unsafe { core::ptr::write_volatile(b, 0) };
    }
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut d = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        d |= x ^ y;
    }
    d == 0
}

fn short_hex(bytes: &[u8; 32]) -> String {
    bytes.iter().take(4).map(|b| alloc_hex(*b)).collect::<String>()
}

fn alloc_hex(b: u8) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::new();
    s.push(HEX[(b >> 4) as usize] as char);
    s.push(HEX[(b & 0x0f) as usize] as char);
    s
}

impl crate::kernel::Kernel {
    pub fn checkpoint_auth_status_text(&self) -> String {
        self.checkpoint_auth.status_text()
    }

    pub fn checkpoint_auth_hardware_text(&self) -> String {
        let _ = HardwareBoundSecret {}.derive_checkpoint_key(
            &CheckpointContext {
                device_id: [0u8; 32],
                boot_session_root: [0u8; 32],
                key_epoch: 0,
                key_backend: CheckpointKeyBackend::HardwareBoundSecret,
            },
            &mut [0u8; 32],
        );
        self.checkpoint_auth.hardware_status_text()
    }

    pub fn checkpoint_auth_seal(&mut self) -> Result<Vec<u8>, KeyError> {
        let app_root = {
            #[cfg(feature = "phasestack")]
            {
                self.phasestack.registry_root()
            }
            #[cfg(not(feature = "phasestack"))]
            {
                [0u8; 32]
            }
        };
        self.checkpoint_auth.seal(
            self.authority.boot_session_root,
            self.statechain.kernel_root(),
            self.statechain.equation_root(),
            self.clock.chronos_tick,
            self.clock.kairos_tick,
            app_root,
        )
    }

    pub fn checkpoint_auth_verify_last(
        &mut self,
    ) -> Result<AuthenticatedCheckpointFields, KeyError> {
        let boot = self.authority.boot_session_root;
        let record = self
            .checkpoint_auth
            .last_record()
            .ok_or(KeyError::Empty)?
            .to_vec();
        self.checkpoint_auth.verify_bytes(boot, &record)
    }

    pub fn checkpoint_auth_verify_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<AuthenticatedCheckpointFields, KeyError> {
        self.checkpoint_auth
            .verify_bytes(self.authority.boot_session_root, bytes)
    }
}
