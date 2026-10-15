//! Offline hosted PhaseStack packer. Never built into `zynq-a9` / `utility`.

use kairos::kernel::crypto::sign::SignKeypair;
use kairos::kernel::phasestack::cert::{issue_leaf_cert, issue_root_cert_with};
use kairos::kernel::phasestack::parser::{encode_gpa, parse_gpa};
use kairos::kernel::phasestack::types::AppVersion;
use kairos::sdk::phasestack::{hello_app, pack_native_elf};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

fn load_seed(path: &PathBuf) -> [u8; 32] {
    let bytes = fs::read(path).unwrap_or_else(|e| {
        eprintln!("read {}: {e}", path.display());
        process::exit(1);
    });
    if bytes.len() != 32 {
        eprintln!("seed must be 32 bytes, got {}", bytes.len());
        process::exit(1);
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&bytes);
    seed
}

fn pack_app(sk_path: &PathBuf, out_path: &PathBuf, app: kairos::sdk::phasestack::PhaseApplication) {
    let root_kp = SignKeypair::from_seed(&load_seed(sk_path));
    let root = issue_root_cert_with(1, &root_kp);
    let leaf_kp = SignKeypair::from_seed(&load_seed(sk_path));
    let leaf = issue_leaf_cert(2, &leaf_kp, &root, &root_kp, None);
    let bytes = app.pack(&leaf_kp, &leaf, &[root]).expect("pack");
    if let Some(dir) = out_path.parent() {
        if !dir.as_os_str().is_empty() {
            let _ = fs::create_dir_all(dir);
        }
    }
    fs::write(out_path, &bytes).expect("write gpa");
    println!("wrote {} ({} bytes)", out_path.display(), bytes.len());
}

fn hex32(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() {
    let mut args = env::args().skip(1);
    match args.next().unwrap_or_default().as_str() {
        "pack-hello" => {
            let sk_path = PathBuf::from(args.next().expect("usage: pack-hello <root.sk> <out.gpa>"));
            let out_path = PathBuf::from(args.next().expect("usage: pack-hello <root.sk> <out.gpa>"));
            pack_app(&sk_path, &out_path, hello_app());
        }
        "pack-over" => {
            let sk_path = PathBuf::from(args.next().expect("usage: pack-over <root.sk> <out.gpa>"));
            let out_path = PathBuf::from(args.next().expect("usage: pack-over <root.sk> <out.gpa>"));
            let mut app = hello_app();
            app.name = "over-cap-v1".into();
            app.requested_caps = kairos::kernel::captable::RIGHT_READ;
            pack_app(&sk_path, &out_path, app);
        }
        "pack-elf" => {
            let sk_path = PathBuf::from(args.next().expect("usage: pack-elf <root.sk> <out.gpa>"));
            let out_path = PathBuf::from(args.next().expect("usage: pack-elf <root.sk> <out.gpa>"));
            #[cfg(feature = "mmu-app-isolation")]
            {
                let elf = kairos::kernel::phasestack::native::encode_a32_exec_elf(
                    &kairos::arch::armv7::payloads::legal_read(),
                )
                .expect("elf");
                let root_kp = SignKeypair::from_seed(&load_seed(&sk_path));
                let root = issue_root_cert_with(1, &root_kp);
                let leaf_kp = SignKeypair::from_seed(&load_seed(&sk_path));
                let leaf = issue_leaf_cert(2, &leaf_kp, &root, &root_kp, None);
                let bytes = pack_native_elf(
                    "legal-elf",
                    AppVersion {
                        major: 1,
                        minor: 0,
                        patch: 0,
                    },
                    &elf,
                    &leaf_kp,
                    &leaf,
                    &[root],
                )
                .expect("pack");
                if let Some(dir) = out_path.parent() {
                    if !dir.as_os_str().is_empty() {
                        let _ = fs::create_dir_all(dir);
                    }
                }
                fs::write(&out_path, &bytes).expect("write gpa");
                println!("wrote {} ({} bytes)", out_path.display(), bytes.len());
            }
            #[cfg(not(feature = "mmu-app-isolation"))]
            {
                let _ = (sk_path, out_path);
                eprintln!("pack-elf needs feature mmu-app-isolation");
                process::exit(2);
            }
        }
        "pubkey" => {
            let sk_path = PathBuf::from(args.next().expect("usage: pubkey <root.sk>"));
            let kp = SignKeypair::from_seed(&load_seed(&sk_path));
            println!("{}", hex32(&kp.public_bytes()));
        }
        "unsigned" => {
            let in_path = PathBuf::from(args.next().expect("usage: unsigned <in.gpa> <out.gpa>"));
            let out_path = PathBuf::from(args.next().expect("usage: unsigned <in.gpa> <out.gpa>"));
            let bytes = fs::read(&in_path).expect("read gpa");
            let mut parsed = parse_gpa(&bytes).expect("parse gpa");
            let sig = parsed
                .entries
                .iter_mut()
                .find(|e| e.name == "package.sig")
                .expect("package.sig");
            sig.data = vec![0u8; 32];
            let out = encode_gpa(&parsed.entries).expect("encode");
            fs::write(&out_path, &out).expect("write gpa");
            println!("wrote {} ({} bytes)", out_path.display(), out.len());
        }
        _ => {
            eprintln!(
                "usage: phasestack_sign pack-hello|pack-over|pack-elf|pubkey|unsigned ..."
            );
            process::exit(2);
        }
    }
}
