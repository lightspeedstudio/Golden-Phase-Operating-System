//! Optional AVX2 backend descriptor.

pub fn backend_name() -> &'static str {
    "avx2"
}

#[cfg(all(feature = "std", any(target_arch = "x86", target_arch = "x86_64")))]
pub fn available() -> bool {
    std::is_x86_feature_detected!("avx2")
}

#[cfg(not(all(feature = "std", any(target_arch = "x86", target_arch = "x86_64"))))]
pub fn available() -> bool {
    false
}
