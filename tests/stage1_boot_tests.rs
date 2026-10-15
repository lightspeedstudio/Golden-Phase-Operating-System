// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
use kairos::board::zynq_boot_auth::{
    dev_only_seed, sign_image_root, split_signed, verify_image_root, STAGE1_ROOT_PUBKEY,
};
use kairos::board::zynq_bootimg::{loader_bitstream_app, partition_bytes};
use kairos::kernel::crypto::phase_hash::{ph_boot_image_root, ph_content};
use kairos::kernel::crypto::sign::SignKeypair;

#[test]
fn gold_path1_boot_bin_has_loader_bit_app() {
    let boot = include_bytes!("../fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN");
    let (loader, bit, app) = loader_bitstream_app(boot).expect("gold BOOT.BIN");
    assert!(loader.is_ps() && loader.load_addr == 0);
    assert!(bit.is_pl());
    assert_eq!(app.load_addr, 0x0010_0000);
    assert_eq!(app.exec_addr, 0x0010_0060);
    assert!(!bit.is_encrypted());
    assert!(!app.has_vendor_rsa());
}

#[test]
fn signed_trailer_roundtrip_on_gold_payloads() {
    let boot = include_bytes!("../fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN");
    let (loader, bit, app) = loader_bitstream_app(boot).unwrap();
    let root = ph_boot_image_root(
        &ph_content(partition_bytes(boot, &loader).unwrap()),
        &ph_content(partition_bytes(boot, &bit).unwrap()),
        &ph_content(partition_bytes(boot, &app).unwrap()),
    );
    let seed = dev_only_seed();
    let pk = SignKeypair::from_seed(&seed).public_bytes();
    assert_eq!(pk, STAGE1_ROOT_PUBKEY);
    let sig = sign_image_root(&seed, &root);
    assert!(verify_image_root(&STAGE1_ROOT_PUBKEY, &root, &sig));
    let mut signed = boot.to_vec();
    signed.extend_from_slice(&sig);
    let (img, trail) = split_signed(&signed).unwrap();
    assert_eq!(img.len(), boot.len());
    assert_eq!(trail, &sig);
}

#[test]
fn unsigned_gold_does_not_verify() {
    let boot = include_bytes!("../fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN");
    let (loader, bit, app) = loader_bitstream_app(boot).unwrap();
    let root = ph_boot_image_root(
        &ph_content(partition_bytes(boot, &loader).unwrap()),
        &ph_content(partition_bytes(boot, &bit).unwrap()),
        &ph_content(partition_bytes(boot, &app).unwrap()),
    );
    let sig: [u8; 64] = boot[boot.len() - 64..].try_into().unwrap();
    assert!(!verify_image_root(&STAGE1_ROOT_PUBKEY, &root, &sig));
}
