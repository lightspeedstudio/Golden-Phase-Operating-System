//! Emit G64 research oracle JSON/MD beside GPLV1 binary artifacts.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::core::gphase_core::{
    compose, left_group, reflect, right_group, topology_status, PhaseState, TopologyKind,
};

use super::format::{hex_digest, sha256_bytes};
use super::generate::{generate_oracle, GenerateError};

#[derive(Debug, Clone)]
struct TripleRow {
    a: u8,
    b: u8,
    c: u8,
    left: u8,
    right: u8,
    class: &'static str,
}

pub fn write_g64_research_oracle(dir: &Path) -> Result<(String, String), GenerateError> {
    fs::create_dir_all(dir)?;
    let oracle = generate_oracle()?;
    let blob_sha = hex_digest(&sha256_bytes(&oracle.blob));
    let commit = git_head_sha().unwrap_or_else(|| "UNKNOWN".into());

    let mut compose_pairs = Vec::new();
    for a in 0u8..4 {
        for b in 0u8..4 {
            compose_pairs.push((a, b, compose(ps(a), ps(b)).as_u8()));
        }
    }
    let mut reflects = Vec::new();
    for a in 0u8..4 {
        reflects.push((a, reflect(ps(a)).as_u8()));
    }

    let mut triples = Vec::new();
    let mut id_mask = 0u64;
    let mut ref_mask = 0u64;
    let err_mask = 0u64;
    let mut idx = 0u32;
    let mut n_id = 0u32;
    let mut n_ref = 0u32;
    let n_err = 0u32;
    for a in 0u8..4 {
        for b in 0u8..4 {
            for c in 0u8..4 {
                let left = left_group(ps(a), ps(b), ps(c)).as_u8();
                let right = right_group(ps(a), ps(b), ps(c)).as_u8();
                let class = match topology_status(ps(a), ps(b), ps(c)).kind {
                    TopologyKind::Same => {
                        id_mask |= 1u64 << idx;
                        n_id += 1;
                        "IDENTITY"
                    }
                    TopologyKind::Reflection => {
                        ref_mask |= 1u64 << idx;
                        n_ref += 1;
                        "REFLECTION"
                    }
                };
                triples.push(TripleRow {
                    a,
                    b,
                    c,
                    left,
                    right,
                    class,
                });
                idx += 1;
            }
        }
    }

    let json = format!(
        "{{\n  \"source\": \"kairos::core::gphase_core\",\n  \"source_commit\": \"{commit}\",\n  \
\"oracle_blob_sha256\": \"{blob_sha}\",\n  \"compose_count\": 16,\n  \"reflect_count\": 4,\n  \
\"triple_count\": 64,\n  \"census\": {{\"identity\": {n_id}, \"reflection\": {n_ref}, \"error\": {n_err}}},\n  \
\"identity_mask\": \"0x{id_mask:016X}\",\n  \"reflection_mask\": \"0x{ref_mask:016X}\",\n  \
\"error_mask\": \"0x{err_mask:016X}\",\n  \"compose\": [\n{}\n  ],\n  \"reflect\": [\n{}\n  ],\n  \
\"triples\": [\n{}\n  ]\n}}\n",
        compose_pairs
            .iter()
            .map(|(a, b, r)| format!("    {{\"a\":{a},\"b\":{b},\"result\":{r}}}"))
            .collect::<Vec<_>>()
            .join(",\n"),
        reflects
            .iter()
            .map(|(a, r)| format!("    {{\"a\":{a},\"result\":{r}}}"))
            .collect::<Vec<_>>()
            .join(",\n"),
        triples
            .iter()
            .map(|t| format!(
                "    {{\"a\":{},\"b\":{},\"c\":{},\"left\":{},\"right\":{},\"class\":\"{}\"}}",
                t.a, t.b, t.c, t.left, t.right, t.class
            ))
            .collect::<Vec<_>>()
            .join(",\n")
    );

    let md = format!(
        "# Golden Phase Oracle (G64 research)\n\n\
Source: `gphase_core` @ `{commit}`\n\n\
Oracle blob SHA-256: `{blob_sha}`\n\n\
## Census (from live canonical code)\n\n\
| Class | Count |\n|-------|-------|\n\
| IDENTITY | {n_id} |\n\
| REFLECTION | {n_ref} |\n\
| ERROR | {n_err} |\n\n\
Masks: identity=`0x{id_mask:016X}` reflection=`0x{ref_mask:016X}` error=`0x{err_mask:016X}`\n\n\
Do not hardcode these counts into hardware pass criteria — regenerate and compare.\n"
    );

    fs::write(dir.join("golden_phase_oracle.json"), &json)?;
    fs::write(dir.join("golden_phase_oracle.md"), &md)?;
    fs::write(dir.join("golden_phase_oracle.bin"), &oracle.blob)?;

    let json_sha = hex_digest(&sha256_bytes(json.as_bytes()));
    let md_sha = hex_digest(&sha256_bytes(md.as_bytes()));
    let digests = format!(
        "{blob_sha}  golden_phase_oracle.bin\n{json_sha}  golden_phase_oracle.json\n{md_sha}  golden_phase_oracle.md\n\
SOURCE_COMMIT={commit}\n"
    );
    fs::write(dir.join("SHA256SUMS.txt"), digests)?;
    Ok((blob_sha, commit))
}

fn ps(v: u8) -> PhaseState {
    PhaseState::from_u8(v).expect("0..3")
}

fn git_head_sha() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
