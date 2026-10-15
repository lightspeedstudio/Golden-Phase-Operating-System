//! Optional hosted GPU backend descriptor.

pub fn backend_name() -> &'static str {
    "gpu"
}

pub fn available() -> bool {
    false
}
