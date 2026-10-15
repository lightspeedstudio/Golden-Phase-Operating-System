//! Product branding — Golden Phase Operating System (GPOS).
//!
//! Internal crate/package id remains `kairos` for build stability.
//! User-facing name is Golden Phase Operating System / gφlden phase φpertating system.
//! Discrete algebra values are **Kairos States** (Kairos0..Kairos3).

/// Full product name.
pub const OS_NAME: &str = "Golden Phase Operating System";

/// Stylized product line (phi in place of o).
pub const OS_STYLIZED: &str = "gφlden phase φpertating system";

/// Short product id for prompts and tags.
pub const OS_SHORT: &str = "GPOS";

/// Shell prompt.
pub const SHELL_PROMPT: &str = "gpos> ";

/// Family name for the four discrete algebra states.
pub const KAIROS_STATES: &str = "Kairos States";

/// Boot splash first-line marker.
pub const BOOT_SPLASH: &str = "Golden Phase Operating System boot splash";

/// Operator console one-liner.
pub const SPLASH: &str = "Golden Phase Operating System — Kairos States operator console";

/// Build/target label for splash, `version`, and `host`.
///
/// Reflects the **physical host CPU** (compile-time `target_arch`), not the
/// GP VM guest ISA. Hosted `std` builds always report `hosted-std`.
pub fn build_target_label(hosted: bool) -> &'static str {
    if hosted {
        "hosted-std"
    } else if cfg!(target_arch = "arm") {
        "baremetal-armv7-a-cortex-a9"
    } else if cfg!(target_arch = "riscv64") {
        "baremetal-riscv64"
    } else if cfg!(target_arch = "aarch64") {
        "baremetal-aarch64"
    } else {
        "baremetal-unknown-arch"
    }
}
