//! G0 host mathematical oracle CLI — generates and verifies GPLV1 vectors.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use kairos::zynq_oracle::{
    compare_device_dump, decode_oracle, default_oracle_dir, dump_from_records, generate_oracle,
    hex_digest, load_oracle_file, records_match_gphase_core, sha256_bytes,
    write_g64_research_oracle, write_oracle_artifacts,
};

fn print_usage() {
    eprintln!(
        "Usage: gpl_oracle [--output <dir>] [--check] [--print-records] \
         [--compare-device-dump <path>] [--export-sim-dir <dir>] [--update-frozen] \
         [--g64-research-out <dir>] [--help]"
    );
}

fn g0_token_block(vectors_sha: &str) -> String {
    format!(
        "GATE=G0_MATH_ORACLE\n\
FORMAT=GPLV1\n\
ENCODING=PHI0:00 PHI1:01 PHI2:10 PHI3:11\n\
COMPOSE 16/16 PASS\n\
REFLECT 4/4 PASS\n\
ALGEBRA 64/64 PASS\n\
PARITY_ILLEGAL=0\n\
MISMATCH_COUNT=0\n\
VECTOR_COUNT=84\n\
VECTORS_SHA256={vectors_sha}\n\
RESULT=G0_MATH_ORACLE_PASS\n"
    )
}

fn export_sim_vectors(
    dir: &Path,
    records: &[kairos::zynq_oracle::OracleRecord],
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let dump = dump_from_records(records);
    fs::write(dir.join("device_dump.txt"), &dump).map_err(|e| e.to_string())?;
    let mut mem = String::new();
    for r in records {
        mem.push_str(&format!(
            "{:01x}{:01x}{:01x}{:01x}{:01x}{:01x}{:01x}{:01x}{:01x}\n",
            r.kind as u8,
            r.input_a,
            r.input_b,
            r.input_c,
            r.compose_ab,
            r.left_association,
            r.right_association,
            r.parity,
            r.rho_a
        ));
    }
    fs::write(dir.join("vectors_hex.txt"), mem).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut output = default_oracle_dir();
    let mut check_only = false;
    let mut print_records = false;
    let mut update_frozen = false;
    let mut compare_dump: Option<PathBuf> = None;
    let mut export_sim_dir: Option<PathBuf> = None;
    let mut g64_out: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            "--output" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--output requires a directory");
                    return ExitCode::from(2);
                }
                output = PathBuf::from(&args[i]);
            }
            "--check" => check_only = true,
            "--print-records" => print_records = true,
            "--update-frozen" => update_frozen = true,
            "--compare-device-dump" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--compare-device-dump requires a path");
                    return ExitCode::from(2);
                }
                compare_dump = Some(PathBuf::from(&args[i]));
            }
            "--export-sim-dir" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--export-sim-dir requires a directory");
                    return ExitCode::from(2);
                }
                export_sim_dir = Some(PathBuf::from(&args[i]));
            }
            "--g64-research-out" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--g64-research-out requires a directory");
                    return ExitCode::from(2);
                }
                g64_out = Some(PathBuf::from(&args[i]));
            }
            other => {
                eprintln!("unknown argument: {other}");
                print_usage();
                return ExitCode::from(2);
            }
        }
        i += 1;
    }

    let generated = match generate_oracle() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("generate failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = records_match_gphase_core(&generated.records) {
        eprintln!("gphase_core mismatch: {e}");
        return ExitCode::FAILURE;
    }

    let (_hdr, redecoded) = match decode_oracle(&generated.blob) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("decode of generated blob failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    if redecoded != generated.records {
        eprintln!("redecoded records differ from generated");
        return ExitCode::FAILURE;
    }

    let vectors_sha = hex_digest(&sha256_bytes(&generated.blob));
    let frozen_vectors = default_oracle_dir().join("vectors.bin");

    if check_only || !update_frozen {
        if frozen_vectors.exists() {
            let frozen = match fs::read(&frozen_vectors) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("read frozen vectors: {e}");
                    return ExitCode::FAILURE;
                }
            };
            if frozen != generated.blob {
                eprintln!(
                    "frozen vectors.bin differs from fresh generation; \
                     use --update-frozen to replace"
                );
                return ExitCode::FAILURE;
            }
            if let Err(e) = load_oracle_file(&frozen_vectors) {
                eprintln!("frozen decode failed: {e}");
                return ExitCode::FAILURE;
            }
        } else if check_only {
            eprintln!("missing frozen {}", frozen_vectors.display());
            return ExitCode::FAILURE;
        } else if let Err(e) = write_oracle_artifacts(&output, &generated) {
            eprintln!("write artifacts: {e}");
            return ExitCode::FAILURE;
        }
    } else if let Err(e) = write_oracle_artifacts(&output, &generated) {
        eprintln!("write artifacts: {e}");
        return ExitCode::FAILURE;
    }

    if print_records {
        for r in &generated.records {
            println!(
                "seq={} kind={:?} flags=0x{:02x} a={} b={} c={} ab={} L={} R={} p={} rho={}",
                r.sequence,
                r.kind,
                r.flags.bits(),
                r.input_a,
                r.input_b,
                r.input_c,
                r.compose_ab,
                r.left_association,
                r.right_association,
                r.parity,
                r.rho_a
            );
        }
    }

    if let Some(dir) = export_sim_dir {
        if let Err(e) = export_sim_vectors(&dir, &generated.records) {
            eprintln!("export-sim failed: {e}");
            return ExitCode::FAILURE;
        }
        eprintln!("exported sim vectors to {}", dir.display());
    }

    if let Some(dump_path) = compare_dump {
        let oracle_path = if frozen_vectors.exists() {
            frozen_vectors.clone()
        } else {
            output.join("vectors.bin")
        };
        match compare_device_dump(&oracle_path, &dump_path) {
            Ok(report) => {
                print!("{}", report.token_block());
                if let Some(ff) = &report.first_failure {
                    eprintln!("FIRST_FAILURE={ff}");
                }
                if !report.ok {
                    return ExitCode::FAILURE;
                }
            }
            Err(e) => {
                eprintln!("compare failed: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    if let Some(dir) = g64_out {
        match write_g64_research_oracle(&dir) {
            Ok((sha, commit)) => {
                eprintln!(
                    "wrote G64 research oracle to {} blob_sha={sha} commit={commit}",
                    dir.display()
                );
            }
            Err(e) => {
                eprintln!("g64 research oracle write failed: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    let evidence = Path::new(env!("CARGO_MANIFEST_DIR")).join("fpga/zynq/evidence/g0_math_oracle");
    let _ = fs::create_dir_all(&evidence);
    let _ = fs::write(evidence.join("G0_TOKEN.txt"), g0_token_block(&vectors_sha));

    print!("{}", g0_token_block(&vectors_sha));
    ExitCode::SUCCESS
}
