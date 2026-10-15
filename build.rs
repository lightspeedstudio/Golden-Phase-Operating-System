fn main() {
    if std::env::var("CARGO_FEATURE_BAREMETAL").is_ok() {
        let target = std::env::var("TARGET").unwrap_or_default();
        if target.starts_with("riscv64") {
            println!("cargo:rerun-if-changed=src/boot/linker.ld");
            println!("cargo:rustc-link-arg=-Tsrc/boot/linker.ld");
        } else if target.starts_with("armv7a") {
            if std::env::var("CARGO_FEATURE_MMU_APP_ISOLATION").is_ok() {
                println!("cargo:rerun-if-changed=src/arch/armv7/linker_iso.ld");
                // GNU note at 0x00100000 so vectors are +0x40 and `_start` is 0x00100060.
                println!("cargo:rustc-link-arg=-Wl,--build-id");
                println!("cargo:rustc-link-arg=-Tsrc/arch/armv7/linker_iso.ld");
            } else {
                println!("cargo:rerun-if-changed=src/arch/armv7/linker.ld");
                println!("cargo:rustc-link-arg=-Tsrc/arch/armv7/linker.ld");
            }
        }
    }
}
