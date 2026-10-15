//! Optional FPGA backend descriptor.

pub fn backend_name() -> &'static str {
    "fpga"
}

pub fn available() -> bool {
    false
}
