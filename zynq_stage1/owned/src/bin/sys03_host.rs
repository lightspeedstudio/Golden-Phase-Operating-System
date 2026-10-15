// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Host: emit owned PS7 trace and artifact hashes. No COM12.

use std::fs;
use std::path::PathBuf;
use zynq_stage1_owned::platform::{sys03_ops, Op};
use zynq_stage1_owned::sha256::sha256;

fn hex32(h: [u8; 32]) -> String {
    h.iter().map(|b| format!("{:02x}", b)).collect()
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut jsonl = String::new();
    for (seq, op) in sys03_ops().enumerate() {
        let line = match op {
            Op::Write { addr, value } => format!(
                "{{\"addr\": \"0x{addr:08X}\", \"op\": \"WRITE\", \"seq\": {seq}, \"value\": \"0x{value:08X}\"}}\n"
            ),
            Op::Clear { addr } => format!(
                "{{\"addr\": \"0x{addr:08X}\", \"op\": \"CLEAR\", \"seq\": {seq}}}\n"
            ),
            Op::MaskWrite { addr, mask, value } => format!(
                "{{\"addr\": \"0x{addr:08X}\", \"mask\": \"0x{mask:08X}\", \"op\": \"MASKWRITE\", \"seq\": {seq}, \"value\": \"0x{value:08X}\"}}\n"
            ),
            Op::Poll { addr, mask } => format!(
                "{{\"addr\": \"0x{addr:08X}\", \"mask\": \"0x{mask:08X}\", \"op\": \"MASKPOLL\", \"seq\": {seq}}}\n"
            ),
            Op::DelayMs { ms } => format!(
                "{{\"addr\": \"0xF8F00200\", \"delay_ms\": {ms}, \"op\": \"MASKDELAY\", \"seq\": {seq}}}\n"
            ),
        };
        jsonl.push_str(&line);
    }
    let out = root.join("docs/boot/SYS03_OWNED_PS7_TRACE.jsonl");
    fs::write(&out, &jsonl).unwrap();

    let mut report = String::from("# System 3 artifact hashes\n\n");
    for rel in [
        "docs/boot/SYS03_PS7_REFERENCE.md",
        "docs/boot/SYS03_WORKING_PS7_TRACE.jsonl",
        "docs/boot/SYS03_OWNED_PS7_TRACE.expected.jsonl",
        "docs/boot/SYS03_OWNED_PS7_TRACE.jsonl",
        "zynq_stage1/src/main.rs",
        "zynq_stage1/owned/src/platform/entry.rs",
        "zynq_stage1/owned/src/start.S",
    ] {
        let p = root.join(rel);
        if !p.exists() {
            continue;
        }
        let b = fs::read(&p).unwrap();
        report.push_str(&format!(
            "| `{rel}` | `{}` | {} |\n",
            hex32(sha256(&b)),
            b.len()
        ));
    }
    let hashes = root.join("docs/boot/SYS03_ARTIFACT_HASHES.txt");
    fs::write(&hashes, report).unwrap();
    print!("{}", fs::read_to_string(hashes).unwrap());
}
