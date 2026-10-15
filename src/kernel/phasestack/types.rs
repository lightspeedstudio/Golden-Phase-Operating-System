//! PhaseStack types. Developer PKI types are independent of `identity`.

use crate::kernel::hash::StateHash;
use crate::platform::{String, Vec};
use core::fmt;

pub type AppId = StateHash;
pub type PackageRoot = StateHash;
pub type PrincipalId = StateHash;

pub const MAX_NAME_LEN: usize = 64;
pub const MAX_CHAIN_DEPTH: usize = 8;
pub const MAX_CERTIFICATES: usize = 256;
pub const MAX_GPA_BYTES: usize = 1_048_576;
pub const MAX_GPA_ENTRIES: usize = 64;
pub const MAX_GPA_NAME_LEN: usize = 128;
pub const MAX_HOOK_PAYLOAD: usize = 256;
pub const MAX_HOOK_QUEUE: usize = 16;
pub const MAX_HOOK_FUEL: u32 = 100_000;
pub const MAX_PLUGINS: usize = 8;
pub const MAX_APPS: usize = 16;
pub const MAX_PERMITS: usize = 32;
pub const MAX_NONCES: usize = 64;
pub const LAUNCH_NONCE_LEN: usize = 16;
pub const NONCE_MEM_OFFSET: usize = 0;
pub const MAX_BYTE_OUTPUT: usize = 32;
pub const PRIVILEGED_HOOK_PRIORITY: u8 = 0;

pub const CERT_ALG_ED25519: u8 = 0;
pub const CERTIFICATE_VERSION_V1: u16 = 1;
pub const GPA_VERSION_V1: u16 = 1;
pub const ABI_VERSION_V1: u16 = 1;
pub const MANIFEST_VERSION_V2: u16 = 2;

pub const ARCH_PHASE_VM: u16 = 0;
pub const ARCH_NATIVE: u16 = 1;
pub const PHASEVM_MAX_MEM: u32 = 256;
pub const PHASEVM_ENTRY: u32 = 0;
/// Print instruction. Syscall numbers 1..=31 occupy the remaining bits.
pub const SYSCALL_PRINT: u32 = 1 << 0;
/// Emit one memory byte to the VM byte-output buffer (nonce-bearing output).
pub const SYSCALL_EMIT: u32 = 1 << 1;

pub const PKG_TYPE_APP: u16 = 1 << 0;
pub const PKG_TYPE_PLUGIN: u16 = 1 << 1;
pub const SCOPE_USER: u8 = 1 << 0;
pub const SCOPE_SYSTEM: u8 = 1 << 1;

pub const CERT_FLAG_CAN_ISSUE: u16 = 1 << 0;
pub const CERT_FLAG_ROOT: u16 = 1 << 1;

pub const NO_ISSUER: StateHash = [0u8; 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl AppVersion {
    pub fn encode(self) -> [u8; 6] {
        let mut out = [0u8; 6];
        out[0..2].copy_from_slice(&self.major.to_le_bytes());
        out[2..4].copy_from_slice(&self.minor.to_le_bytes());
        out[4..6].copy_from_slice(&self.patch.to_le_bytes());
        out
    }

    pub fn cmp_tuple(self) -> (u16, u16, u16) {
        (self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemPrincipal {
    User,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Verified,
    Installed,
    Running,
    Stopped,
    Quarantined,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppExecutionKind {
    PhaseVm,
    Native,
}

impl AppExecutionKind {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::PhaseVm => 0,
            Self::Native => 1,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::PhaseVm),
            1 => Some(Self::Native),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardHook {
    OnLoad,
    OnStart,
    OnStop,
    OnMessage,
    OnFault,
}

impl StandardHook {
    pub fn as_u32(self) -> u32 {
        match self {
            Self::OnLoad => 1,
            Self::OnStart => 2,
            Self::OnStop => 3,
            Self::OnMessage => 4,
            Self::OnFault => 5,
        }
    }

    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            1 => Some(Self::OnLoad),
            2 => Some(Self::OnStart),
            3 => Some(Self::OnStop),
            4 => Some(Self::OnMessage),
            5 => Some(Self::OnFault),
            _ => None,
        }
    }

    pub fn bit(self) -> u32 {
        1 << (self.as_u32() - 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookMode {
    Notify,
    RequestResponse,
}

impl HookMode {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Notify => 0,
            Self::RequestResponse => 1,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Notify),
            1 => Some(Self::RequestResponse),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallScope {
    User,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedPackage {
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedVec<T> {
    inner: Vec<T>,
    max: usize,
}

impl<T> BoundedVec<T> {
    pub fn new(max: usize) -> Self {
        Self {
            inner: Vec::new(),
            max,
        }
    }

    pub fn push(&mut self, item: T) -> Result<(), PhaseStackError> {
        if self.inner.len() >= self.max {
            return Err(PhaseStackError::BoundExceeded);
        }
        self.inner.push(item);
        Ok(())
    }

    pub fn as_slice(&self) -> &[T] {
        &self.inner
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecEnvelope {
    pub arch: u16,
    pub entry_point: u32,
    pub mem_bytes: u32,
    pub declared_syscalls: u32,
    pub mmio_caps: u32,
    pub dependency_root: StateHash,
    pub min_version_counter: u64,
    pub publisher_key_id: [u8; 32],
}

impl Default for ExecEnvelope {
    fn default() -> Self {
        Self {
            arch: ARCH_PHASE_VM,
            entry_point: PHASEVM_ENTRY,
            mem_bytes: PHASEVM_MAX_MEM,
            declared_syscalls: 0,
            mmio_caps: 0,
            dependency_root: [0u8; 32],
            min_version_counter: 0,
            publisher_key_id: [0u8; 32],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchResult {
    pub pid: u32,
    pub retired_instructions: usize,
    pub output: Vec<u8>,
    pub byte_output: Vec<u8>,
    pub launch_nonce: [u8; LAUNCH_NONCE_LEN],
    pub payload_root: StateHash,
    pub manifest_root: StateHash,
    pub registry_generation: u64,
}

/// Verified package. Fields are private; only `verifier` may construct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPackage {
    app_id: AppId,
    version: AppVersion,
    package_root: PackageRoot,
    code_hash: StateHash,
    manifest_hash: StateHash,
    hook_manifest_hash: StateHash,
    resource_root: StateHash,
    developer_cert_hash: StateHash,
    exec_kind: AppExecutionKind,
    requested_caps: u32,
    requested_hooks: u32,
    abi_version: u16,
    name: String,
    code: Vec<u8>,
    package_type: u16,
    envelope: ExecEnvelope,
}

impl VerifiedPackage {
    pub(super) fn from_parts(
        app_id: AppId,
        version: AppVersion,
        package_root: PackageRoot,
        code_hash: StateHash,
        manifest_hash: StateHash,
        hook_manifest_hash: StateHash,
        resource_root: StateHash,
        developer_cert_hash: StateHash,
        exec_kind: AppExecutionKind,
        requested_caps: u32,
        requested_hooks: u32,
        abi_version: u16,
        name: String,
        code: Vec<u8>,
        package_type: u16,
        envelope: ExecEnvelope,
    ) -> Self {
        Self {
            app_id,
            version,
            package_root,
            code_hash,
            manifest_hash,
            hook_manifest_hash,
            resource_root,
            developer_cert_hash,
            exec_kind,
            requested_caps,
            requested_hooks,
            abi_version,
            name,
            code,
            package_type,
            envelope,
        }
    }

    pub fn app_id(&self) -> AppId {
        self.app_id
    }
    pub fn version(&self) -> AppVersion {
        self.version
    }
    pub fn package_root(&self) -> PackageRoot {
        self.package_root
    }
    pub fn code_hash(&self) -> StateHash {
        self.code_hash
    }
    pub fn manifest_hash(&self) -> StateHash {
        self.manifest_hash
    }
    pub fn hook_manifest_hash(&self) -> StateHash {
        self.hook_manifest_hash
    }
    pub fn resource_root(&self) -> StateHash {
        self.resource_root
    }
    pub fn developer_cert_hash(&self) -> StateHash {
        self.developer_cert_hash
    }
    pub fn exec_kind(&self) -> AppExecutionKind {
        self.exec_kind
    }
    pub fn requested_caps(&self) -> u32 {
        self.requested_caps
    }
    pub fn requested_hooks(&self) -> u32 {
        self.requested_hooks
    }
    pub fn abi_version(&self) -> u16 {
        self.abi_version
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn code(&self) -> &[u8] {
        &self.code
    }
    pub fn package_type(&self) -> u16 {
        self.package_type
    }
    pub fn envelope(&self) -> &ExecEnvelope {
        &self.envelope
    }
    pub fn publisher_key_id(&self) -> [u8; 32] {
        self.envelope.publisher_key_id
    }
    pub fn payload_root(&self) -> StateHash {
        self.resource_root
    }
}

/// Installed, verified row. Fields private; minted by verifier at install commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedInstallation {
    package: VerifiedPackage,
    granted_caps: u32,
    granted_hooks: u32,
    granted_max_priority: u8,
    scope: InstallScope,
    state: AppState,
    store_path: String,
    process_id: Option<u32>,
    registry_generation: u64,
    install_sequence: u64,
}

impl VerifiedInstallation {
    pub(super) fn from_verified(
        package: VerifiedPackage,
        granted_caps: u32,
        granted_hooks: u32,
        granted_max_priority: u8,
        scope: InstallScope,
        store_path: String,
    ) -> Self {
        Self {
            package,
            granted_caps,
            granted_hooks,
            granted_max_priority,
            scope,
            state: AppState::Installed,
            store_path,
            process_id: None,
            registry_generation: 0,
            install_sequence: 0,
        }
    }

    pub fn package(&self) -> &VerifiedPackage {
        &self.package
    }
    pub fn granted_caps(&self) -> u32 {
        self.granted_caps
    }
    pub fn granted_hooks(&self) -> u32 {
        self.granted_hooks
    }
    pub fn granted_max_priority(&self) -> u8 {
        self.granted_max_priority
    }
    pub fn scope(&self) -> InstallScope {
        self.scope
    }
    pub fn state(&self) -> AppState {
        self.state
    }
    pub fn store_path(&self) -> &str {
        &self.store_path
    }
    pub fn process_id(&self) -> Option<u32> {
        self.process_id
    }
    pub fn registry_generation(&self) -> u64 {
        self.registry_generation
    }
    pub fn install_sequence(&self) -> u64 {
        self.install_sequence
    }

    pub(super) fn set_state(&mut self, state: AppState) {
        self.state = state;
    }

    pub(super) fn set_process_id(&mut self, pid: Option<u32>) {
        self.process_id = pid;
    }

    pub(super) fn set_registry_generation(&mut self, generation: u64) {
        self.registry_generation = generation;
    }

    pub(super) fn set_install_sequence(&mut self, sequence: u64) {
        self.install_sequence = sequence;
    }

    pub(super) fn set_granted_caps(&mut self, caps: u32) {
        self.granted_caps = caps;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseStackError {
    UnknownVersion,
    DuplicateSerial,
    ChainTooDeep,
    ChainCycle,
    AlgorithmRejected,
    UnknownIssuer,
    Revoked,
    GenerationExpired,
    SignatureInvalid,
    NotSelfSigned,
    DuplicateCert,
    TableFull,
    BoundExceeded,
    BadMagic,
    Overflow,
    PathTraversal,
    AbsolutePath,
    DuplicateEntry,
    Oversize,
    MissingEntry,
    CanonicalMismatch,
    HashMismatch,
    UnknownCert,
    WrongSignature,
    Unsigned,
    AbiRejected,
    GrantNotSubset,
    PrivilegedPriorityDenied,
    LoadDenied,
    Quarantined,
    NativeIsolationUnavailable,
    NotFound,
    Immutable,
    AlreadyInstalled,
    HookQueueFull,
    HookPayloadOversize,
    HookFuelExceeded,
    HookDeadline,
    FuelAbort,
    VmFault,
    PluginCapEscalation,
    NativePluginDenied,
    AuthorityDenied,
    VerificationRequired,
    InstallFailed,
    Locked,
    Fs,
    RetiredSigner,
    UndeclaredSyscall,
    UndeclaredMmio,
    ExcessiveMemory,
    BadEntry,
    WrongArch,
    RollbackVersion,
    SentinelDenied,
    NoInstruction,
    RegistryMissing,
    NotInstalled,
    ProcessAlreadyRunning,
    ManifestRootMismatch,
    PayloadRootMismatch,
    RegistryGenerationMismatch,
    InstallSequenceMismatch,
    CapabilityEscalation,
    MemoryEnvelopeInvalid,
    EntryNotDeclared,
    NonceReplayed,
    PermitConsumed,
    KernelStateInvalid,
    SignerUnknown,
    PackageQuarantined,
}

impl fmt::Display for PhaseStackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

pub fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::new();
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

pub fn parse_hex32(s: &str) -> Result<StateHash, PhaseStackError> {
    if s.len() != 64 {
        return Err(PhaseStackError::CanonicalMismatch);
    }
    let mut out = [0u8; 32];
    let bytes = s.as_bytes();
    for i in 0..32 {
        let hi = hex_nibble(bytes[i * 2])?;
        let lo = hex_nibble(bytes[i * 2 + 1])?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, PhaseStackError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(PhaseStackError::CanonicalMismatch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_package_has_no_public_constructor() {
        // Construction is `pub(super)` — this test proves getters work on a
        // value minted through the crate-internal constructor.
        let pkg = VerifiedPackage::from_parts(
            [1u8; 32],
            AppVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            [2u8; 32],
            [3u8; 32],
            [4u8; 32],
            [5u8; 32],
            [6u8; 32],
            [7u8; 32],
            AppExecutionKind::PhaseVm,
            0,
            0,
            ABI_VERSION_V1,
            String::from("t"),
            Vec::new(),
            PKG_TYPE_APP,
            ExecEnvelope::default(),
        );
        assert_eq!(pkg.name(), "t");
        assert_eq!(pkg.exec_kind(), AppExecutionKind::PhaseVm);
    }

    #[test]
    fn hex_roundtrip() {
        let h = [0xabu8, 0xcd, 0x00, 0xff].iter().copied().chain([0u8; 28]).collect::<Vec<_>>();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&h);
        assert_eq!(parse_hex32(&hex_lower(&arr)).unwrap(), arr);
    }
}
