// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Append Ed25519(BootImageRoot) to a Zynq `BOOT.BIN` for Stage 1.

use std::{env, fs, process};

use kairos::board::zynq_boot_auth::{dev_only_seed, sign_image_root};
use kairos::board::zynq_bootimg::{
    loader_bitstream_app, partition_bytes, ps_ddr_partitions, MAX_PS_DDR,
};
use kairos::kernel::crypto::phase_hash::{ph_boot_image_root, ph_content};

fn main() {
    let mut args = env::args().skip(1);
    let (Some(inp), Some(outp)) = (args.next(), args.next()) else {
        eprintln!("usage: stage1_sign IN_BOOT.BIN OUT_BOOT.BIN");
        process::exit(1);
    };
    let img = fs::read(&inp).expect("read");
    let (loader, bit, app) = loader_bitstream_app(&img).expect("parse BOOT.BIN");
    let lb = partition_bytes(&img, &loader).unwrap();
    let bb = partition_bytes(&img, &bit).unwrap();
    let mut ddr = [app; MAX_PS_DDR];
    let nd = ps_ddr_partitions(&img, &mut ddr).expect("ps ddr");
    let mut k = Vec::new();
    for p in ddr.iter().take(nd) {
        k.extend_from_slice(partition_bytes(&img, p).unwrap());
    }
    let root = ph_boot_image_root(&ph_content(lb), &ph_content(bb), &ph_content(&k));
    let sig = sign_image_root(&dev_only_seed(), &root);
    let mut out = img;
    out.extend_from_slice(&sig);
    fs::write(&outp, &out).expect("write");
    println!("signed {} bytes -> {}", out.len(), outp);
}
