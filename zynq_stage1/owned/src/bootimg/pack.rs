// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Pack a Zynq-7000 `BOOT.BIN` that follows the gold header geometry.
//!
//! Later payloads are 512-byte aligned. Field `0x030` is Source Offset.
//! Register-init unused addresses are `0xFFFFFFFF`. Refuses `zynq_fsbl.elf`.

use super::frame::{
    check_encrypted_image, parse_allow_encrypted, ENC_STATUS_BBRAM, PART_ALIGN,
};
use super::parse::{
    parse, parse_all_product, parse_all_rsa_later, partition_header_checksum, u32le, ATTR_PL,
    ATTR_PS, ATTR_RSA,
    HEADER_WINDOW_LEN, IHT_OFFSET, IMAGE_IDENT_XLNX, MAX_PARTITIONS, OFF_IMAGE_LEN, OFF_REG_INIT,
    OFF_SOURCE_OFFSET, PAYLOAD_ALIGN, PARTITION_HEADER_LEN, PH_OFFSET, WIDTH_DETECT,
};
use super::stream::{encrypt_partition, StreamError, StreamKeys};
use crate::sha256::sha256;
use crate::{validate_bootloader_artifact, ArtifactError};

const VEC_WORD: u32 = 0xEAFF_FFFE;
const ATTR_WORD: u32 = 0x0101_0000;
const QSPI_WORD: u32 = 0x0000_0001;
const STAGE1_OFF: usize = 0x1700;
const IH0: usize = 0x900;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackError {
    Empty,
    FsblFilename,
    QuarantinedHash,
    Unaligned,
    Frame,
    Auth,
    Parse(super::parse::ParseError),
}

impl From<ArtifactError> for PackError {
    fn from(e: ArtifactError) -> Self {
        match e {
            ArtifactError::FsblFilename => PackError::FsblFilename,
            ArtifactError::QuarantinedHash => PackError::QuarantinedHash,
        }
    }
}

impl From<crate::rsa::Error> for PackError {
    fn from(_: crate::rsa::Error) -> Self {
        PackError::Auth
    }
}

impl From<StreamError> for PackError {
    fn from(e: StreamError) -> Self {
        match e {
            StreamError::Empty => PackError::Empty,
            StreamError::Unaligned => PackError::Unaligned,
            StreamError::Header | StreamError::Length | StreamError::Trailer | StreamError::Auth
            | StreamError::Bitstream => PackError::Frame,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Part<'a> {
    pub name: &'a str,
    pub data: &'a [u8],
    pub load_addr: u32,
    pub exec_addr: u32,
    pub attributes: u32,
}

fn align_up(n: usize, a: usize) -> usize {
    (n + a - 1) & !(a - 1)
}

fn pad4(data: &[u8]) -> Vec<u8> {
    let mut v = data.to_vec();
    while v.len() % 4 != 0 {
        v.push(0);
    }
    v
}

fn put_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn name_words(name: &str) -> [u32; 4] {
    let mut raw = [0u8; 16];
    let b = name.as_bytes();
    let n = b.len().min(16);
    raw[..n].copy_from_slice(&b[..n]);
    [
        u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]),
        u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]),
        u32::from_be_bytes([raw[8], raw[9], raw[10], raw[11]]),
        u32::from_be_bytes([raw[12], raw[13], raw[14], raw[15]]),
    ]
}

fn write_ph(
    buf: &mut [u8],
    off: usize,
    p: &Part,
    data_off: usize,
    enc_words: u32,
    data_words: u32,
    total_words: u32,
    ih_words: u32,
) {
    let mut w = [0u32; 16];
    w[0] = enc_words;
    w[1] = data_words;
    w[2] = total_words;
    w[3] = p.load_addr;
    w[4] = p.exec_addr;
    w[5] = (data_off / 4) as u32;
    w[6] = p.attributes;
    w[7] = 1;
    w[9] = ih_words;
    w[15] = partition_header_checksum(&w);
    for i in 0..16 {
        put_u32(buf, off + i * 4, w[i]);
    }
}

fn write_ih(buf: &mut [u8], off: usize, next_ih_words: u32, ph_words: u32, name: &str) {
    put_u32(buf, off, next_ih_words);
    put_u32(buf, off + 4, ph_words);
    put_u32(buf, off + 12, 1);
    let nw = name_words(name);
    for i in 0..4 {
        put_u32(buf, off + 16 + i * 4, nw[i]);
    }
    // Measured Bootgen: 16-byte name. If the last name word is used, the next
    // word is 0 (NUL). Otherwise unused words are 0xFFFFFFFF.
    let word8 = if nw[3] != 0 { 0 } else { 0xFFFF_FFFF };
    put_u32(buf, off + 32, word8);
    for i in 9..16 {
        put_u32(buf, off + i * 4, 0xFFFF_FFFF);
    }
}

fn write_null_ph(buf: &mut [u8], off: usize) {
    let mut w = [0u32; 16];
    w[15] = partition_header_checksum(&w);
    for i in 0..16 {
        put_u32(buf, off + i * 4, w[i]);
    }
}

fn boot_header_checksum(buf: &[u8]) -> u32 {
    let mut s = 0u32;
    let mut off = 0x20;
    while off <= 0x44 {
        s = s.wrapping_add(u32le(buf, off).unwrap());
        off += 4;
    }
    !s
}

/// Pack Stage 1 + bitstream + application. Refuses quarantined FSBL.
pub fn pack(stage1: Part, bitstream: Part, app: Part) -> Result<Vec<u8>, PackError> {
    for p in [stage1, bitstream, app] {
        if p.data.is_empty() {
            return Err(PackError::Empty);
        }
        validate_bootloader_artifact(p.name, &sha256(p.data))?;
    }
    let s1 = pad4(stage1.data);
    let bit = pad4(bitstream.data);
    let ps = pad4(app.data);
    let s1_off = STAGE1_OFF;
    let bit_off = align_up(s1_off + s1.len(), PAYLOAD_ALIGN);
    let app_off = align_up(bit_off + bit.len(), PAYLOAD_ALIGN);
    if bit_off % PAYLOAD_ALIGN != 0 || app_off % PAYLOAD_ALIGN != 0 {
        return Err(PackError::Unaligned);
    }
    let total = app_off + ps.len();
    let mut buf = vec![0u8; total];
    for i in 0..8 {
        put_u32(&mut buf, i * 4, VEC_WORD);
    }
    put_u32(&mut buf, 0x20, WIDTH_DETECT);
    put_u32(&mut buf, 0x24, IMAGE_IDENT_XLNX);
    put_u32(&mut buf, 0x28, 0);
    put_u32(&mut buf, 0x2C, ATTR_WORD);
    put_u32(&mut buf, OFF_SOURCE_OFFSET, s1_off as u32);
    put_u32(&mut buf, OFF_IMAGE_LEN, s1.len() as u32);
    put_u32(&mut buf, 0x38, 0);
    put_u32(&mut buf, 0x3C, 0);
    put_u32(&mut buf, 0x40, s1.len() as u32);
    put_u32(&mut buf, 0x44, QSPI_WORD);
    put_u32(&mut buf, 0x98, IHT_OFFSET as u32);
    put_u32(&mut buf, 0x9C, PH_OFFSET as u32);
    let mut off = OFF_REG_INIT;
    while off + 8 <= 0x8A0 {
        put_u32(&mut buf, off, 0xFFFF_FFFF);
        put_u32(&mut buf, off + 4, 0);
        off += 8;
    }
    while off < IHT_OFFSET {
        put_u32(&mut buf, off, 0xFFFF_FFFF);
        off += 4;
    }
    put_u32(&mut buf, IHT_OFFSET, 0x0102_0000);
    put_u32(&mut buf, IHT_OFFSET + 4, 3);
    put_u32(&mut buf, IHT_OFFSET + 8, (PH_OFFSET / 4) as u32);
    put_u32(&mut buf, IHT_OFFSET + 12, (IH0 / 4) as u32);
    put_u32(&mut buf, IHT_OFFSET + 16, 0);
    for i in 5..16 {
        put_u32(&mut buf, IHT_OFFSET + i * 4, 0xFFFF_FFFF);
    }
    write_ih(&mut buf, IH0, (IH0 as u32 + 0x40) / 4, (PH_OFFSET / 4) as u32, stage1.name);
    write_ih(
        &mut buf,
        IH0 + 0x40,
        (IH0 as u32 + 0x80) / 4,
        ((PH_OFFSET + 0x40) / 4) as u32,
        bitstream.name,
    );
    write_ih(&mut buf, IH0 + 0x80, 0, ((PH_OFFSET + 0x80) / 4) as u32, app.name);
    write_ph(
        &mut buf,
        PH_OFFSET,
        &stage1,
        s1_off,
        (s1.len() / 4) as u32,
        (s1.len() / 4) as u32,
        (s1.len() / 4) as u32,
        (IH0 / 4) as u32,
    );
    write_ph(
        &mut buf,
        PH_OFFSET + 0x40,
        &bitstream,
        bit_off,
        (bit.len() / 4) as u32,
        (bit.len() / 4) as u32,
        (bit.len() / 4) as u32,
        ((IH0 + 0x40) / 4) as u32,
    );
    write_ph(
        &mut buf,
        PH_OFFSET + 0x80,
        &app,
        app_off,
        (ps.len() / 4) as u32,
        (ps.len() / 4) as u32,
        (ps.len() / 4) as u32,
        ((IH0 + 0x80) / 4) as u32,
    );
    let csum = boot_header_checksum(&buf);
    put_u32(&mut buf, 0x48, csum);
    buf[s1_off..s1_off + s1.len()].copy_from_slice(&s1);
    buf[bit_off..bit_off + bit.len()].copy_from_slice(&bit);
    buf[app_off..app_off + ps.len()].copy_from_slice(&ps);
    let parsed = parse(&buf).map_err(PackError::Parse)?;
    if parsed.source_offset as usize != s1_off {
        return Err(PackError::Unaligned);
    }
    if parsed.partitions[1].data_off % PAYLOAD_ALIGN != 0
        || parsed.partitions[2].data_off % PAYLOAD_ALIGN != 0
    {
        return Err(PackError::Unaligned);
    }
    Ok(buf)
}

/// Unsigned P0 + RSA-authenticated later partition. Gold `parse()` returns `VendorRsa`.
pub fn pack_rsa_later(
    stage1: Part,
    later: Part,
    ppk_n: &[u8; 256],
    psk_d: &[u8; 256],
    spk_n: &[u8; 256],
    ssk_d: &[u8; 256],
) -> Result<Vec<u8>, PackError> {
    if stage1.data.is_empty() || later.data.is_empty() {
        return Err(PackError::Empty);
    }
    validate_bootloader_artifact(stage1.name, &sha256(stage1.data))?;
    validate_bootloader_artifact(later.name, &sha256(later.data))?;
    if (stage1.attributes & ATTR_RSA) != 0 {
        return Err(PackError::Auth);
    }
    let s1 = pad4(stage1.data);
    let mut p1 = later.data.to_vec();
    while p1.len() % 64 != 0 {
        p1.push(0);
    }
    let ac = crate::rsa::cert::sign_later_partition(ppk_n, psk_d, spk_n, ssk_d, &p1)?;
    let mut stored = p1.clone();
    stored.extend_from_slice(&ac);
    let s1_off = STAGE1_OFF;
    let p1_off = align_up(s1_off + s1.len(), PAYLOAD_ALIGN);
    let total = p1_off + stored.len();
    let mut buf = vec![0u8; total];
    for i in 0..8 {
        put_u32(&mut buf, i * 4, VEC_WORD);
    }
    put_u32(&mut buf, 0x20, WIDTH_DETECT);
    put_u32(&mut buf, 0x24, IMAGE_IDENT_XLNX);
    put_u32(&mut buf, 0x28, 0);
    put_u32(&mut buf, 0x2C, ATTR_WORD);
    put_u32(&mut buf, OFF_SOURCE_OFFSET, s1_off as u32);
    put_u32(&mut buf, OFF_IMAGE_LEN, s1.len() as u32);
    put_u32(&mut buf, 0x38, 0);
    put_u32(&mut buf, 0x3C, 0);
    put_u32(&mut buf, 0x40, s1.len() as u32);
    put_u32(&mut buf, 0x44, QSPI_WORD);
    put_u32(&mut buf, 0x98, IHT_OFFSET as u32);
    put_u32(&mut buf, 0x9C, PH_OFFSET as u32);
    let mut off = OFF_REG_INIT;
    while off + 8 <= 0x8A0 {
        put_u32(&mut buf, off, 0xFFFF_FFFF);
        put_u32(&mut buf, off + 4, 0);
        off += 8;
    }
    while off < IHT_OFFSET {
        put_u32(&mut buf, off, 0xFFFF_FFFF);
        off += 4;
    }
    put_u32(&mut buf, IHT_OFFSET, 0x0102_0000);
    put_u32(&mut buf, IHT_OFFSET + 4, 2);
    put_u32(&mut buf, IHT_OFFSET + 8, (PH_OFFSET / 4) as u32);
    put_u32(&mut buf, IHT_OFFSET + 12, (IH0 / 4) as u32);
    put_u32(&mut buf, IHT_OFFSET + 16, 0);
    for i in 5..16 {
        put_u32(&mut buf, IHT_OFFSET + i * 4, 0xFFFF_FFFF);
    }
    write_ih(
        &mut buf,
        IH0,
        (IH0 as u32 + 0x40) / 4,
        (PH_OFFSET / 4) as u32,
        stage1.name,
    );
    write_ih(&mut buf, IH0 + 0x40, 0, ((PH_OFFSET + 0x40) / 4) as u32, later.name);
    let later_part = Part {
        name: later.name,
        data: &p1,
        load_addr: later.load_addr,
        exec_addr: later.exec_addr,
        attributes: later.attributes | ATTR_RSA | ATTR_PS,
    };
    write_ph(
        &mut buf,
        PH_OFFSET,
        &stage1,
        s1_off,
        (s1.len() / 4) as u32,
        (s1.len() / 4) as u32,
        (s1.len() / 4) as u32,
        (IH0 / 4) as u32,
    );
    write_ph(
        &mut buf,
        PH_OFFSET + 0x40,
        &later_part,
        p1_off,
        (p1.len() / 4) as u32,
        (p1.len() / 4) as u32,
        (stored.len() / 4) as u32,
        ((IH0 + 0x40) / 4) as u32,
    );
    let csum = boot_header_checksum(&buf);
    put_u32(&mut buf, 0x48, csum);
    buf[s1_off..s1_off + s1.len()].copy_from_slice(&s1);
    buf[p1_off..p1_off + stored.len()].copy_from_slice(&stored);
    let win = core::cmp::min(HEADER_WINDOW_LEN, buf.len());
    let parsed = parse_all_rsa_later(&buf[..win], win, buf.len()).map_err(PackError::Parse)?;
    if parsed.partition_count != 2 || parsed.partitions[0].data_off != s1_off {
        return Err(PackError::Unaligned);
    }
    if (parsed.partitions[1].attributes & ATTR_RSA) == 0 {
        return Err(PackError::Auth);
    }
    crate::rsa::cert::verify_later_partition(&p1, &ac)?;
    Ok(buf)
}

/// Unsigned P0 + RSA-authenticated KAT + plaintext bitstream + plaintext application.
///
/// Gold `parse()` returns `VendorRsa`. `parse_all_product` accepts the image.
pub fn pack_rsa_later_product(
    stage1: Part,
    kat: Part,
    bitstream: Part,
    app: Part,
    ppk_n: &[u8; 256],
    psk_d: &[u8; 256],
    spk_n: &[u8; 256],
    ssk_d: &[u8; 256],
) -> Result<Vec<u8>, PackError> {
    for p in [stage1, kat, bitstream, app] {
        if p.data.is_empty() {
            return Err(PackError::Empty);
        }
        validate_bootloader_artifact(p.name, &sha256(p.data))?;
    }
    if (stage1.attributes & ATTR_RSA) != 0
        || (bitstream.attributes & ATTR_RSA) != 0
        || (app.attributes & ATTR_RSA) != 0
    {
        return Err(PackError::Auth);
    }
    let s1 = pad4(stage1.data);
    let mut kat_pt = kat.data.to_vec();
    while kat_pt.len() % 64 != 0 {
        kat_pt.push(0);
    }
    let ac = crate::rsa::cert::sign_later_partition(ppk_n, psk_d, spk_n, ssk_d, &kat_pt)?;
    let mut kat_stored = kat_pt.clone();
    kat_stored.extend_from_slice(&ac);
    let bit = pad4(bitstream.data);
    let ps = pad4(app.data);
    let s1_off = STAGE1_OFF;
    let kat_off = align_up(s1_off + s1.len(), PAYLOAD_ALIGN);
    let bit_off = align_up(kat_off + kat_stored.len(), PAYLOAD_ALIGN);
    let app_off = align_up(bit_off + bit.len(), PAYLOAD_ALIGN);
    if kat_off % PAYLOAD_ALIGN != 0
        || bit_off % PAYLOAD_ALIGN != 0
        || app_off % PAYLOAD_ALIGN != 0
    {
        return Err(PackError::Unaligned);
    }
    let total = app_off + ps.len();
    let mut buf = vec![0u8; total];
    for i in 0..8 {
        put_u32(&mut buf, i * 4, VEC_WORD);
    }
    put_u32(&mut buf, 0x20, WIDTH_DETECT);
    put_u32(&mut buf, 0x24, IMAGE_IDENT_XLNX);
    put_u32(&mut buf, 0x28, 0);
    put_u32(&mut buf, 0x2C, ATTR_WORD);
    put_u32(&mut buf, OFF_SOURCE_OFFSET, s1_off as u32);
    put_u32(&mut buf, OFF_IMAGE_LEN, s1.len() as u32);
    put_u32(&mut buf, 0x38, 0);
    put_u32(&mut buf, 0x3C, 0);
    put_u32(&mut buf, 0x40, s1.len() as u32);
    put_u32(&mut buf, 0x44, QSPI_WORD);
    put_u32(&mut buf, 0x98, IHT_OFFSET as u32);
    put_u32(&mut buf, 0x9C, PH_OFFSET as u32);
    let mut off = OFF_REG_INIT;
    while off + 8 <= 0x8A0 {
        put_u32(&mut buf, off, 0xFFFF_FFFF);
        put_u32(&mut buf, off + 4, 0);
        off += 8;
    }
    while off < IHT_OFFSET {
        put_u32(&mut buf, off, 0xFFFF_FFFF);
        off += 4;
    }
    put_u32(&mut buf, IHT_OFFSET, 0x0102_0000);
    put_u32(&mut buf, IHT_OFFSET + 4, 4);
    put_u32(&mut buf, IHT_OFFSET + 8, (PH_OFFSET / 4) as u32);
    put_u32(&mut buf, IHT_OFFSET + 12, (IH0 / 4) as u32);
    put_u32(&mut buf, IHT_OFFSET + 16, 0);
    for i in 5..16 {
        put_u32(&mut buf, IHT_OFFSET + i * 4, 0xFFFF_FFFF);
    }
    write_ih(
        &mut buf,
        IH0,
        (IH0 as u32 + 0x40) / 4,
        (PH_OFFSET / 4) as u32,
        stage1.name,
    );
    write_ih(
        &mut buf,
        IH0 + 0x40,
        (IH0 as u32 + 0x80) / 4,
        ((PH_OFFSET + 0x40) / 4) as u32,
        kat.name,
    );
    write_ih(
        &mut buf,
        IH0 + 0x80,
        (IH0 as u32 + 0xC0) / 4,
        ((PH_OFFSET + 0x80) / 4) as u32,
        bitstream.name,
    );
    write_ih(
        &mut buf,
        IH0 + 0xC0,
        0,
        ((PH_OFFSET + 0xC0) / 4) as u32,
        app.name,
    );
    let kat_part = Part {
        name: kat.name,
        data: &kat_pt,
        load_addr: kat.load_addr,
        exec_addr: kat.exec_addr,
        attributes: kat.attributes | ATTR_RSA | ATTR_PS,
    };
    write_ph(
        &mut buf,
        PH_OFFSET,
        &stage1,
        s1_off,
        (s1.len() / 4) as u32,
        (s1.len() / 4) as u32,
        (s1.len() / 4) as u32,
        (IH0 / 4) as u32,
    );
    write_ph(
        &mut buf,
        PH_OFFSET + 0x40,
        &kat_part,
        kat_off,
        (kat_pt.len() / 4) as u32,
        (kat_pt.len() / 4) as u32,
        (kat_stored.len() / 4) as u32,
        ((IH0 + 0x40) / 4) as u32,
    );
    write_ph(
        &mut buf,
        PH_OFFSET + 0x80,
        &bitstream,
        bit_off,
        (bit.len() / 4) as u32,
        (bit.len() / 4) as u32,
        (bit.len() / 4) as u32,
        ((IH0 + 0x80) / 4) as u32,
    );
    write_ph(
        &mut buf,
        PH_OFFSET + 0xC0,
        &app,
        app_off,
        (ps.len() / 4) as u32,
        (ps.len() / 4) as u32,
        (ps.len() / 4) as u32,
        ((IH0 + 0xC0) / 4) as u32,
    );
    let csum = boot_header_checksum(&buf);
    put_u32(&mut buf, 0x48, csum);
    buf[s1_off..s1_off + s1.len()].copy_from_slice(&s1);
    buf[kat_off..kat_off + kat_stored.len()].copy_from_slice(&kat_stored);
    buf[bit_off..bit_off + bit.len()].copy_from_slice(&bit);
    buf[app_off..app_off + ps.len()].copy_from_slice(&ps);
    if parse(&buf) != Err(super::parse::ParseError::VendorRsa) {
        return Err(PackError::Auth);
    }
    let win = core::cmp::min(HEADER_WINDOW_LEN, buf.len());
    let parsed = parse_all_product(&buf[..win], win, buf.len()).map_err(PackError::Parse)?;
    if parsed.partition_count != 4 || parsed.partitions[0].data_off != s1_off {
        return Err(PackError::Unaligned);
    }
    if (parsed.partitions[1].attributes & ATTR_RSA) == 0 {
        return Err(PackError::Auth);
    }
    if parsed.partitions[1].data_off % PAYLOAD_ALIGN != 0
        || parsed.partitions[2].data_off % PAYLOAD_ALIGN != 0
        || parsed.partitions[3].data_off % PAYLOAD_ALIGN != 0
    {
        return Err(PackError::Unaligned);
    }
    crate::rsa::cert::verify_later_partition(&kat_pt, &ac)?;
    Ok(buf)
}

/// Pack encrypted Stage 1 + encrypted bitstream + plaintext application.
///
/// Gold `parse()` still rejects this image. Use `parse_allow_encrypted`.
pub fn pack_secure(
    stage1: Part,
    bitstream: Part,
    app: Part,
    keys: &StreamKeys,
) -> Result<Vec<u8>, PackError> {
    for p in [stage1, bitstream, app] {
        if p.data.is_empty() {
            return Err(PackError::Empty);
        }
        validate_bootloader_artifact(p.name, &sha256(p.data))?;
    }
    let s1_plain = pad4(stage1.data);
    let bit_plain = pad4(bitstream.data);
    let ps = pad4(app.data);
    let s1 = encrypt_partition(&s1_plain, false, keys)?;
    let bit = encrypt_partition(&bit_plain, (bitstream.attributes & ATTR_PL) != 0, keys)?;
    let s1_off = STAGE1_OFF;
    let bit_off = align_up(s1_off + s1.len(), PART_ALIGN);
    let app_off = align_up(bit_off + bit.len(), PART_ALIGN);
    if bit_off % PART_ALIGN != 0 || app_off % PART_ALIGN != 0 {
        return Err(PackError::Unaligned);
    }
    let total = app_off + ps.len();
    let mut buf = vec![0u8; total];
    for i in 0..8 {
        put_u32(&mut buf, i * 4, VEC_WORD);
    }
    put_u32(&mut buf, 0x20, WIDTH_DETECT);
    put_u32(&mut buf, 0x24, IMAGE_IDENT_XLNX);
    put_u32(&mut buf, 0x28, ENC_STATUS_BBRAM);
    put_u32(&mut buf, 0x2C, ATTR_WORD);
    put_u32(&mut buf, OFF_SOURCE_OFFSET, s1_off as u32);
    put_u32(&mut buf, OFF_IMAGE_LEN, s1_plain.len() as u32);
    put_u32(&mut buf, 0x38, 0);
    put_u32(&mut buf, 0x3C, 0);
    put_u32(&mut buf, 0x40, s1.len() as u32);
    put_u32(&mut buf, 0x44, QSPI_WORD);
    put_u32(&mut buf, 0x98, IHT_OFFSET as u32);
    put_u32(&mut buf, 0x9C, PH_OFFSET as u32);
    let mut off = OFF_REG_INIT;
    while off + 8 <= 0x8A0 {
        put_u32(&mut buf, off, 0xFFFF_FFFF);
        put_u32(&mut buf, off + 4, 0);
        off += 8;
    }
    while off < IHT_OFFSET {
        put_u32(&mut buf, off, 0xFFFF_FFFF);
        off += 4;
    }
    put_u32(&mut buf, IHT_OFFSET, 0x0102_0000);
    put_u32(&mut buf, IHT_OFFSET + 4, 3);
    put_u32(&mut buf, IHT_OFFSET + 8, (PH_OFFSET / 4) as u32);
    put_u32(&mut buf, IHT_OFFSET + 12, (IH0 / 4) as u32);
    put_u32(&mut buf, IHT_OFFSET + 16, 0);
    for i in 5..16 {
        put_u32(&mut buf, IHT_OFFSET + i * 4, 0xFFFF_FFFF);
    }
    write_ih(&mut buf, IH0, (IH0 as u32 + 0x40) / 4, (PH_OFFSET / 4) as u32, stage1.name);
    write_ih(
        &mut buf,
        IH0 + 0x40,
        (IH0 as u32 + 0x80) / 4,
        ((PH_OFFSET + 0x40) / 4) as u32,
        bitstream.name,
    );
    write_ih(&mut buf, IH0 + 0x80, 0, ((PH_OFFSET + 0x80) / 4) as u32, app.name);
    write_ph(
        &mut buf,
        PH_OFFSET,
        &stage1,
        s1_off,
        (s1.len() / 4) as u32,
        (s1_plain.len() / 4) as u32,
        (s1.len() / 4) as u32,
        (IH0 / 4) as u32,
    );
    write_ph(
        &mut buf,
        PH_OFFSET + 0x40,
        &bitstream,
        bit_off,
        (bit.len() / 4) as u32,
        (bit_plain.len() / 4) as u32,
        (bit.len() / 4) as u32,
        ((IH0 + 0x40) / 4) as u32,
    );
    write_ph(
        &mut buf,
        PH_OFFSET + 0x80,
        &app,
        app_off,
        (ps.len() / 4) as u32,
        (ps.len() / 4) as u32,
        (ps.len() / 4) as u32,
        ((IH0 + 0x80) / 4) as u32,
    );
    let csum = boot_header_checksum(&buf);
    put_u32(&mut buf, 0x48, csum);
    buf[s1_off..s1_off + s1.len()].copy_from_slice(&s1);
    buf[bit_off..bit_off + bit.len()].copy_from_slice(&bit);
    buf[app_off..app_off + ps.len()].copy_from_slice(&ps);
    let parsed = parse_allow_encrypted(&buf).map_err(PackError::Parse)?;
    if parsed.source_offset as usize != s1_off {
        return Err(PackError::Unaligned);
    }
    if parsed.partitions[0].enc_words == parsed.partitions[0].data_words {
        return Err(PackError::Unaligned);
    }
    check_encrypted_image(&buf).map_err(|_| PackError::Frame)?;
    Ok(buf)
}

/// Pack encrypted partitions plus one trailing plaintext partition.
///
/// Gold `parse()` still rejects this image. Use `parse_allow_encrypted`.
pub fn pack_secure_image(
    encrypted: &[Part<'_>],
    plain: Part<'_>,
    keys: &StreamKeys,
) -> Result<Vec<u8>, PackError> {
    if encrypted.is_empty() {
        return Err(PackError::Empty);
    }
    let n = encrypted.len() + 1;
    let ih_end = IH0 + n * 0x40;
    let term_off = PH_OFFSET + n * PARTITION_HEADER_LEN;
    if n > MAX_PARTITIONS || ih_end > PH_OFFSET || term_off + PARTITION_HEADER_LEN > STAGE1_OFF {
        return Err(PackError::Unaligned);
    }
    for p in encrypted {
        if p.data.is_empty() {
            return Err(PackError::Empty);
        }
        validate_bootloader_artifact(p.name, &sha256(p.data))?;
    }
    if plain.data.is_empty() {
        return Err(PackError::Empty);
    }
    validate_bootloader_artifact(plain.name, &sha256(plain.data))?;

    let mut plains = Vec::with_capacity(n);
    let mut stored = Vec::with_capacity(n);
    let mut offs = Vec::with_capacity(n);
    let mut off = STAGE1_OFF;
    for p in encrypted {
        let pt = pad4(p.data);
        let blob = encrypt_partition(&pt, (p.attributes & ATTR_PL) != 0, keys)?;
        offs.push(off);
        off = align_up(off + blob.len(), PART_ALIGN);
        plains.push(pt);
        stored.push(blob);
    }
    let app = pad4(plain.data);
    offs.push(off);
    plains.push(app.clone());
    stored.push(app);

    if stored[0].len() == plains[0].len() || stored[n - 1].len() != plains[n - 1].len() {
        return Err(PackError::Unaligned);
    }

    let total = offs[n - 1] + stored[n - 1].len();
    let mut buf = vec![0u8; total];
    for i in 0..8 {
        put_u32(&mut buf, i * 4, VEC_WORD);
    }
    put_u32(&mut buf, 0x20, WIDTH_DETECT);
    put_u32(&mut buf, 0x24, IMAGE_IDENT_XLNX);
    put_u32(&mut buf, 0x28, ENC_STATUS_BBRAM);
    put_u32(&mut buf, 0x2C, ATTR_WORD);
    put_u32(&mut buf, OFF_SOURCE_OFFSET, STAGE1_OFF as u32);
    put_u32(&mut buf, OFF_IMAGE_LEN, plains[0].len() as u32);
    put_u32(&mut buf, 0x38, 0);
    put_u32(&mut buf, 0x3C, 0);
    put_u32(&mut buf, 0x40, stored[0].len() as u32);
    put_u32(&mut buf, 0x44, QSPI_WORD);
    put_u32(&mut buf, 0x98, IHT_OFFSET as u32);
    put_u32(&mut buf, 0x9C, PH_OFFSET as u32);
    let mut roff = OFF_REG_INIT;
    while roff + 8 <= 0x8A0 {
        put_u32(&mut buf, roff, 0xFFFF_FFFF);
        put_u32(&mut buf, roff + 4, 0);
        roff += 8;
    }
    while roff < IHT_OFFSET {
        put_u32(&mut buf, roff, 0xFFFF_FFFF);
        roff += 4;
    }
    put_u32(&mut buf, IHT_OFFSET, 0x0102_0000);
    put_u32(&mut buf, IHT_OFFSET + 4, n as u32);
    put_u32(&mut buf, IHT_OFFSET + 8, (PH_OFFSET / 4) as u32);
    put_u32(&mut buf, IHT_OFFSET + 12, (IH0 / 4) as u32);
    put_u32(&mut buf, IHT_OFFSET + 16, 0);
    for i in 5..16 {
        put_u32(&mut buf, IHT_OFFSET + i * 4, 0xFFFF_FFFF);
    }
    for i in 0..n {
        let next = if i + 1 == n {
            0
        } else {
            (IH0 as u32 + ((i as u32) + 1) * 0x40) / 4
        };
        let name = if i + 1 == n {
            plain.name
        } else {
            encrypted[i].name
        };
        write_ih(
            &mut buf,
            IH0 + i * 0x40,
            next,
            ((PH_OFFSET + i * PARTITION_HEADER_LEN) / 4) as u32,
            name,
        );
    }
    buf[ih_end..PH_OFFSET].fill(0xFF);
    for (i, p) in encrypted.iter().enumerate() {
        write_ph(
            &mut buf,
            PH_OFFSET + i * PARTITION_HEADER_LEN,
            p,
            offs[i],
            (stored[i].len() / 4) as u32,
            (plains[i].len() / 4) as u32,
            (stored[i].len() / 4) as u32,
            ((IH0 + i * 0x40) / 4) as u32,
        );
    }
    write_ph(
        &mut buf,
        PH_OFFSET + encrypted.len() * PARTITION_HEADER_LEN,
        &plain,
        offs[n - 1],
        (stored[n - 1].len() / 4) as u32,
        (plains[n - 1].len() / 4) as u32,
        (stored[n - 1].len() / 4) as u32,
        ((IH0 + (n - 1) * 0x40) / 4) as u32,
    );
    write_null_ph(&mut buf, term_off);
    buf[term_off + PARTITION_HEADER_LEN..STAGE1_OFF].fill(0xFF);
    let csum = boot_header_checksum(&buf);
    put_u32(&mut buf, 0x48, csum);
    for i in 0..n {
        let o = offs[i];
        let s = &stored[i];
        buf[o..o + s.len()].copy_from_slice(s);
        if i + 1 < n {
            buf[o + s.len()..offs[i + 1]].fill(0xFF);
        }
    }
    let parsed = parse_allow_encrypted(&buf).map_err(PackError::Parse)?;
    if parsed.partition_count != n || parsed.source_offset as usize != STAGE1_OFF {
        return Err(PackError::Unaligned);
    }
    if parsed.partitions[0].enc_words == parsed.partitions[0].data_words {
        return Err(PackError::Unaligned);
    }
    if parsed.partitions[n - 1].enc_words != parsed.partitions[n - 1].data_words {
        return Err(PackError::Unaligned);
    }
    check_encrypted_image(&buf).map_err(|_| PackError::Frame)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootimg::parse::{
        loader_bitstream_app, parse_all, parse_all_product, ATTR_PL, ATTR_PS, ParseError,
        PAYLOAD_ALIGN,
    };
    use crate::QUARANTINED_FSBL_SHA256;
    use crate::UTILITY_ENTRY;

    fn tiny_parts() -> (Part<'static>, Part<'static>, Part<'static>) {
        (
            Part {
                name: "stage1.bin",
                data: &[0x11, 0x22, 0x33, 0x44],
                load_addr: 0,
                exec_addr: 0,
                attributes: ATTR_PS,
            },
            Part {
                name: "system.bit",
                data: &[0xAA, 0x99, 0x55, 0x66, 0, 0, 0, 0],
                load_addr: 0,
                exec_addr: 0,
                attributes: ATTR_PL,
            },
            Part {
                name: "kairos_zynq.elf",
                data: &[0xE0, 0xE1, 0xE2, 0xE3],
                load_addr: 0x0010_0000,
                exec_addr: UTILITY_ENTRY,
                attributes: ATTR_PS,
            },
        )
    }

    #[test]
    fn pack_roundtrip_and_corrections() {
        let (a, b, c) = tiny_parts();
        let img = pack(a, b, c).unwrap();
        let h = parse(&img).unwrap();
        assert_eq!(h.source_offset as usize, 0x1700);
        assert_eq!(h.source_offset, h.partitions[0].data_off as u32);
        assert_ne!(h.source_offset, h.stage1_len);
        assert_eq!(u32le(&img, OFF_REG_INIT).unwrap(), 0xFFFF_FFFF);
        assert_eq!(h.reg_init0, 0xFFFF_FFFF);
        assert_eq!(h.partitions[1].data_off % PAYLOAD_ALIGN, 0);
        assert_eq!(h.partitions[2].data_off % PAYLOAD_ALIGN, 0);
        assert!(h.partitions[1].data_off >= h.partitions[0].data_off + h.partitions[0].byte_len());
        let (l, bit, app) = loader_bitstream_app(&h).unwrap();
        assert_eq!(l.load_addr, 0);
        assert!(bit.is_pl());
        assert_eq!(app.exec_addr, UTILITY_ENTRY);
        assert_eq!(&img[l.data_off..l.data_off + 4], &[0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn owned_later_payloads_512_aligned() {
        let (a, b, c) = tiny_parts();
        let h = parse(&pack(a, b, c).unwrap()).unwrap();
        assert_eq!(h.partitions[1].data_off % 512, 0);
        assert_eq!(h.partitions[2].data_off % 512, 0);
    }

    #[test]
    fn register_init_unused_is_ffffffff() {
        let (a, b, c) = tiny_parts();
        let img = pack(a, b, c).unwrap();
        assert_eq!(u32le(&img, 0xA0).unwrap(), 0xFFFF_FFFF);
        assert_eq!(u32le(&img, 0xA4).unwrap(), 0);
    }

    #[test]
    fn pack_refuses_zynq_fsbl() {
        let (mut a, b, c) = tiny_parts();
        a.name = "zynq_fsbl.elf";
        assert_eq!(pack(a, b, c), Err(PackError::FsblFilename));
        a.name = "ZYNQ_FSBL.ELF";
        assert_eq!(pack(a, b, c), Err(PackError::FsblFilename));
    }

    #[test]
    fn pack_refuses_quarantined_hash() {
        let fsbl = std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/zynq_fsbl.elf"),
        )
        .expect("quarantined fsbl");
        assert_eq!(crate::sha256::sha256(&fsbl), QUARANTINED_FSBL_SHA256);
        let (_a, b, c) = tiny_parts();
        let a = Part {
            name: "stage1.bin",
            data: &fsbl,
            load_addr: 0,
            exec_addr: 0,
            attributes: ATTR_PS,
        };
        assert_eq!(pack(a, b, c), Err(PackError::QuarantinedHash));
    }

    #[test]
    fn packed_source_offset_field() {
        let (a, b, c) = tiny_parts();
        let img = pack(a, b, c).unwrap();
        let src = u32le(&img, OFF_SOURCE_OFFSET).unwrap();
        let len = u32le(&img, OFF_IMAGE_LEN).unwrap();
        assert_eq!(src, 0x1700);
        assert_eq!(len, 4);
        assert_ne!(src, len);
    }

    fn wp_le(s: &str) -> [u8; 256] {
        let t = if s.len() == 514 && s.starts_with("00") {
            &s[2..]
        } else {
            s
        };
        let mut be = [0u8; 256];
        for i in 0..256 {
            be[i] = u8::from_str_radix(&t[i * 2..i * 2 + 2], 16).unwrap();
        }
        let mut le = [0u8; 256];
        for i in 0..256 {
            le[i] = be[255 - i];
        }
        le
    }

    #[test]
    fn pack_rsa_later_gold_parse_stays_vendor_rsa() {
        let (a, _, _) = tiny_parts();
        let mut kat = [0u8; 256];
        for i in 0..256 {
            kat[i] = i as u8;
        }
        let n = wp_le("00b3510a2bcd4ce644c5b594ae5059e12b2f054b658d5da5959a2fdf1871b808bc3df3e628d2792e51aad5c124b43bda453dca5cde4bcf28e7bd4effba0cb4b742bbb6d5a013cb63d1aa3a89e02627ef5398b52c0cfd97d208abeb8d7c9bce0bbeb019a86ddb589beb29a5b74bf861075c677c81d430f030c265247af9d3c9140ccb65309d07e0adc1efd15cf17e7b055d7da3868e4648cc3a180f0ee7f8e1e7b18098a3391b4ce7161e98d57af8a947e201a463e2d6bbca8059e5706e9dfed8f4856465ffa712ed1aa18e888d12dc6aa09ce95ecfca83cc5b0b15db09c8647f5d524c0f2e7620a3416b9623cadc0f097af573261c98c8400aa12af38e43cad84d");
        let d = wp_le("1a502d0eea6c7b69e21d5839101f705456ed0ef852fb47fe21071f54c5f33c8ceb066c62d727e32d26c58137329f89d3195325b795264c195d85472f7507dbd0961d2951f935a26b34f0ac24d15490e1128a9b7138915bc7dbfa8fe396357131c543ae9c98507368d9ceb08c1c6198a3eda7aea185a0e976cd42c22d00f003d9f19d96ea4c9afcbfe1441ccc802cfb0689f59d804c6a4e4f404c15174745ed6cb8bc88ef0b33ba0d2a80e35e43bc90f350052e72016e75b00d357a381c9c0d467069ca660887c987766349fcc43460b4aa516bce079edd87ba164307b752c277ed9528ad3ba0bf1877349ed3b7966a6c240110409bf4d0fade0c68fdadd847fd");
        let p1 = Part {
            name: "ddr_kat.bin",
            data: &kat,
            load_addr: 0x0300_0000,
            exec_addr: 0,
            attributes: ATTR_PS,
        };
        let img = pack_rsa_later(a, p1, &n, &d, &n, &d).unwrap();
        assert_eq!(parse(&img), Err(ParseError::VendorRsa));
        let win = core::cmp::min(HEADER_WINDOW_LEN, img.len());
        let h = parse_all_rsa_later(&img[..win], win, img.len()).unwrap();
        assert_eq!(h.partition_count, 2);
        assert_eq!(h.partitions[1].load_addr, 0x0300_0000);
        assert_ne!(h.partitions[1].attributes & ATTR_RSA, 0);
        assert_eq!(h.partitions[0].attributes & ATTR_RSA, 0);
        let ac_off = h.partitions[1].data_off + h.partitions[1].byte_len();
        crate::rsa::cert::verify_later_partition(
            &img[h.partitions[1].data_off..ac_off],
            &img[ac_off..ac_off + crate::rsa::AC_LEN],
        )
        .unwrap();
    }

    #[test]
    fn pack_rsa_later_product_parse_all_product() {
        let (a, b, c) = tiny_parts();
        let mut kat = [0u8; 256];
        for i in 0..256 {
            kat[i] = i as u8;
        }
        let n = wp_le("00b3510a2bcd4ce644c5b594ae5059e12b2f054b658d5da5959a2fdf1871b808bc3df3e628d2792e51aad5c124b43bda453dca5cde4bcf28e7bd4effba0cb4b742bbb6d5a013cb63d1aa3a89e02627ef5398b52c0cfd97d208abeb8d7c9bce0bbeb019a86ddb589beb29a5b74bf861075c677c81d430f030c265247af9d3c9140ccb65309d07e0adc1efd15cf17e7b055d7da3868e4648cc3a180f0ee7f8e1e7b18098a3391b4ce7161e98d57af8a947e201a463e2d6bbca8059e5706e9dfed8f4856465ffa712ed1aa18e888d12dc6aa09ce95ecfca83cc5b0b15db09c8647f5d524c0f2e7620a3416b9623cadc0f097af573261c98c8400aa12af38e43cad84d");
        let d = wp_le("1a502d0eea6c7b69e21d5839101f705456ed0ef852fb47fe21071f54c5f33c8ceb066c62d727e32d26c58137329f89d3195325b795264c195d85472f7507dbd0961d2951f935a26b34f0ac24d15490e1128a9b7138915bc7dbfa8fe396357131c543ae9c98507368d9ceb08c1c6198a3eda7aea185a0e976cd42c22d00f003d9f19d96ea4c9afcbfe1441ccc802cfb0689f59d804c6a4e4f404c15174745ed6cb8bc88ef0b33ba0d2a80e35e43bc90f350052e72016e75b00d357a381c9c0d467069ca660887c987766349fcc43460b4aa516bce079edd87ba164307b752c277ed9528ad3ba0bf1877349ed3b7966a6c240110409bf4d0fade0c68fdadd847fd");
        let p1 = Part {
            name: "ddr_kat.bin",
            data: &kat,
            load_addr: 0x0300_0000,
            exec_addr: 0,
            attributes: ATTR_PS,
        };
        let img = pack_rsa_later_product(a, p1, b, c, &n, &d, &n, &d).unwrap();
        assert_eq!(parse(&img), Err(ParseError::VendorRsa));
        let win = core::cmp::min(HEADER_WINDOW_LEN, img.len());
        assert_eq!(
            parse_all(&img[..win], win, img.len()),
            Err(ParseError::VendorRsa)
        );
        let h = parse_all_product(&img[..win], win, img.len()).unwrap();
        assert_eq!(h.partition_count, 4);
        assert_eq!(h.partitions[1].load_addr, 0x0300_0000);
        assert_ne!(h.partitions[1].attributes & ATTR_RSA, 0);
        assert!(h.partitions[2].is_pl());
        assert_eq!(h.partitions[3].exec_addr, UTILITY_ENTRY);
        let (l, bit, app) = loader_bitstream_app(&h).unwrap();
        assert_eq!(l.load_addr, 0);
        assert!(bit.is_pl());
        assert_eq!(app.exec_addr, UTILITY_ENTRY);
    }

    #[test]
    fn negative_packed_checksum_and_offset() {
        let (a, b, c) = tiny_parts();
        let mut img = pack(a, b, c).unwrap();
        img[PH_OFFSET] ^= 0xFF;
        assert_eq!(parse(&img), Err(ParseError::BadChecksum));
        let mut img = pack(a, b, c).unwrap();
        let too_big = 0x00FF_FFF0u32;
        img[OFF_SOURCE_OFFSET..OFF_SOURCE_OFFSET + 4].copy_from_slice(&too_big.to_le_bytes());
        assert!(parse(&img).is_err());
    }

    fn test_keys() -> StreamKeys {
        let mut aes = [0u8; 32];
        let mut iv = [0u8; 16];
        let mut mac = [0u8; 32];
        for i in 0..32 {
            aes[i] = i as u8;
            mac[i] = (0xA0u8).wrapping_add(i as u8);
        }
        for i in 0..16 {
            iv[i] = (0x50u8).wrapping_add(i as u8);
        }
        StreamKeys { aes, iv, mac }
    }

    #[test]
    fn pack_secure_gold_parse_stays_encrypted() {
        let (a, b, c) = tiny_parts();
        let img = pack_secure(a, b, c, &test_keys()).unwrap();
        assert_eq!(parse(&img), Err(ParseError::Encrypted));
        let h = parse_allow_encrypted(&img).unwrap();
        assert_eq!(h.source_offset, 0x1700);
        assert_ne!(h.partitions[0].enc_words, h.partitions[0].data_words);
        assert_ne!(h.partitions[1].enc_words, h.partitions[1].data_words);
        assert_eq!(h.partitions[2].enc_words, h.partitions[2].data_words);
        crate::bootimg::frame::check_encrypted_image(&img).unwrap();
        let rec = crate::bootimg::stream::decrypt_user(
            &img[h.partitions[0].data_off
                ..h.partitions[0].data_off + (h.partitions[0].enc_words as usize) * 4],
            false,
            &test_keys(),
        )
        .unwrap();
        assert_eq!(&rec[..4], &[0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn pack_secure_refuses_fsbl_name() {
        let (mut a, b, c) = tiny_parts();
        a.name = "zynq_fsbl.elf";
        assert_eq!(pack_secure(a, b, c, &test_keys()), Err(PackError::FsblFilename));
    }

    #[test]
    fn pack_secure_image_four_part_gold_parse_stays_encrypted() {
        let (a, b, c) = tiny_parts();
        let kat = Part {
            name: "ddr_kat.bin",
            data: &[0, 1, 2, 3],
            load_addr: 0x0300_0000,
            exec_addr: 0,
            attributes: ATTR_PS,
        };
        let img = pack_secure_image(&[a, kat, b], c, &test_keys()).unwrap();
        assert_eq!(parse(&img), Err(ParseError::Encrypted));
        let h = parse_allow_encrypted(&img).unwrap();
        assert_eq!(h.partition_count, 4);
        assert_eq!(h.source_offset, 0x1700);
        assert_ne!(h.partitions[0].enc_words, h.partitions[0].data_words);
        assert_ne!(h.partitions[1].enc_words, h.partitions[1].data_words);
        assert_ne!(h.partitions[2].enc_words, h.partitions[2].data_words);
        assert_eq!(h.partitions[3].enc_words, h.partitions[3].data_words);
        assert_eq!(h.partitions[3].exec_addr, UTILITY_ENTRY);
        assert_eq!(h.partitions[1].load_addr, 0x0300_0000);
        crate::bootimg::frame::check_encrypted_image(&img).unwrap();
        let term = PH_OFFSET + 4 * PARTITION_HEADER_LEN;
        assert_eq!(u32le(&img, term).unwrap(), 0);
        assert_eq!(u32le(&img, term + 60).unwrap(), 0xFFFF_FFFF);
        assert_eq!(img[0xA00], 0xFF);
        assert_eq!(img[STAGE1_OFF - 1], 0xFF);
    }

    #[test]
    fn pack_secure_image_refuses_fsbl_name() {
        let (mut a, b, c) = tiny_parts();
        a.name = "zynq_fsbl.elf";
        let kat = Part {
            name: "ddr_kat.bin",
            data: &[0, 1, 2, 3],
            load_addr: 0x0300_0000,
            exec_addr: 0,
            attributes: ATTR_PS,
        };
        assert_eq!(
            pack_secure_image(&[a, kat, b], c, &test_keys()),
            Err(PackError::FsblFilename)
        );
    }

    fn elf32_pt_load(elf: &[u8]) -> Vec<u8> {
        let _ = crate::elf32_le_entry(elf).expect("elf");
        let phoff = u32::from_le_bytes(elf[0x1C..0x20].try_into().unwrap()) as usize;
        let phentsize = u16::from_le_bytes(elf[0x2A..0x2C].try_into().unwrap()) as usize;
        let phnum = u16::from_le_bytes(elf[0x2C..0x2E].try_into().unwrap()) as usize;
        let mut loads = Vec::new();
        let mut i = 0;
        while i < phnum {
            let o = phoff + i * phentsize;
            let p_type = u32::from_le_bytes(elf[o..o + 4].try_into().unwrap());
            if p_type == 1 {
                let p_offset = u32::from_le_bytes(elf[o + 4..o + 8].try_into().unwrap());
                let p_paddr = u32::from_le_bytes(elf[o + 12..o + 16].try_into().unwrap());
                let p_filesz = u32::from_le_bytes(elf[o + 16..o + 20].try_into().unwrap());
                loads.push((p_paddr, p_offset, p_filesz));
            }
            i += 1;
        }
        loads.sort_by_key(|l| l.0);
        let base = loads[0].0;
        let mut end = base;
        for &(paddr, _, filesz) in &loads {
            let seg_end = paddr + filesz;
            if seg_end > end {
                end = seg_end;
            }
        }
        let mut img = vec![0u8; (end - base) as usize];
        for &(paddr, offset, filesz) in &loads {
            let dest = (paddr - base) as usize;
            let off = offset as usize;
            let n = filesz as usize;
            img[dest..dest + n].copy_from_slice(&elf[off..off + n]);
        }
        img
    }

    fn first_diff(a: &[u8], b: &[u8]) -> usize {
        let n = a.len().min(b.len());
        let mut i = 0;
        while i < n {
            if a[i] != b[i] {
                return i;
            }
            i += 1;
        }
        n
    }

    #[test]
    fn oracle_boot_bin_byte_identical_parse_and_recover() {
        let owned = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = owned.join("build/oracle");
        let boot_p = dir.join("BOOT_SYS06_ORACLE.BIN");
        let nky_p = dir.join("sys06_dev.nky");
        let kat_p = dir.join("ddr_kat.bin");
        let elf_p = owned.join("build/sys05_plat.elf");
        let bit_p = owned.join("../../fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit");
        let util_p = owned.join("../../fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf");
        if !boot_p.exists()
            || !nky_p.exists()
            || !kat_p.exists()
            || !elf_p.exists()
            || !bit_p.exists()
            || !util_p.exists()
        {
            return;
        }
        let keys =
            crate::bootimg::stream::keys_from_nky(&std::fs::read_to_string(&nky_p).unwrap())
                .expect("nky");
        let stage1 = elf32_pt_load(&std::fs::read(&elf_p).unwrap());
        assert_eq!(stage1.len(), 13380);
        let kat = std::fs::read(&kat_p).unwrap();
        assert_eq!(kat.len(), 256);
        let pl = crate::bootimg::bit::strip_encrypted_pl(&std::fs::read(&bit_p).unwrap()).unwrap();
        assert_eq!(pl.len(), 4_043_904);
        let util_elf = std::fs::read(&util_p).unwrap();
        assert_eq!(crate::elf32_le_entry(&util_elf), Some(UTILITY_ENTRY));
        let utility = elf32_pt_load(&util_elf);
        assert_eq!(utility.len(), 1_934_548);
        let img = pack_secure_image(
            &[
                Part {
                    name: "sys05_plat.elf",
                    data: &stage1,
                    load_addr: 0,
                    exec_addr: 0,
                    attributes: ATTR_PS,
                },
                Part {
                    name: "ddr_kat.bin",
                    data: &kat,
                    load_addr: 0x0300_0000,
                    exec_addr: 0,
                    attributes: ATTR_PS,
                },
                Part {
                    name: "system.bit",
                    data: &pl,
                    load_addr: 0,
                    exec_addr: 0,
                    attributes: ATTR_PL,
                },
            ],
            Part {
                name: "kairos_zynq.elf",
                data: &utility,
                load_addr: 0x0010_0000,
                exec_addr: UTILITY_ENTRY,
                attributes: ATTR_PS,
            },
            &keys,
        )
        .unwrap();
        let oracle = std::fs::read(&boot_p).unwrap();
        if img != oracle {
            panic!(
                "owned BOOT.BIN first_diff 0x{:x} owned_len {} oracle_len {}",
                first_diff(&img, &oracle),
                img.len(),
                oracle.len()
            );
        }
        assert_eq!(parse(&img), Err(ParseError::Encrypted));
        assert_eq!(u32le(&img, OFF_IMAGE_LEN).unwrap(), 13380);
        assert_eq!(u32le(&img, 0x40).unwrap(), 14188);
        let h = parse_allow_encrypted(&img).unwrap();
        assert_eq!(h.partition_count, 4);
        assert_eq!(h.source_offset, 0x1700);
        assert_eq!(h.partitions[0].data_off, 0x1700);
        assert_eq!(h.partitions[1].data_off, 0x4E80);
        assert_eq!(h.partitions[2].data_off, 0x52C0);
        assert_eq!(h.partitions[3].data_off, 0x3E0A40);
        assert_eq!(h.partitions[3].load_addr, 0x0010_0000);
        assert_eq!(h.partitions[3].exec_addr, UTILITY_ENTRY);
        assert_eq!(h.partitions[3].enc_words, h.partitions[3].data_words);
        crate::bootimg::frame::check_encrypted_image(&img).unwrap();
        let rec0 = crate::bootimg::stream::open_encrypted_partition(
            &img[h.partitions[0].data_off
                ..h.partitions[0].data_off + (h.partitions[0].enc_words as usize) * 4],
            false,
            &keys,
            13380,
        )
        .unwrap();
        assert_eq!(rec0, stage1);
        let rec1 = crate::bootimg::stream::open_encrypted_partition(
            &img[h.partitions[1].data_off
                ..h.partitions[1].data_off + (h.partitions[1].enc_words as usize) * 4],
            false,
            &keys,
            256,
        )
        .unwrap();
        assert_eq!(rec1, kat);
        let rec2 = crate::bootimg::stream::open_encrypted_partition(
            &img[h.partitions[2].data_off
                ..h.partitions[2].data_off + (h.partitions[2].enc_words as usize) * 4],
            true,
            &keys,
            4_043_904,
        )
        .unwrap();
        assert_eq!(rec2, pl);
        let p3n = (h.partitions[3].data_words as usize) * 4;
        assert_eq!(
            &img[h.partitions[3].data_off..h.partitions[3].data_off + p3n],
            utility.as_slice()
        );
        let mut tamper = img.clone();
        tamper[PH_OFFSET] ^= 1;
        assert_eq!(parse_allow_encrypted(&tamper), Err(ParseError::BadChecksum));
        let mut cut = img.clone();
        cut[h.partitions[0].data_off + crate::bootimg::frame::STREAM_HEADER_LEN] ^= 1;
        assert_eq!(
            crate::bootimg::stream::open_encrypted_partition(
                &cut[h.partitions[0].data_off
                    ..h.partitions[0].data_off + (h.partitions[0].enc_words as usize) * 4],
                false,
                &keys,
                13380,
            ),
            Err(crate::bootimg::stream::StreamError::Auth)
        );
    }
}
