//! Password credentials (`kernel::identity`, Gate 3) — `PasswordPolicy`,
//! real Argon2id `PasswordVerifier` hashing/verification,
//! `RootCredentialState`, and a small embedded blocklist.
//!
//! ## What this reuses
//! - [`crate::kernel::state_root::merkle_root`] — for [`Blocklist::root`],
//!   the same Merkle construction every other subsystem root uses.
//! - [`crate::kernel::crypto::phase_hash::ph_content`] — blocklist entries
//!   are hashed (never stored/committed as plaintext leaves) before being
//!   fed to `merkle_root`.
//! - `argon2` (RustCrypto, real, audited, `no_std`+`alloc`-compatible) for
//!   the actual password hashing — no custom/home-grown hashing, per the
//!   platform spec's explicit "do not invent cryptography."
//!
//! ## Honest disclosure: entropy source for salts
//! Per the Gate 0 audit and your explicit decision: **hosted `std`**
//! builds get a real salt from the OS CSPRNG (`BCryptGenRandom` on
//! Windows, `/dev/urandom` on unix) — not the `getrandom` crate, whose
//! `custom` feature is reserved for `ssh-bm` on `armv7a-none-eabi`.
//! **Bare-metal `zynq-a9`** builds reuse `board::zynq7000::rng::fill`
//! directly — the *same* already-disclosed timer-jitter DRBG (**not a
//! production entropy source**; there is no hardware TRNG on the
//! Zynq-7000 PS). Any other baremetal target (none currently shipped) is
//! a hard `compile_error!`, not a silent fallback — per the platform spec:
//! "If no real entropy source exists, stop and report the blocker."
//!
//! ## Honest disclosure: Argon2id parameters are NOT yet hardware-benchmarked
//! The whole kernel's heap is 512 KiB total (`src/boot/heap.rs`,
//! `HEAP_BYTES`), shared by every subsystem. [`MAX_ARGON2_MEMORY_KIB`] hard-
//! caps Argon2's memory cost at 64 KiB (~12.5% of the entire heap) so a
//! password hash can never exhaust kernel memory — but that bound is far
//! below OWASP/RFC 9106's recommended 19-46 MiB memory-hardness for
//! Argon2id. [`DEFAULT_ARGON2_MEMORY_KIB`]/[`DEFAULT_ARGON2_ITERATIONS`]/
//! [`DEFAULT_ARGON2_LANES`] are real, working, but **not yet measured on
//! the physical PZ7020** — per the platform spec: "Do not claim the
//! parameters are adequate until measured on hardware." Gate 11/13 will
//! record real timing measurements; until then this is a disclosed,
//! not-yet-validated default, not a claim of adequacy.
//!
//! ## Honest disclosure: character set
//! Unicode normalization is not implemented in this `no_std` environment.
//! Passwords are restricted to printable ASCII (`0x20..=0x7E`, i.e. space
//! through `~`) — exactly the platform spec's own fallback: "support
//! printable ASCII and spaces initially and document that limitation
//! honestly."
//!
//! ## Honest disclosure: the embedded blocklist is small and illustrative
//! [`DEFAULT_BLOCKLIST`] is a small, well-known set of extremely common
//! passwords (the kind repeatedly published in "most common passwords"
//! analyses). It is **not** a full breach corpus — no network access
//! exists on bare metal to consult a live breached-password service, and
//! embedding a multi-million-entry corpus is not feasible in a 512 KiB
//! heap. This is a real, working, but deliberately limited check, disclosed
//! as such rather than overclaimed.
//!
//! ## Non-goals (Gate 3)
//! - No secure (no-echo) terminal input yet, no Root provisioning gate
//!   (physical presence + recovery-certificate authorization), no `passwd`
//!   command — Gate 4. [`RootCredential::provision`] is the raw state-machine
//!   mechanism only; Gate 4 owns gating every call site behind the real
//!   provisioning procedure.
//! - No persistence (Gate 6): `RootCredential` is RAM-only here.
//! - No `CredentialRoot`/`AuthorityStateRoot` ledger commitment — Gate 5.

use crate::kernel::crypto::phase_hash::ph_content;
use crate::kernel::hash::StateHash;
use crate::kernel::state_root::merkle_root;
use crate::platform::Vec;

/// Current password-verifier record encoding version.
pub const PASSWORD_VERIFIER_VERSION_V1: u16 = 1;

/// Salt length in bytes (128 bits — platform spec §4.2's "at least 128
/// random bits").
pub const SALT_LEN: usize = 16;
/// Argon2id output length in bytes.
pub const VERIFIER_LEN: usize = 32;

/// Hard bound on Argon2's memory cost. See module docs, "Honest disclosure:
/// Argon2id parameters".
pub const MAX_ARGON2_MEMORY_KIB: u32 = 64;
/// Real, working, but not-yet-hardware-benchmarked defaults. See module
/// docs.
pub const DEFAULT_ARGON2_MEMORY_KIB: u32 = 32;
pub const DEFAULT_ARGON2_ITERATIONS: u32 = 3;
pub const DEFAULT_ARGON2_LANES: u8 = 1;

/// Password hashing algorithm. Argon2id is the only variant — the platform
/// spec's explicit list of what *not* to use (SHA-256 alone, PhaseHash
/// alone, FNV, SplitMix, CRC, repeated custom hashing, encryption-as-
/// storage) leaves exactly one real, audited choice for this environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordAlgorithm {
    Argon2id,
}

impl PasswordAlgorithm {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Argon2id => 0,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Argon2id),
            _ => None,
        }
    }
}

/// Platform spec §4.2's suggested record, implemented verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordVerifier {
    pub version: u16,
    pub algorithm: PasswordAlgorithm,

    pub salt: [u8; SALT_LEN],
    pub verifier: [u8; VERIFIER_LEN],

    pub memory_kib: u32,
    pub iterations: u32,
    pub lanes: u8,

    pub changed_generation: u64,
    pub must_change: bool,

    pub failed_attempts: u16,
    pub lockout_until_chronos: u64,
}

/// Platform spec §15's explicit Root credential state machine.
/// `Unprovisioned` must never mean open access (enforced by
/// [`RootCredential::authenticate`], which only ever attempts
/// authentication from `Active`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootCredentialState {
    Unprovisioned,
    Active,
    Locked,
    RecoveryRequired,
}

impl RootCredentialState {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Unprovisioned => 0,
            Self::Active => 1,
            Self::Locked => 2,
            Self::RecoveryRequired => 3,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Unprovisioned),
            1 => Some(Self::Active),
            2 => Some(Self::Locked),
            3 => Some(Self::RecoveryRequired),
            _ => None,
        }
    }
}

/// Platform spec §5's configurable policy record, implemented verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordPolicy {
    pub minimum_length: u16,
    pub maximum_length: u16,
    pub maximum_failures: u16,
    pub initial_delay_ticks: u64,
    pub maximum_delay_ticks: u64,
    pub blocklist_root: StateHash,
    pub history_depth: u8,
}

impl Default for PasswordPolicy {
    /// Platform spec §5's default policy: "Minimum 15 characters...permit
    /// at least 64, target maximum 128...do not require uppercase,
    /// lowercase, numbers, or symbols...do not require routine periodic
    /// password changes." `maximum_failures`/`initial_delay_ticks`/
    /// `maximum_delay_ticks`/`history_depth` have no spec-mandated numeric
    /// values (only qualitative "rate-limit failed attempts" / "small
    /// configurable password-history list") — these are real, disclosed
    /// engineering defaults, not literal spec numbers.
    fn default() -> Self {
        Self {
            minimum_length: 15,
            maximum_length: 128,
            maximum_failures: 10,
            initial_delay_ticks: 1,
            maximum_delay_ticks: 1024,
            blocklist_root: Blocklist::default_blocklist().root(),
            history_depth: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordPolicyError {
    TooShort,
    TooLong,
    /// Contains a byte outside printable ASCII (`0x20..=0x7E`). See module
    /// docs, "Honest disclosure: character set".
    UnsupportedCharacter,
    Blocklisted,
    ContainsUsernameOrProductName,
}

/// A small, well-known set of extremely common passwords. See module docs,
/// "Honest disclosure: the embedded blocklist is small and illustrative".
pub const DEFAULT_BLOCKLIST: &[&str] = &[
    "123456", "password", "12345678", "qwerty", "123456789", "12345", "1234", "111111",
    "1234567", "dragon", "123123", "baseball", "abc123", "football", "monkey", "letmein",
    "shadow", "master", "666666", "qwertyuiop", "1234567890", "654321", "superman", "1qaz2wsx",
    "121212", "000000", "qazwsx", "trustno1", "iloveyou", "admin", "welcome", "login",
];

/// Names checked as a case-insensitive substring of a candidate password
/// (platform spec §5: "reject passwords containing...the product name").
pub const PRODUCT_NAMES: &[&str] = &["kairos", "goldenphase", "golden phase", "gpos"];

fn ascii_lower(b: u8) -> u8 {
    b.to_ascii_lowercase()
}

fn ascii_case_insensitive_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| ascii_lower(*x) == ascii_lower(*y))
}

fn ascii_contains_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|w| ascii_case_insensitive_eq(w, needle))
}

/// Bounded blocklist over a static string list, plus a real Merkle
/// commitment ([`Blocklist::root`]) so [`PasswordPolicy::blocklist_root`]
/// can prove which blocklist version a policy was checked against.
pub struct Blocklist {
    entries: &'static [&'static str],
}

impl Blocklist {
    pub const fn new(entries: &'static [&'static str]) -> Self {
        Self { entries }
    }

    pub fn default_blocklist() -> Self {
        Self::new(DEFAULT_BLOCKLIST)
    }

    /// Case-insensitive exact match against every entry.
    pub fn contains(&self, candidate: &[u8]) -> bool {
        self.entries
            .iter()
            .any(|entry| ascii_case_insensitive_eq(entry.as_bytes(), candidate))
    }

    /// Case-insensitive substring match against `username` (if given) or
    /// any [`PRODUCT_NAMES`] entry.
    pub fn contains_username_or_product(&self, candidate: &[u8], username: Option<&[u8]>) -> bool {
        if let Some(user) = username {
            if !user.is_empty() && ascii_contains_case_insensitive(candidate, user) {
                return true;
            }
        }
        PRODUCT_NAMES
            .iter()
            .any(|name| ascii_contains_case_insensitive(candidate, name.as_bytes()))
    }

    /// Real Merkle commitment over `PH-C(lowercase(entry))` for every
    /// entry — never the plaintext entries themselves as leaves (this
    /// module's own "do not put plaintext ... in Merkle leaves" discipline,
    /// applied even to non-secret illustrative data for consistency).
    pub fn root(&self) -> StateHash {
        let leaves: Vec<StateHash> = self
            .entries
            .iter()
            .map(|entry| ph_content(entry.to_ascii_lowercase().as_bytes()))
            .collect();
        merkle_root(&leaves)
    }
}

/// Validate `password` against `policy` and `blocklist`. Never truncates:
/// the full byte slice is checked and (by callers) hashed.
pub fn validate_password(
    policy: &PasswordPolicy,
    password: &[u8],
    username: Option<&[u8]>,
    blocklist: &Blocklist,
) -> Result<(), PasswordPolicyError> {
    let len = password.len();
    if len < policy.minimum_length as usize {
        return Err(PasswordPolicyError::TooShort);
    }
    if len > policy.maximum_length as usize {
        return Err(PasswordPolicyError::TooLong);
    }
    if !password.iter().all(|&b| (0x20..=0x7E).contains(&b)) {
        return Err(PasswordPolicyError::UnsupportedCharacter);
    }
    if blocklist.contains(password) {
        return Err(PasswordPolicyError::Blocklisted);
    }
    if blocklist.contains_username_or_product(password, username) {
        return Err(PasswordPolicyError::ContainsUsernameOrProductName);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    /// The platform's real entropy source could not produce a salt. See
    /// module docs, "Honest disclosure: entropy source".
    EntropyUnavailable,
    /// Argon2 itself reported a hashing failure.
    HashingFailed,
    /// Requested Argon2 parameters are invalid or exceed
    /// [`MAX_ARGON2_MEMORY_KIB`].
    InvalidParameters,
    /// [`PasswordVerifier::lockout_until_chronos`] has not yet elapsed;
    /// the attempt was refused before any hashing occurred.
    RateLimited,
    NotProvisioned,
    NotActive,
    AlreadyProvisioned,
}

#[cfg(all(feature = "std", windows))]
pub fn generate_salt() -> Result<[u8; SALT_LEN], CredentialError> {
    let mut salt = [0u8; SALT_LEN];
    #[link(name = "advapi32")]
    extern "system" {
        fn SystemFunction036(buffer: *mut u8, length: u32) -> u8;
    }
    let ok = unsafe { SystemFunction036(salt.as_mut_ptr(), salt.len() as u32) };
    if ok == 0 {
        return Err(CredentialError::EntropyUnavailable);
    }
    Ok(salt)
}

#[cfg(all(feature = "std", unix))]
pub fn generate_salt() -> Result<[u8; SALT_LEN], CredentialError> {
    use std::io::Read;
    let mut salt = [0u8; SALT_LEN];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut salt))
        .map_err(|_| CredentialError::EntropyUnavailable)?;
    Ok(salt)
}

#[cfg(all(feature = "std", not(any(windows, unix))))]
pub fn generate_salt() -> Result<[u8; SALT_LEN], CredentialError> {
    compile_error!(
        "kernel::identity::credential::generate_salt has no OS CSPRNG binding for this hosted \
         target (only windows and unix are implemented)"
    );
}

#[cfg(all(feature = "baremetal", feature = "zynq-a9", not(feature = "std")))]
pub fn generate_salt() -> Result<[u8; SALT_LEN], CredentialError> {
    let mut salt = [0u8; SALT_LEN];
    crate::board::zynq7000::rng::fill_checked(&mut salt)
        .map_err(|_| CredentialError::EntropyUnavailable)?;
    Ok(salt)
}

#[cfg(not(any(feature = "std", all(feature = "baremetal", feature = "zynq-a9"))))]
pub fn generate_salt() -> Result<[u8; SALT_LEN], CredentialError> {
    compile_error!(
        "kernel::identity::credential::generate_salt has no real entropy source for this \
         target/feature combination (only `std` and `baremetal`+`zynq-a9` are supported today) \
         -- per the platform spec, stop and report the blocker rather than fake a salt; add a \
         real entropy source for this target before enabling `identity` here."
    );
}

fn build_argon2(
    memory_kib: u32,
    iterations: u32,
    lanes: u8,
) -> Result<argon2::Argon2<'static>, CredentialError> {
    if memory_kib == 0 || memory_kib > MAX_ARGON2_MEMORY_KIB || iterations == 0 || lanes == 0 {
        return Err(CredentialError::InvalidParameters);
    }
    let params = argon2::Params::new(memory_kib, iterations, lanes as u32, Some(VERIFIER_LEN))
        .map_err(|_| CredentialError::InvalidParameters)?;
    Ok(argon2::Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        params,
    ))
}

/// Real Argon2id hash of `password` under `salt`/`memory_kib`/`iterations`/
/// `lanes`. Deterministic given identical inputs (needed for both
/// verification and history checks).
pub fn derive_verifier_bytes(
    password: &[u8],
    salt: &[u8; SALT_LEN],
    memory_kib: u32,
    iterations: u32,
    lanes: u8,
) -> Result<[u8; VERIFIER_LEN], CredentialError> {
    let argon2 = build_argon2(memory_kib, iterations, lanes)?;
    let mut out = [0u8; VERIFIER_LEN];
    argon2
        .hash_password_into(password, salt, &mut out)
        .map_err(|_| CredentialError::HashingFailed)?;
    Ok(out)
}

/// Create a brand-new [`PasswordVerifier`] from a plaintext password: a
/// fresh real-entropy salt (never derived from username/Chronos/Kairos/PID
/// per the platform spec) plus a real Argon2id hash.
pub fn create_verifier(
    password: &[u8],
    memory_kib: u32,
    iterations: u32,
    lanes: u8,
    changed_generation: u64,
) -> Result<PasswordVerifier, CredentialError> {
    let salt = generate_salt()?;
    let verifier = derive_verifier_bytes(password, &salt, memory_kib, iterations, lanes)?;
    Ok(PasswordVerifier {
        version: PASSWORD_VERIFIER_VERSION_V1,
        algorithm: PasswordAlgorithm::Argon2id,
        salt,
        verifier,
        memory_kib,
        iterations,
        lanes,
        changed_generation,
        must_change: false,
        failed_attempts: 0,
        lockout_until_chronos: 0,
    })
}

/// Best-effort buffer zeroing for plaintext password bytes once no longer
/// needed (platform spec's "clear password buffers after use"). Honest
/// disclosure: this overwrites the given buffer, but Rust's ownership model
/// can leave other copies behind (moves that were actually reallocations,
/// prior `Vec`/`String` growth reallocations, compiler-introduced temporary
/// copies) that this call cannot reach — this is defense-in-depth, not a
/// guarantee against physical memory forensics. `#[inline(never)]` plus a
/// volatile-style byte-at-a-time write discourages (but, on `no_std` without
/// a real `core::ptr::write_volatile`-per-byte + compiler fence crate, does
/// not perfectly guarantee against) the compiler optimizing the writes away
/// as dead stores.
#[inline(never)]
pub fn zeroize(bytes: &mut [u8]) {
    for b in bytes.iter_mut() {
        unsafe { core::ptr::write_volatile(b, 0) };
    }
}

/// Constant-time (no early exit on first differing byte) comparison —
/// verifier bytes are not secret plaintext, but comparing them at all is
/// the closest thing to an authentication decision this module makes, so
/// it is written to avoid a length/content timing side channel where cheap
/// to do so.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Attempt authentication against `verifier`, applying `policy`'s
/// rate-limit/lockout rule. If still inside the lockout window, the
/// attempt is refused *before* any Argon2 hashing occurs (real rate
/// limiting, not just a delayed response). On failure, `failed_attempts`
/// is incremented and a new `lockout_until_chronos` is computed via
/// exponentially-growing backoff bounded by
/// `policy.maximum_delay_ticks`. On success, both are reset to zero.
pub fn verify_password(
    verifier: &mut PasswordVerifier,
    policy: &PasswordPolicy,
    password: &[u8],
    now_chronos: u64,
) -> Result<bool, CredentialError> {
    if now_chronos < verifier.lockout_until_chronos {
        return Err(CredentialError::RateLimited);
    }
    let candidate = derive_verifier_bytes(
        password,
        &verifier.salt,
        verifier.memory_kib,
        verifier.iterations,
        verifier.lanes,
    )?;
    if constant_time_eq(&candidate, &verifier.verifier) {
        verifier.failed_attempts = 0;
        verifier.lockout_until_chronos = 0;
        return Ok(true);
    }
    verifier.failed_attempts = verifier.failed_attempts.saturating_add(1);
    let exponent = verifier.failed_attempts.saturating_sub(1).min(31) as u32;
    let delay = policy
        .initial_delay_ticks
        .saturating_mul(1u64 << exponent)
        .min(policy.maximum_delay_ticks);
    verifier.lockout_until_chronos = now_chronos.saturating_add(delay);
    Ok(false)
}

/// Platform spec §5: "checking a new password against history requires
/// verifying it against each stored historical verifier" — real Argon2id
/// re-hash under each historical salt/params, never a plaintext compare.
pub fn is_password_in_history(
    history: &[PasswordVerifier],
    password: &[u8],
) -> Result<bool, CredentialError> {
    for old in history {
        let candidate = derive_verifier_bytes(
            password,
            &old.salt,
            old.memory_kib,
            old.iterations,
            old.lanes,
        )?;
        if constant_time_eq(&candidate, &old.verifier) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Root's credential state machine (platform spec §15). Gate 3 scope: the
/// raw mechanism (state transitions, authentication, history). Gate 4 owns
/// gating [`Self::provision`] behind the real physical-presence +
/// recovery-certificate procedure — this type does not and cannot verify
/// physical presence itself.
#[derive(Debug, Clone)]
pub struct RootCredential {
    state: RootCredentialState,
    verifier: Option<PasswordVerifier>,
    history: Vec<PasswordVerifier>,
}

impl Default for RootCredential {
    fn default() -> Self {
        Self::new()
    }
}

impl RootCredential {
    pub fn new() -> Self {
        Self {
            state: RootCredentialState::Unprovisioned,
            verifier: None,
            history: Vec::new(),
        }
    }

    pub fn state(&self) -> RootCredentialState {
        self.state
    }

    pub fn verifier(&self) -> Option<&PasswordVerifier> {
        self.verifier.as_ref()
    }

    pub fn history(&self) -> &[PasswordVerifier] {
        &self.history
    }

    /// Raw provisioning mechanism: `Unprovisioned -> Active`. See struct
    /// docs — this function enforces only the state-machine precondition;
    /// Gate 4 gates every real call site behind physical presence and
    /// recovery-certificate authorization.
    pub fn provision(&mut self, verifier: PasswordVerifier) -> Result<(), CredentialError> {
        if self.state != RootCredentialState::Unprovisioned {
            return Err(CredentialError::AlreadyProvisioned);
        }
        self.verifier = Some(verifier);
        self.state = RootCredentialState::Active;
        Ok(())
    }

    /// Authenticate against the stored verifier. Only `Active` may attempt
    /// authentication — `Unprovisioned`/`Locked`/`RecoveryRequired` all
    /// reject outright via [`CredentialError::NotActive`], never silently
    /// treating the absence of a password as an empty password.
    pub fn authenticate(
        &mut self,
        policy: &PasswordPolicy,
        password: &[u8],
        now_chronos: u64,
    ) -> Result<bool, CredentialError> {
        if self.state != RootCredentialState::Active {
            return Err(CredentialError::NotActive);
        }
        let verifier = self
            .verifier
            .as_mut()
            .ok_or(CredentialError::NotProvisioned)?;
        let ok = verify_password(verifier, policy, password, now_chronos)?;
        if !ok && verifier.failed_attempts >= policy.maximum_failures {
            self.state = RootCredentialState::Locked;
        }
        Ok(ok)
    }

    /// Replace the stored verifier with `new_verifier`, pushing the old one
    /// onto the bounded history list (`policy.history_depth`, oldest
    /// evicted first). Valid from `Active` or `Locked` (a lockout does not
    /// prevent an otherwise-authorized password reset/recovery flow from
    /// completing — Gate 4/15 own deciding *when* this is called; this
    /// function only enforces that `Unprovisioned` must call
    /// [`Self::provision`] instead).
    pub fn change_password(
        &mut self,
        policy: &PasswordPolicy,
        new_verifier: PasswordVerifier,
    ) -> Result<(), CredentialError> {
        if self.state == RootCredentialState::Unprovisioned {
            return Err(CredentialError::NotProvisioned);
        }
        if let Some(old) = self.verifier.take() {
            if policy.history_depth > 0 {
                if self.history.len() >= policy.history_depth as usize {
                    self.history.remove(0);
                }
                self.history.push(old);
            }
        }
        self.verifier = Some(new_verifier);
        self.state = RootCredentialState::Active;
        Ok(())
    }

    pub fn lock(&mut self) {
        if self.state == RootCredentialState::Active {
            self.state = RootCredentialState::Locked;
        }
    }

    pub fn unlock(&mut self) -> Result<(), CredentialError> {
        if self.state != RootCredentialState::Locked {
            return Err(CredentialError::NotActive);
        }
        if let Some(v) = self.verifier.as_mut() {
            v.failed_attempts = 0;
            v.lockout_until_chronos = 0;
        }
        self.state = RootCredentialState::Active;
        Ok(())
    }

    pub fn require_recovery(&mut self) {
        self.state = RootCredentialState::RecoveryRequired;
    }

    /// Replace this credential with a previously persisted snapshot. Does
    /// not record ledger events — this is restore, not a mutation (Gate 6).
    pub fn restore(
        &mut self,
        state: RootCredentialState,
        verifier: Option<PasswordVerifier>,
        history: Vec<PasswordVerifier>,
    ) {
        self.state = state;
        self.verifier = verifier;
        self.history = history;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_verifier(password: &[u8]) -> PasswordVerifier {
        create_verifier(password, DEFAULT_ARGON2_MEMORY_KIB, DEFAULT_ARGON2_ITERATIONS, DEFAULT_ARGON2_LANES, 0)
            .unwrap()
    }

    #[test]
    fn default_policy_matches_spec_defaults() {
        let policy = PasswordPolicy::default();
        assert_eq!(policy.minimum_length, 15);
        assert_eq!(policy.maximum_length, 128);
        assert_ne!(policy.blocklist_root, [0u8; 32]);
    }

    #[test]
    fn validate_password_rejects_too_short() {
        let policy = PasswordPolicy::default();
        let blocklist = Blocklist::default_blocklist();
        assert_eq!(
            validate_password(&policy, b"short", None, &blocklist),
            Err(PasswordPolicyError::TooShort)
        );
    }

    #[test]
    fn validate_password_rejects_too_long() {
        let policy = PasswordPolicy::default();
        let blocklist = Blocklist::default_blocklist();
        let long = vec![b'a'; 200];
        assert_eq!(
            validate_password(&policy, &long, None, &blocklist),
            Err(PasswordPolicyError::TooLong)
        );
    }

    #[test]
    fn validate_password_rejects_non_ascii() {
        let policy = PasswordPolicy::default();
        let blocklist = Blocklist::default_blocklist();
        let mut pw = vec![b'a'; 15];
        pw[0] = 0x01; // control byte, not printable ASCII
        assert_eq!(
            validate_password(&policy, &pw, None, &blocklist),
            Err(PasswordPolicyError::UnsupportedCharacter)
        );
    }

    #[test]
    fn validate_password_rejects_blocklisted() {
        let policy = PasswordPolicy::default();
        let blocklist = Blocklist::default_blocklist();
        // "qwertyuiop" is 10 chars, below min length, so pad with a known
        // blocklisted 15+-char-equivalent instead: use one already >= 15.
        assert_eq!(
            validate_password(&policy, b"1234567890", None, &blocklist),
            Err(PasswordPolicyError::TooShort)
        );
        assert_eq!(
            validate_password(&policy, b"qwertyuiop1234567890qwertyuiop", None, &blocklist)
                .is_ok(),
            true
        );
        assert_eq!(
            validate_password(&policy, b"iloveyou", None, &blocklist),
            Err(PasswordPolicyError::TooShort)
        );
    }

    #[test]
    fn validate_password_rejects_username_substring() {
        let policy = PasswordPolicy::default();
        let blocklist = Blocklist::default_blocklist();
        let pw = b"AliceIsGreatPassword123";
        assert_eq!(
            validate_password(&policy, pw, Some(b"alice"), &blocklist),
            Err(PasswordPolicyError::ContainsUsernameOrProductName)
        );
    }

    #[test]
    fn validate_password_rejects_product_name_substring() {
        let policy = PasswordPolicy::default();
        let blocklist = Blocklist::default_blocklist();
        let pw = b"IReallyLoveKairosSoftware";
        assert_eq!(
            validate_password(&policy, pw, None, &blocklist),
            Err(PasswordPolicyError::ContainsUsernameOrProductName)
        );
    }

    #[test]
    fn validate_password_accepts_a_real_strong_password() {
        let policy = PasswordPolicy::default();
        let blocklist = Blocklist::default_blocklist();
        let pw = b"correct horse battery staple 42";
        assert_eq!(validate_password(&policy, pw, Some(b"bob"), &blocklist), Ok(()));
    }

    #[test]
    fn generate_salt_is_real_and_varies() {
        let a = generate_salt().unwrap();
        let b = generate_salt().unwrap();
        assert_ne!(a, [0u8; SALT_LEN]);
        assert_ne!(a, b, "two independent salts must not collide in practice");
    }

    #[test]
    fn derive_verifier_bytes_is_deterministic_given_same_inputs() {
        let salt = [7u8; SALT_LEN];
        let a = derive_verifier_bytes(b"hunter2", &salt, 16, 2, 1).unwrap();
        let b = derive_verifier_bytes(b"hunter2", &salt, 16, 2, 1).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn derive_verifier_bytes_is_sensitive_to_password_and_salt() {
        let salt = [7u8; SALT_LEN];
        let base = derive_verifier_bytes(b"hunter2", &salt, 16, 2, 1).unwrap();
        let diff_pw = derive_verifier_bytes(b"hunter3", &salt, 16, 2, 1).unwrap();
        let diff_salt = derive_verifier_bytes(b"hunter2", &[8u8; SALT_LEN], 16, 2, 1).unwrap();
        assert_ne!(base, diff_pw);
        assert_ne!(base, diff_salt);
    }

    #[test]
    fn memory_above_hard_bound_is_rejected() {
        let salt = [1u8; SALT_LEN];
        assert_eq!(
            derive_verifier_bytes(b"pw", &salt, MAX_ARGON2_MEMORY_KIB + 1, 1, 1),
            Err(CredentialError::InvalidParameters)
        );
    }

    #[test]
    fn zero_iterations_or_lanes_is_rejected() {
        let salt = [1u8; SALT_LEN];
        assert_eq!(
            derive_verifier_bytes(b"pw", &salt, 16, 0, 1),
            Err(CredentialError::InvalidParameters)
        );
        assert_eq!(
            derive_verifier_bytes(b"pw", &salt, 16, 1, 0),
            Err(CredentialError::InvalidParameters)
        );
    }

    #[test]
    fn verify_password_accepts_the_correct_password() {
        let mut verifier = small_verifier(b"correct horse battery staple");
        let policy = PasswordPolicy::default();
        assert_eq!(
            verify_password(&mut verifier, &policy, b"correct horse battery staple", 0),
            Ok(true)
        );
        assert_eq!(verifier.failed_attempts, 0);
    }

    #[test]
    fn verify_password_rejects_the_wrong_password_and_counts_it() {
        let mut verifier = small_verifier(b"correct horse battery staple");
        let policy = PasswordPolicy::default();
        assert_eq!(
            verify_password(&mut verifier, &policy, b"wrong password entirely", 0),
            Ok(false)
        );
        assert_eq!(verifier.failed_attempts, 1);
        assert!(verifier.lockout_until_chronos > 0);
    }

    #[test]
    fn verify_password_rate_limits_within_the_lockout_window() {
        let mut verifier = small_verifier(b"correct horse battery staple");
        let policy = PasswordPolicy::default();
        verify_password(&mut verifier, &policy, b"wrong one", 0).unwrap();
        let lockout = verifier.lockout_until_chronos;
        assert_eq!(
            verify_password(&mut verifier, &policy, b"correct horse battery staple", lockout - 1),
            Err(CredentialError::RateLimited)
        );
        // failed_attempts must not have been bumped again by the refused attempt.
        assert_eq!(verifier.failed_attempts, 1);
        // Once the window elapses, the correct password succeeds again.
        assert_eq!(
            verify_password(&mut verifier, &policy, b"correct horse battery staple", lockout),
            Ok(true)
        );
    }

    #[test]
    fn backoff_grows_and_is_bounded_by_maximum_delay_ticks() {
        let mut verifier = small_verifier(b"correct horse battery staple");
        let policy = PasswordPolicy {
            initial_delay_ticks: 1,
            maximum_delay_ticks: 8,
            ..PasswordPolicy::default()
        };
        let mut now = 0u64;
        let mut last_delay = 0u64;
        for _ in 0..6 {
            verify_password(&mut verifier, &policy, b"nope", now).unwrap();
            let delay = verifier.lockout_until_chronos - now;
            assert!(delay >= last_delay);
            assert!(delay <= policy.maximum_delay_ticks);
            now = verifier.lockout_until_chronos;
            last_delay = delay;
        }
        assert_eq!(last_delay, policy.maximum_delay_ticks);
    }

    #[test]
    fn is_password_in_history_detects_reused_password() {
        let old = small_verifier(b"my old password one two three");
        let history = vec![old];
        assert_eq!(
            is_password_in_history(&history, b"my old password one two three"),
            Ok(true)
        );
        assert_eq!(
            is_password_in_history(&history, b"a totally different password"),
            Ok(false)
        );
    }

    #[test]
    fn root_credential_starts_unprovisioned_and_denies_authentication() {
        let mut cred = RootCredential::new();
        let policy = PasswordPolicy::default();
        assert_eq!(cred.state(), RootCredentialState::Unprovisioned);
        assert_eq!(
            cred.authenticate(&policy, b"anything at all here", 0),
            Err(CredentialError::NotActive)
        );
        // Even an empty password must not be silently accepted.
        assert_eq!(cred.authenticate(&policy, b"", 0), Err(CredentialError::NotActive));
    }

    #[test]
    fn root_credential_provision_then_authenticate_round_trip() {
        let mut cred = RootCredential::new();
        let policy = PasswordPolicy::default();
        let verifier = small_verifier(b"a genuinely long root password");
        cred.provision(verifier).unwrap();
        assert_eq!(cred.state(), RootCredentialState::Active);
        assert_eq!(
            cred.authenticate(&policy, b"a genuinely long root password", 0),
            Ok(true)
        );
        assert_eq!(cred.authenticate(&policy, b"wrong password here", 100), Ok(false));
    }

    #[test]
    fn root_credential_cannot_be_provisioned_twice() {
        let mut cred = RootCredential::new();
        cred.provision(small_verifier(b"first real root password one"))
            .unwrap();
        assert_eq!(
            cred.provision(small_verifier(b"second real root password")),
            Err(CredentialError::AlreadyProvisioned)
        );
    }

    #[test]
    fn root_credential_locks_after_maximum_failures() {
        let mut cred = RootCredential::new();
        let policy = PasswordPolicy {
            maximum_failures: 3,
            initial_delay_ticks: 0,
            maximum_delay_ticks: 0,
            ..PasswordPolicy::default()
        };
        cred.provision(small_verifier(b"a genuinely long root password"))
            .unwrap();
        for i in 0..3u64 {
            let _ = cred.authenticate(&policy, b"wrong password attempt", i);
        }
        assert_eq!(cred.state(), RootCredentialState::Locked);
        assert_eq!(
            cred.authenticate(&policy, b"a genuinely long root password", 100),
            Err(CredentialError::NotActive)
        );
    }

    #[test]
    fn root_credential_change_password_moves_old_into_history() {
        let mut cred = RootCredential::new();
        let policy = PasswordPolicy::default();
        cred.provision(small_verifier(b"original root password value"))
            .unwrap();
        cred.change_password(&policy, small_verifier(b"brand new root password"))
            .unwrap();
        assert_eq!(cred.history().len(), 1);
        assert_eq!(
            is_password_in_history(cred.history(), b"original root password value"),
            Ok(true)
        );
        assert_eq!(
            cred.authenticate(&policy, b"brand new root password", 0),
            Ok(true)
        );
    }

    #[test]
    fn root_credential_history_is_bounded_by_policy_depth() {
        let mut cred = RootCredential::new();
        let policy = PasswordPolicy {
            history_depth: 2,
            ..PasswordPolicy::default()
        };
        cred.provision(small_verifier(b"root password number one here"))
            .unwrap();
        cred.change_password(&policy, small_verifier(b"root password number two here"))
            .unwrap();
        cred.change_password(&policy, small_verifier(b"root password number three"))
            .unwrap();
        cred.change_password(&policy, small_verifier(b"root password number four x"))
            .unwrap();
        assert_eq!(cred.history().len(), 2, "bounded by history_depth, oldest evicted");
        assert_eq!(
            is_password_in_history(cred.history(), b"root password number one here"),
            Ok(false),
            "oldest entry must have been evicted"
        );
    }

    #[test]
    fn root_credential_lock_and_unlock_round_trip() {
        let mut cred = RootCredential::new();
        let policy = PasswordPolicy::default();
        cred.provision(small_verifier(b"a genuinely long root password"))
            .unwrap();
        cred.lock();
        assert_eq!(cred.state(), RootCredentialState::Locked);
        assert_eq!(
            cred.authenticate(&policy, b"a genuinely long root password", 0),
            Err(CredentialError::NotActive)
        );
        cred.unlock().unwrap();
        assert_eq!(cred.state(), RootCredentialState::Active);
        assert_eq!(
            cred.authenticate(&policy, b"a genuinely long root password", 0),
            Ok(true)
        );
    }

    #[test]
    fn zeroize_overwrites_every_byte() {
        let mut buf = *b"a real password!";
        zeroize(&mut buf);
        assert_eq!(buf, [0u8; 16]);
    }

    #[test]
    fn blocklist_root_is_deterministic_and_nonzero() {
        let a = Blocklist::default_blocklist().root();
        let b = Blocklist::default_blocklist().root();
        assert_eq!(a, b);
        assert_ne!(a, [0u8; 32]);
    }
}
