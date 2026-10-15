//! Optional NEON backend descriptor.

pub fn backend_name() -> &'static str {
    "neon"
}

#[cfg(all(feature = "std", any(target_arch = "aarch64", target_arch = "arm")))]
pub fn available() -> bool {
    true
}

#[cfg(not(all(feature = "std", any(target_arch = "aarch64", target_arch = "arm"))))]
pub fn available() -> bool {
    false
}
