//! Optional AVX-512 backend descriptor.

pub fn backend_name() -> &'static str {
    "avx512"
}

#[cfg(all(feature = "std", any(target_arch = "x86", target_arch = "x86_64")))]
pub fn available() -> bool {
    std::is_x86_feature_detected!("avx512f")
}

#[cfg(not(all(feature = "std", any(target_arch = "x86", target_arch = "x86_64"))))]
pub fn available() -> bool {
    false
}
