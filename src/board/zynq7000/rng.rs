//! Custom `getrandom` backend for bare-metal SSH (sunset needs a CSPRNG for
//! key exchange, and `armv7a-none-eabi` has no OS entropy source).
//!
//! `HARDWARE_TRNG: UNAVAILABLE_ON_CURRENT_BOARD`
//! `CURRENT_SOURCE: TIMER_JITTER_SEEDED_DRBG`
//!
//! The Zynq-7000 PS has **no hardware TRNG**. This backend seeds a SHA-256
//! counter-mode DRBG from Cortex-A9 global-timer reads interleaved with jitter
//! loops. That is **not cryptographic hardware entropy**. A future board may
//! call [`reseed`] from an external/custom-board TRNG provider. Do not treat
//! these keys as high-assurance.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use sha2::{Digest, Sha256};

/// Honest source name. Never "TRNG".
pub const CURRENT_SOURCE: &str = "TIMER_JITTER_SEEDED_DRBG";
pub const HARDWARE_TRNG: &str = "UNAVAILABLE_ON_CURRENT_BOARD";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntropyPolicy {
    AllowTimerJitter,
    HardwareRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntropyError {
    HardwareUnavailable,
    Stuck,
    Repeated,
}

impl EntropyError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HardwareUnavailable => "entropy: HARDWARE_TRNG unavailable (fail closed)",
            Self::Stuck => "entropy: stuck timer samples",
            Self::Repeated => "entropy: repeated seed block",
        }
    }
}

static POLICY: AtomicU32 = AtomicU32::new(0); // 0 = AllowTimerJitter
static LAST_HEALTH_OK: AtomicBool = AtomicBool::new(true);
static LAST_SEED0: AtomicU64 = AtomicU64::new(0);
static STUCK_FAILS: AtomicU32 = AtomicU32::new(0);
static HEALTH_OKS: AtomicU32 = AtomicU32::new(0);

// Cortex-A9 global timer (same registers as kernel::net_ip).
const GTC_LO: u32 = 0xF8F0_0200;
const GTC_HI: u32 = 0xF8F0_0204;

/// Monotonic call counter so identical timer states never repeat an output.
static CALL_CTR: AtomicU64 = AtomicU64::new(0);
/// Optional externally-supplied entropy (mixed into every seed). Set via
/// [`reseed`] once a real TRNG is available.
static EXTRA_ENTROPY: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);

#[inline]
fn read_gtc() -> u64 {
    unsafe {
        loop {
            let hi = core::ptr::read_volatile(GTC_HI as *const u32);
            let lo = core::ptr::read_volatile(GTC_LO as *const u32);
            let hi2 = core::ptr::read_volatile(GTC_HI as *const u32);
            if hi == hi2 {
                return ((hi as u64) << 32) | lo as u64;
            }
        }
    }
}

/// Mix an external entropy word (e.g. from a hardware TRNG) into future seeds.
pub fn reseed(word: u64) {
    let cur = EXTRA_ENTROPY.load(Ordering::Relaxed);
    EXTRA_ENTROPY.store(
        cur.rotate_left(17) ^ word.wrapping_mul(0xD1B5_4A32_D192_ED03),
        Ordering::Relaxed,
    );
}

pub fn set_policy(p: EntropyPolicy) {
    POLICY.store(
        match p {
            EntropyPolicy::AllowTimerJitter => 0,
            EntropyPolicy::HardwareRequired => 1,
        },
        Ordering::SeqCst,
    );
}

pub fn policy() -> EntropyPolicy {
    if POLICY.load(Ordering::SeqCst) == 0 {
        EntropyPolicy::AllowTimerJitter
    } else {
        EntropyPolicy::HardwareRequired
    }
}

fn health_samples(samples: &[u64]) -> Result<(), EntropyError> {
    if samples.len() < 4 {
        return Err(EntropyError::Stuck);
    }
    let first = samples[0];
    if samples.iter().all(|&s| s == first) {
        STUCK_FAILS.fetch_add(1, Ordering::SeqCst);
        LAST_HEALTH_OK.store(false, Ordering::SeqCst);
        return Err(EntropyError::Stuck);
    }
    let mut prev = samples[0];
    let mut repeats = 0u32;
    for &s in &samples[1..] {
        if s == prev {
            repeats += 1;
        }
        prev = s;
    }
    if repeats + 1 == samples.len() as u32 {
        STUCK_FAILS.fetch_add(1, Ordering::SeqCst);
        LAST_HEALTH_OK.store(false, Ordering::SeqCst);
        return Err(EntropyError::Repeated);
    }
    HEALTH_OKS.fetch_add(1, Ordering::SeqCst);
    LAST_HEALTH_OK.store(true, Ordering::SeqCst);
    Ok(())
}

/// Gather a 32-byte seed from timer jitter + call/extra state.
fn gather_seed() -> Result<[u8; 32], EntropyError> {
    if policy() == EntropyPolicy::HardwareRequired {
        return Err(EntropyError::HardwareUnavailable);
    }
    let ctr = CALL_CTR.fetch_add(1, Ordering::Relaxed);
    let mut h = Sha256::new();
    h.update(ctr.to_le_bytes());
    h.update(EXTRA_ENTROPY.load(Ordering::Relaxed).to_le_bytes());
    let mut samples = [0u64; 8];
    for i in 0..8 {
        let t = read_gtc();
        samples[i] = t;
        h.update(t.to_le_bytes());
        let spins = (t as u8 & 0x3f) as u32;
        for _ in 0..spins {
            core::hint::spin_loop();
        }
    }
    health_samples(&samples)?;
    let seed: [u8; 32] = h.finalize().into();
    let word = u64::from_le_bytes(seed[0..8].try_into().unwrap_or([0; 8]));
    let prev = LAST_SEED0.swap(word, Ordering::SeqCst);
    if prev != 0 && prev == word {
        LAST_HEALTH_OK.store(false, Ordering::SeqCst);
        return Err(EntropyError::Repeated);
    }
    Ok(seed)
}

/// SHA-256 counter-mode expansion of the seed into `dest`.
pub fn fill(dest: &mut [u8]) {
    let _ = fill_checked(dest);
}

/// Fail closed when policy requires hardware entropy or health tests fail.
pub fn fill_checked(dest: &mut [u8]) -> Result<(), EntropyError> {
    let seed = gather_seed()?;
    let mut block: u64 = 0;
    let mut off = 0;
    while off < dest.len() {
        let mut h = Sha256::new();
        h.update(seed);
        h.update(block.to_le_bytes());
        let out = h.finalize();
        let n = core::cmp::min(out.len(), dest.len() - off);
        dest[off..off + n].copy_from_slice(&out[..n]);
        off += n;
        block = block.wrapping_add(1);
    }
    Ok(())
}

pub fn status_lines() -> crate::platform::String {
    alloc::format!(
        "HARDWARE_TRNG: {}\nCURRENT_SOURCE: {}\npolicy: {}\nhealth: {}\nstuck_fails: {}\nhealth_ok: {}\n",
        HARDWARE_TRNG,
        CURRENT_SOURCE,
        match policy() {
            EntropyPolicy::AllowTimerJitter => "allow-timer-jitter",
            EntropyPolicy::HardwareRequired => "hardware-required (fail closed)",
        },
        if LAST_HEALTH_OK.load(Ordering::SeqCst) {
            "ok"
        } else {
            "fail"
        },
        STUCK_FAILS.load(Ordering::SeqCst),
        HEALTH_OKS.load(Ordering::SeqCst),
    )
}

/// `getrandom` custom backend entry point (`sunset`/SSH's own CSPRNG need).
/// Gated to `ssh-bm` specifically — the `getrandom` crate dependency (and
/// its one-per-binary global registration) is only pulled in for that
/// feature; `identity`'s own entropy use calls [`fill`] directly instead
/// (see that function's doc).
#[cfg(feature = "ssh-bm")]
pub fn board_getrandom(dest: &mut [u8]) -> Result<(), getrandom::Error> {
    fill_checked(dest).map_err(|_| {
        getrandom::Error::from(core::num::NonZeroU32::new(0x5f00).expect("nonzero"))
    })
}

#[cfg(feature = "ssh-bm")]
getrandom::register_custom_getrandom!(board_getrandom);
