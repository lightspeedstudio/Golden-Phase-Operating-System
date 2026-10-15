//! Per-object ANOM protection policy and identifiers.

use crate::kernel::hash::StateHash;
use crate::platform::String;

pub const OBJECT_PROTECTION_VERSION: u32 = 1;
pub const OBJECT_WRAP_TAG: &str = "KAIROS_ANOM_OBJECT_WRAP_V1";
pub const OBJECT_STATE_TAG: &str = "KAIROS_ANOM_OBJECT_STATE_V1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ProtectedObjectKind {
    Process,
    Memory,
    File,
    Vm,
    Craft,
    StateClock,
    Network,
}

impl ProtectedObjectKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ProtectedObjectKind::Process => "process",
            ProtectedObjectKind::Memory => "memory",
            ProtectedObjectKind::File => "file",
            ProtectedObjectKind::Vm => "vm",
            ProtectedObjectKind::Craft => "craft",
            ProtectedObjectKind::StateClock => "clock",
            ProtectedObjectKind::Network => "network",
        }
    }

    pub fn parse(prefix: &str) -> Option<Self> {
        match prefix {
            "process" => Some(ProtectedObjectKind::Process),
            "memory" | "page" => Some(ProtectedObjectKind::Memory),
            "file" => Some(ProtectedObjectKind::File),
            "vm" => Some(ProtectedObjectKind::Vm),
            "craft" => Some(ProtectedObjectKind::Craft),
            "clock" | "stateclock" => Some(ProtectedObjectKind::StateClock),
            "network" => Some(ProtectedObjectKind::Network),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtectedObjectId {
    pub kind: ProtectedObjectKind,
    pub value: String,
}

impl ProtectedObjectId {
    pub fn new(kind: ProtectedObjectKind, value: impl Into<String>) -> Self {
        Self {
            kind,
            value: value.into(),
        }
    }

    pub fn canonical(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.value)
    }

    pub fn parse(spec: &str) -> Result<Self, ObjectProtectionError> {
        let (kind_str, value) = spec
            .split_once(':')
            .ok_or(ObjectProtectionError::InvalidObjectId)?;
        if value.is_empty() {
            return Err(ObjectProtectionError::InvalidObjectId);
        }
        let kind =
            ProtectedObjectKind::parse(kind_str).ok_or(ObjectProtectionError::InvalidObjectId)?;
        Ok(Self {
            kind,
            value: value.into(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ObjectProtectionStatus {
    Unprotected,
    ProtectedLocked,
    ProtectedOpen,
    VerificationFailed,
    HostedOnly,
    Inactive,
}

impl ObjectProtectionStatus {
    pub fn label(self) -> &'static str {
        match self {
            ObjectProtectionStatus::Unprotected => "unprotected",
            ObjectProtectionStatus::ProtectedLocked => "protected/locked",
            ObjectProtectionStatus::ProtectedOpen => "protected/open",
            ObjectProtectionStatus::VerificationFailed => "verification failed",
            ObjectProtectionStatus::HostedOnly => "hosted-only",
            ObjectProtectionStatus::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectProtectionRecord {
    pub object_id: ProtectedObjectId,
    pub version: u32,
    pub equation_root: StateHash,
    pub boot_session_root: StateHash,
    pub object_salt: Vec<u8>,
    pub wrap_nonce: Vec<u8>,
    pub wrapped_object_key: Vec<u8>,
    pub wrap_tag: Vec<u8>,
    pub created_sequence: u64,
    pub last_rekey_sequence: Option<u64>,
    pub commitment_root: StateHash,
    pub current_statechain_head: StateHash,
    pub status: ObjectProtectionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectProtectionError {
    ObjectNotFound,
    AlreadyProtected,
    NotProtected,
    PassphraseMismatch,
    AuthenticationFailed,
    InvalidObjectId,
    CryptoUnavailable,
    BaremetalHostedOnly,
    ChainVerificationFailed,
    MerkleVerificationFailed,
    PersistenceFailed,
    InvalidState,
    StatechainUnavailable,
}

impl ObjectProtectionError {
    pub fn user_message(&self) -> &'static str {
        match self {
            ObjectProtectionError::AuthenticationFailed
            | ObjectProtectionError::PassphraseMismatch => "ANOM object authentication failed",
            ObjectProtectionError::ObjectNotFound => "object not found",
            ObjectProtectionError::AlreadyProtected => "object already protected",
            ObjectProtectionError::NotProtected => "object not protected",
            ObjectProtectionError::InvalidObjectId => "invalid object identifier",
            ObjectProtectionError::CryptoUnavailable => "crypto unavailable",
            ObjectProtectionError::BaremetalHostedOnly => "hosted-only (baremetal stub)",
            ObjectProtectionError::ChainVerificationFailed => "chain verification failed",
            ObjectProtectionError::MerkleVerificationFailed => "merkle verification failed",
            ObjectProtectionError::PersistenceFailed => "persistence failed",
            ObjectProtectionError::InvalidState => "invalid state",
            ObjectProtectionError::StatechainUnavailable => "statechain unavailable",
        }
    }
}

use crate::platform::Vec;
