// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Read-only Bootgen encrypted-partition framing (System 6A).
//!
//! For System 6A, use an external host-only crypto verifier such as OpenSSL
//! or Python Cryptography to decrypt and verify the fixtures. Do not add
//! owned AES/HMAC implementation code yet.
//!
//! Manufacturer consumer (installed Vitis 2026.1 zynq_fsbl + DevCfg, UG585):
//! PCAP DMA source words = encrypted length, dest words = unencrypted length.
//! AES/HMAC run in the PL. This crate does not copy vendor source and does
//! not implement AES, HMAC, or PCAP. This is not a packer.

use super::parse::{
    loader_bitstream_app, parse_partition, u32le, ParseError, ParsedHeaders, PartitionHeader,
    APP_LOAD_MAX, APP_LOAD_MIN, ATTR_PL, ATTR_RSA, HEADER_WINDOW_LEN, IHT_OFFSET, IMAGE_IDENT_XLNX,
    MAX_PARTITIONS, OFF_IMAGE_LEN, OFF_SOURCE_OFFSET, PARTITION_HEADER_LEN, PH_OFFSET, WIDTH_DETECT,
};

/// Boot header encryption status: BBRAM key (UG585).
pub const ENC_STATUS_BBRAM: u32 = 0x3A5C_3C5A;
/// Measured plaintext stream-header length.
pub const STREAM_HEADER_LEN: usize = 204;
/// Measured plaintext NOOP trailer (`16 * 0x20000000`).
pub const NOOP_TRAILER_LEN: usize = 64;
pub const NOOP_WORD: u32 = 0x2000_0000;
/// IV in the stream header (16 bytes).
pub const STREAM_IV_OFF: usize = 116;
/// Encrypted word-length field (AES stream + trailer).
pub const STREAM_LEN_OFF: usize = 136;
/// Measured remainder after header, 64-byte-padded user stream, and trailer.
pub const STREAM_OVERHEAD: usize = 480;
/// Measured PS prefix inside the padded user stream (not present for PL).
pub const PS_PREFIX: usize = 8;
pub const STAGE1_FILE_OFF: usize = 0x1700;
pub const PART_ALIGN: usize = 32;

/// Inner AES-CBC / HMAC byte ranges are not independently recovered.
pub const SYS06A_STATUS: &str = "SYS06A_FRAMING_UNRESOLVED";

const W_AA99: u32 = 0xAA99_5566;
const W_MASK: u32 = 0x3000_C001;
const W_CTL: u32 = 0x3000_A001;
const W_IVCMD: u32 = 0x3001_6004;
const W_LENCMD: u32 = 0x3003_4001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    Truncated,
    NotEncrypted,
    BadStreamHeader,
    BadTrailer,
    BadLength,
    BadOffset,
    BadEncStatus,
    Unaligned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamFrame {
    pub file_off: usize,
    pub total_len: usize,
    pub header_len: usize,
    pub cipher_off: usize,
    pub cipher_len: usize,
    pub trailer_off: usize,
}

pub fn align_up(n: usize, a: usize) -> usize {
    if a == 0 {
        return n;
    }
    n.saturating_add(a - 1) / a * a
}

/// UG585/Bootgen unencrypted length is payload padded to 32-bit words.
pub fn unenc_bytes(payload_len: usize) -> usize {
    align_up(payload_len, 4)
}

/// 64-byte-padded user stream inside the AES plaintext (PS prefix + payload).
pub fn user_stream_bytes(unenc: usize, is_pl: bool) -> usize {
    if is_pl {
        align_up(unenc, 64)
    } else {
        align_up(unenc.saturating_add(PS_PREFIX), 64)
    }
}

/// File offset of the 480-byte HMAC remainder in an encrypted partition blob.
pub fn hmac_remainder_off(unenc: usize, is_pl: bool) -> usize {
    STREAM_HEADER_LEN.saturating_add(user_stream_bytes(unenc, is_pl))
}

/// Predicted on-flash encrypted byte length from partition unenc length.
pub fn predict_enc_bytes(unenc: usize, is_pl: bool) -> usize {
    STREAM_HEADER_LEN
        .saturating_add(user_stream_bytes(unenc, is_pl))
        .saturating_add(STREAM_OVERHEAD)
        .saturating_add(NOOP_TRAILER_LEN)
}

#[cfg(feature = "std")]
pub fn word_swap_32(buf: &[u8]) -> Result<Vec<u8>, FrameError> {
    if buf.len() % 4 != 0 {
        return Err(FrameError::Unaligned);
    }
    let mut o = buf.to_vec();
    for c in o.chunks_exact_mut(4) {
        c.reverse();
    }
    Ok(o)
}

fn check_stream_header(h: &[u8]) -> Result<(), FrameError> {
    if h.len() < STREAM_HEADER_LEN {
        return Err(FrameError::Truncated);
    }
    if u32le(h, 24).map_err(|_| FrameError::Truncated)? != W_AA99 {
        return Err(FrameError::BadStreamHeader);
    }
    if u32le(h, 32).map_err(|_| FrameError::Truncated)? != W_MASK {
        return Err(FrameError::BadStreamHeader);
    }
    if u32le(h, 40).map_err(|_| FrameError::Truncated)? != W_CTL {
        return Err(FrameError::BadStreamHeader);
    }
    if u32le(h, 112).map_err(|_| FrameError::Truncated)? != W_IVCMD {
        return Err(FrameError::BadStreamHeader);
    }
    if u32le(h, 132).map_err(|_| FrameError::Truncated)? != W_LENCMD {
        return Err(FrameError::BadStreamHeader);
    }
    Ok(())
}

fn check_trailer(t: &[u8]) -> Result<(), FrameError> {
    if t.len() != NOOP_TRAILER_LEN {
        return Err(FrameError::BadTrailer);
    }
    let mut i = 0;
    while i < t.len() {
        if u32le(t, i).map_err(|_| FrameError::Truncated)? != NOOP_WORD {
            return Err(FrameError::BadTrailer);
        }
        i += 4;
    }
    Ok(())
}

/// Split one encrypted partition blob into header / cipher / trailer.
pub fn split_encrypted_blob(blob: &[u8]) -> Result<StreamFrame, FrameError> {
    if blob.len() < STREAM_HEADER_LEN + NOOP_TRAILER_LEN + 16 {
        return Err(FrameError::Truncated);
    }
    if blob.len() % 16 != 12 {
        return Err(FrameError::Unaligned);
    }
    check_stream_header(&blob[..STREAM_HEADER_LEN])?;
    let trailer_off = blob.len() - NOOP_TRAILER_LEN;
    check_trailer(&blob[trailer_off..])?;
    let cipher_off = STREAM_HEADER_LEN;
    let cipher_len = trailer_off - cipher_off;
    if cipher_len == 0 || cipher_len % 16 != 0 {
        return Err(FrameError::Unaligned);
    }
    let len_words = u32le(blob, STREAM_LEN_OFF).map_err(|_| FrameError::Truncated)? as usize;
    if len_words.saturating_mul(4) != cipher_len + NOOP_TRAILER_LEN {
        return Err(FrameError::BadLength);
    }
    Ok(StreamFrame {
        file_off: 0,
        total_len: blob.len(),
        header_len: STREAM_HEADER_LEN,
        cipher_off,
        cipher_len,
        trailer_off,
    })
}

pub fn partition_blob<'a>(boot: &'a [u8], p: &PartitionHeader) -> Result<&'a [u8], FrameError> {
    let n = (p.total_words as usize).saturating_mul(4);
    boot.get(p.data_off..p.data_off.saturating_add(n))
        .ok_or(FrameError::Truncated)
}

/// Parse boot + partition headers without rejecting `enc != unenc`.
pub fn parse_allow_encrypted(boot: &[u8]) -> Result<ParsedHeaders, ParseError> {
    let ident = u32le(boot, 0x24)?;
    if ident != IMAGE_IDENT_XLNX {
        return Err(ParseError::BadIdent);
    }
    let width = u32le(boot, 0x20)?;
    if width != WIDTH_DETECT {
        return Err(ParseError::BadIdent);
    }
    let source_offset = u32le(boot, OFF_SOURCE_OFFSET)?;
    let stage1_len = u32le(boot, OFF_IMAGE_LEN)?;
    let tot = u32le(boot, 0x40)?;
    let iht_ptr = u32le(boot, 0x98)? as usize;
    if iht_ptr != IHT_OFFSET {
        return Err(ParseError::BadIht);
    }
    let version = u32le(boot, IHT_OFFSET)?;
    let count = u32le(boot, IHT_OFFSET + 4)?;
    let ph_words = u32le(boot, IHT_OFFSET + 8)?;
    let ih_words = u32le(boot, IHT_OFFSET + 12)?;
    if count == 0 || count > MAX_PARTITIONS as u32 || (version >> 16) != 0x0102 {
        return Err(ParseError::BadIht);
    }
    let ph_off = (ph_words as usize).saturating_mul(4);
    if u32le(boot, 0x9C)? as usize != ph_off || ph_off != PH_OFFSET {
        return Err(ParseError::BadIht);
    }
    let n = count as usize;
    let mut partitions = [PartitionHeader {
        enc_words: 0,
        data_words: 0,
        total_words: 0,
        load_addr: 0,
        exec_addr: 0,
        data_off: 0,
        attributes: 0,
    }; MAX_PARTITIONS];
    for i in 0..n {
        partitions[i] = parse_partition(boot, ph_off + i * PARTITION_HEADER_LEN)?;
    }
    if partitions[0].data_off != source_offset as usize {
        return Err(ParseError::SourceOffset);
    }
    if (partitions[0].data_words as usize).saturating_mul(4) != stage1_len as usize {
        return Err(ParseError::SourceOffset);
    }
    if (partitions[0].enc_words as usize).saturating_mul(4) != tot as usize {
        return Err(ParseError::SourceOffset);
    }
    Ok(ParsedHeaders {
        source_offset,
        stage1_len,
        ident,
        reg_init0: u32le(boot, 0xA0)?,
        iht: super::parse::ImageHeaderTable {
            version,
            partition_count: count,
            partition_header_off: ph_off,
            image_header_off: (ih_words as usize).saturating_mul(4),
        },
        partitions,
        partition_count: n,
    })
}

fn flash_bytes(p: &PartitionHeader) -> Result<usize, FrameError> {
    (p.total_words as usize)
        .checked_mul(4)
        .ok_or(FrameError::Truncated)
}

fn map_parse(e: ParseError) -> FrameError {
    match e {
        ParseError::Truncated => FrameError::Truncated,
        ParseError::SourceOffset => FrameError::BadOffset,
        _ => FrameError::BadStreamHeader,
    }
}

/// Parse encrypted-image headers from a 16 KiB window. Payloads are not loaded.
/// Gold `parse()` / `parse_all()` still reject the same bytes as `Encrypted`.
pub fn parse_all_encrypted(
    header_window: &[u8],
    header_window_len: usize,
    full_boot_bin_len: usize,
) -> Result<ParsedHeaders, FrameError> {
    if header_window_len == 0
        || header_window_len > HEADER_WINDOW_LEN
        || header_window_len > header_window.len()
    {
        return Err(FrameError::Truncated);
    }
    let hdr = header_window
        .get(..header_window_len)
        .ok_or(FrameError::Truncated)?;
    if u32le(hdr, 0x28).map_err(|_| FrameError::Truncated)? != ENC_STATUS_BBRAM {
        return Err(FrameError::BadEncStatus);
    }
    let h = parse_allow_encrypted(hdr).map_err(map_parse)?;
    let ph_bytes = h
        .partition_count
        .checked_mul(PARTITION_HEADER_LEN)
        .ok_or(FrameError::Truncated)?;
    let ph_end = h
        .iht
        .partition_header_off
        .checked_add(ph_bytes)
        .ok_or(FrameError::Truncated)?;
    if ph_end > header_window_len || IHT_OFFSET + 16 > header_window_len {
        return Err(FrameError::Truncated);
    }
    for i in 0..h.partition_count {
        let p = h.partitions[i];
        if (p.attributes & ATTR_RSA) != 0 {
            return Err(FrameError::BadStreamHeader);
        }
        if p.enc_words != p.total_words {
            return Err(FrameError::BadLength);
        }
        let end = p
            .data_off
            .checked_add(flash_bytes(&p)?)
            .ok_or(FrameError::Truncated)?;
        if end > full_boot_bin_len {
            return Err(FrameError::Truncated);
        }
        if p.is_ps() && p.load_addr >= APP_LOAD_MIN && p.load_addr > APP_LOAD_MAX {
            return Err(FrameError::BadOffset);
        }
    }
    let mut order: [usize; MAX_PARTITIONS] = [0; MAX_PARTITIONS];
    for i in 0..h.partition_count {
        order[i] = i;
    }
    for a in 0..h.partition_count {
        for b in a + 1..h.partition_count {
            if h.partitions[order[b]].data_off < h.partitions[order[a]].data_off {
                order.swap(a, b);
            }
        }
    }
    for i in 0..h.partition_count.saturating_sub(1) {
        let a = h.partitions[order[i]];
        let b = h.partitions[order[i + 1]];
        let a_end = a
            .data_off
            .checked_add(flash_bytes(&a)?)
            .ok_or(FrameError::Truncated)?;
        if a_end > b.data_off {
            return Err(FrameError::BadOffset);
        }
    }
    let (loader, bit, app) = loader_bitstream_app(&h).map_err(map_parse)?;
    if loader.enc_words == loader.data_words || bit.enc_words == bit.data_words {
        return Err(FrameError::NotEncrypted);
    }
    if app.enc_words != app.data_words {
        return Err(FrameError::NotEncrypted);
    }
    if app.exec_addr != crate::UTILITY_ENTRY {
        return Err(FrameError::BadOffset);
    }
    Ok(h)
}

pub fn frame_encrypted_partition(boot: &[u8], p: &PartitionHeader) -> Result<StreamFrame, FrameError> {
    if p.enc_words == p.data_words {
        return Err(FrameError::NotEncrypted);
    }
    let blob = partition_blob(boot, p)?;
    if blob.len() != (p.enc_words as usize).saturating_mul(4) {
        return Err(FrameError::BadLength);
    }
    let mut f = split_encrypted_blob(blob)?;
    f.file_off = p.data_off;
    let is_pl = (p.attributes & ATTR_PL) != 0;
    let unenc = (p.data_words as usize).saturating_mul(4);
    if predict_enc_bytes(unenc, is_pl) != blob.len() {
        return Err(FrameError::BadLength);
    }
    Ok(f)
}

pub fn check_encrypted_image(boot: &[u8]) -> Result<ParsedHeaders, FrameError> {
    if u32le(boot, 0x28).map_err(|_| FrameError::Truncated)? != ENC_STATUS_BBRAM {
        return Err(FrameError::BadEncStatus);
    }
    let h = parse_allow_encrypted(boot).map_err(|e| match e {
        ParseError::Truncated => FrameError::Truncated,
        ParseError::BadIdent | ParseError::BadIht | ParseError::BadChecksum => {
            FrameError::BadStreamHeader
        }
        ParseError::SourceOffset => FrameError::BadOffset,
        _ => FrameError::BadLength,
    })?;
    if h.source_offset as usize != STAGE1_FILE_OFF {
        return Err(FrameError::BadOffset);
    }
    let mut expect_off = STAGE1_FILE_OFF;
    for i in 0..h.partition_count {
        let p = h.partitions[i];
        if p.data_off != expect_off {
            return Err(FrameError::BadOffset);
        }
        let n = (p.total_words as usize).saturating_mul(4);
        if p.enc_words != p.data_words {
            frame_encrypted_partition(boot, &p)?;
        }
        expect_off = align_up(expect_off.saturating_add(n), PART_ALIGN);
    }
    Ok(h)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::sha256::sha256;
    use std::fs;
    use std::path::PathBuf;

    fn owned() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn oracle_dir() -> PathBuf {
        owned().join("build/oracle")
    }

    fn hex32(h: [u8; 32]) -> String {
        h.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn load_nky_iv_key_hmac(text: &str) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let mut key = None;
        let mut iv = None;
        let mut mac = None;
        for line in text.lines() {
            let p: Vec<&str> = line.split_whitespace().collect();
            if p.len() >= 3 && p[0] == "Key" && p[1] == "0" {
                let h: String = p[2].chars().filter(|c| c.is_ascii_hexdigit()).collect();
                key = hex::decode_owned(&h);
            } else if p.len() >= 3 && p[0] == "Key" && p[1] == "StartCBC" {
                let h: String = p[2].chars().filter(|c| c.is_ascii_hexdigit()).collect();
                iv = hex::decode_owned(&h);
            } else if p.len() >= 3 && p[0] == "Key" && p[1] == "HMAC" {
                let h: String = p[2..].join("").chars().filter(|c| c.is_ascii_hexdigit()).collect();
                mac = hex::decode_owned(&h);
            }
        }
        match (key, iv, mac) {
            (Some(k), Some(i), Some(m)) => Some((k, i, m)),
            _ => None,
        }
    }

    mod hex {
        pub fn decode_owned(s: &str) -> Option<Vec<u8>> {
            if s.len() % 2 != 0 {
                return None;
            }
            let mut v = Vec::with_capacity(s.len() / 2);
            let b = s.as_bytes();
            let mut i = 0;
            while i < b.len() {
                let hi = from_hex(b[i])?;
                let lo = from_hex(b[i + 1])?;
                v.push((hi << 4) | lo);
                i += 2;
            }
            Some(v)
        }
        fn from_hex(c: u8) -> Option<u8> {
            match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            }
        }
    }

    #[test]
    fn status_is_unresolved() {
        assert_eq!(SYS06A_STATUS, "SYS06A_FRAMING_UNRESOLVED");
    }

    #[test]
    fn manufacturer_pcap_src_enc_dest_unenc() {
        let b = fs::read(require_oracle()).unwrap();
        let h = parse_allow_encrypted(&b).unwrap();
        let p1 = h.partitions[1];
        assert_ne!(p1.enc_words, p1.data_words);
        assert_eq!((p1.enc_words as usize).saturating_mul(4), 1068);
        assert_eq!((p1.data_words as usize).saturating_mul(4), 256);
        let p3 = h.partitions[3];
        assert_eq!(p3.enc_words, p3.data_words);
    }

    #[test]
    fn enc_mod16_is_12_because_header_is_204() {
        assert_eq!(STREAM_HEADER_LEN % 16, 12);
        assert_eq!(NOOP_TRAILER_LEN % 16, 0);
        assert_eq!(STREAM_OVERHEAD % 16, 0);
    }

    #[test]
    fn length_formula_matches_alignment_table() {
        let cases = [
            (15, false, 16, 812),
            (16, false, 16, 812),
            (17, false, 20, 812),
            (31, false, 32, 812),
            (32, false, 32, 812),
            (33, false, 36, 812),
            (255, false, 256, 1068),
            (256, false, 256, 1068),
            (257, false, 260, 1068),
            (13380, false, 13380, 14188),
            (4043904, true, 4043904, 4044652),
        ];
        for (raw, pl, un, enc) in cases {
            assert_eq!(unenc_bytes(raw), un, "unenc {raw}");
            assert_eq!(predict_enc_bytes(un, pl), enc, "enc {raw}");
            assert_eq!(enc % 16, 12);
        }
    }

    fn require_oracle() -> PathBuf {
        let p = oracle_dir().join("BOOT_SYS06_ORACLE.BIN");
        assert!(p.exists(), "locked oracle missing");
        p
    }

    #[test]
    fn locked_oracle_hash_and_framing() {
        let b = fs::read(require_oracle()).unwrap();
        assert_eq!(b.len(), 6_000_404);
        assert_eq!(
            hex32(sha256(&b)),
            "684d97f4a07826f6ec9a94b2d0fc2dfd710514a1ad96a0a1dac4fcce855e21de"
        );
        let h = check_encrypted_image(&b).expect("frame");
        assert_eq!(u32le(&b, 0x28).unwrap(), ENC_STATUS_BBRAM);
        assert_eq!(h.stage1_len, 13380);
        assert_eq!(u32le(&b, 0x40).unwrap(), 14188);
        assert_eq!(h.partition_count, 4);
        assert_eq!(h.partitions[3].exec_addr, crate::UTILITY_ENTRY);
        let p1 = frame_encrypted_partition(&b, &h.partitions[1]).unwrap();
        assert_eq!(p1.header_len, 204);
        assert_eq!(p1.cipher_off, 204);
        assert_eq!(p1.cipher_len, 800);
        assert_eq!(p1.trailer_off, 1004);
        assert_eq!(h.partitions[1].load_addr, 0x0300_0000);
        let p2 = frame_encrypted_partition(&b, &h.partitions[2]).unwrap();
        assert_eq!(p2.total_len, 4_044_652);
        assert_eq!(h.partitions[3].enc_words, h.partitions[3].data_words);
        let p0 = frame_encrypted_partition(&b, &h.partitions[0]).unwrap();
        let b0 = partition_blob(&b, &h.partitions[0]).unwrap();
        let b1 = partition_blob(&b, &h.partitions[1]).unwrap();
        let b2 = partition_blob(&b, &h.partitions[2]).unwrap();
        assert_eq!(&b0[STREAM_IV_OFF..STREAM_IV_OFF + 16], &b1[STREAM_IV_OFF..STREAM_IV_OFF + 16]);
        assert_eq!(&b0[STREAM_IV_OFF..STREAM_IV_OFF + 16], &b2[STREAM_IV_OFF..STREAM_IV_OFF + 16]);
        assert_eq!(&b0[140..204], &b1[140..204]);
        assert_eq!(&b0[140..204], &b2[140..204]);
        assert_eq!(hmac_remainder_off(13380, false), p0.header_len + user_stream_bytes(13380, false));
        assert_eq!(hmac_remainder_off(256, false), 524);
        assert_eq!(user_stream_bytes(256, false), 320);
        assert_eq!(p1.cipher_len, 320 + STREAM_OVERHEAD);
        assert_eq!(hmac_remainder_off(4043904, true), 204 + 4043904);
        let _ = p0;
    }

    #[test]
    fn hmac_remainder_is_480_after_user_stream() {
        assert_eq!(STREAM_OVERHEAD, 480);
        assert_eq!(hmac_remainder_off(16, false), 268);
        assert_eq!(hmac_remainder_off(32, false), 268);
        assert_eq!(hmac_remainder_off(256, false), 524);
        assert_eq!(predict_enc_bytes(256, false) - hmac_remainder_off(256, false), 544);
        assert_eq!(544, STREAM_OVERHEAD + NOOP_TRAILER_LEN);
    }

    #[test]
    fn kat_fixtures_predict_length_and_offset() {
        let dir = oracle_dir().join("sys06a");
        if !dir.join("BOOT_KAT_256.BIN").exists() {
            return;
        }
        for n in [15u32, 16, 17, 31, 32, 33, 255, 256, 257] {
            let b = fs::read(dir.join(format!("BOOT_KAT_{n}.BIN"))).unwrap();
            let h = check_encrypted_image(&b).expect("mini frame");
            assert_eq!(h.partition_count, 2);
            assert_eq!(h.partitions[0].data_off, STAGE1_FILE_OFF);
            assert_eq!(h.partitions[1].data_off, 0x4E80);
            let un = (h.partitions[1].data_words as usize) * 4;
            assert_eq!(un, unenc_bytes(n as usize));
            let enc = (h.partitions[1].enc_words as usize) * 4;
            assert_eq!(enc, predict_enc_bytes(un, false));
            frame_encrypted_partition(&b, &h.partitions[1]).unwrap();
        }
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

    fn first_cipher_diff_same_unenc(payload_first_diff: usize) -> usize {
        STREAM_HEADER_LEN + 16 * ((PS_PREFIX + payload_first_diff) / 16)
    }

    #[test]
    fn same_unenc_first_cipher_diff_is_aes_block_of_prefix_plus_payload() {
        assert_eq!(first_cipher_diff_same_unenc(15), 220);
        assert_eq!(first_cipher_diff_same_unenc(31), 236);
        assert_eq!(first_cipher_diff_same_unenc(255), 460);
        assert_eq!(first_cipher_diff_same_unenc(0), 204);
        let dir = oracle_dir().join("sys06a");
        let cases: [(u32, u32, usize); 4] = [
            (15, 16, 15),
            (31, 32, 31),
            (255, 256, 255),
            (256, 256, 0),
        ];
        for (left, right, pdiff) in cases {
            let pa = if left == 256 && right == 256 {
                dir.join("BOOT_KAT_256.BIN")
            } else {
                dir.join(format!("BOOT_KAT_{left}.BIN"))
            };
            let pb = if left == 256 && right == 256 {
                dir.join("BOOT_KAT_256_A5.BIN")
            } else {
                dir.join(format!("BOOT_KAT_{right}.BIN"))
            };
            if !pa.exists() || !pb.exists() {
                continue;
            }
            let ba = fs::read(&pa).unwrap();
            let bb = fs::read(&pb).unwrap();
            let ha = parse_allow_encrypted(&ba).unwrap();
            let hb = parse_allow_encrypted(&bb).unwrap();
            let a = partition_blob(&ba, &ha.partitions[1]).unwrap();
            let b = partition_blob(&bb, &hb.partitions[1]).unwrap();
            assert_eq!(first_diff(a, b), first_cipher_diff_same_unenc(pdiff));
        }
        let p16 = dir.join("BOOT_KAT_16.BIN");
        let p17 = dir.join("BOOT_KAT_17.BIN");
        if p16.exists() && p17.exists() {
            let ba = fs::read(p16).unwrap();
            let bb = fs::read(p17).unwrap();
            let ha = parse_allow_encrypted(&ba).unwrap();
            let hb = parse_allow_encrypted(&bb).unwrap();
            let a = partition_blob(&ba, &ha.partitions[1]).unwrap();
            let b = partition_blob(&bb, &hb.partitions[1]).unwrap();
            assert_eq!(first_diff(a, b), STREAM_HEADER_LEN);
        }
    }

    #[test]
    fn same_length_kat_shares_header_iv_and_differs_in_cipher() {
        let dir = oracle_dir().join("sys06a");
        let a = dir.join("BOOT_KAT_256.BIN");
        let c = dir.join("BOOT_KAT_256_A5.BIN");
        if !a.exists() || !c.exists() {
            return;
        }
        let ba = fs::read(a).unwrap();
        let bc = fs::read(c).unwrap();
        let ha = parse_allow_encrypted(&ba).unwrap();
        let hc = parse_allow_encrypted(&bc).unwrap();
        let pa = partition_blob(&ba, &ha.partitions[1]).unwrap();
        let pc = partition_blob(&bc, &hc.partitions[1]).unwrap();
        assert_eq!(&pa[..204], &pc[..204]);
        assert_ne!(&pa[204..1004], &pc[204..1004]);
        assert_eq!(&pa[1004..], &pc[1004..]);
    }

    #[test]
    fn iv_field_is_word_swapped_nky_startcbc() {
        let nky = oracle_dir().join("sys06_dev.nky");
        if !nky.exists() {
            return;
        }
        let text = fs::read_to_string(&nky).unwrap();
        let (_k, iv, _m) = load_nky_iv_key_hmac(&text).expect("nky fields");
        let boot = fs::read(require_oracle()).unwrap();
        let h = parse_allow_encrypted(&boot).unwrap();
        let blob = partition_blob(&boot, &h.partitions[1]).unwrap();
        let swapped = word_swap_32(&iv).unwrap();
        assert!(
            &blob[STREAM_IV_OFF..STREAM_IV_OFF + 16] == swapped.as_slice(),
            "IV field endianness"
        );
        let _ = (_k, _m);
    }

    #[test]
    fn rejects_header_length_offset_padding_and_status() {
        let mut b = fs::read(require_oracle()).unwrap();
        b[0x28] = 0;
        assert_eq!(check_encrypted_image(&b), Err(FrameError::BadEncStatus));
        let mut b = fs::read(require_oracle()).unwrap();
        let h = parse_allow_encrypted(&b).unwrap();
        let off = h.partitions[1].data_off;
        b[off + 24] ^= 1;
        assert_eq!(check_encrypted_image(&b), Err(FrameError::BadStreamHeader));
        let mut b = fs::read(require_oracle()).unwrap();
        let h = parse_allow_encrypted(&b).unwrap();
        let off = h.partitions[1].data_off;
        b[off + STREAM_LEN_OFF] ^= 1;
        assert_eq!(check_encrypted_image(&b), Err(FrameError::BadLength));
        let mut b = fs::read(require_oracle()).unwrap();
        let h = parse_allow_encrypted(&b).unwrap();
        let off = h.partitions[1].data_off;
        let n = (h.partitions[1].enc_words as usize) * 4;
        b[off + n - 1] ^= 1;
        assert_eq!(check_encrypted_image(&b), Err(FrameError::BadTrailer));
        let mut b = fs::read(require_oracle()).unwrap();
        b[0x31] ^= 1;
        assert_eq!(check_encrypted_image(&b), Err(FrameError::BadOffset));
    }

    #[test]
    fn ciphertext_byte_flip_does_not_claim_auth() {
        let mut b = fs::read(require_oracle()).unwrap();
        let h = parse_allow_encrypted(&b).unwrap();
        let off = h.partitions[1].data_off + STREAM_HEADER_LEN;
        b[off] ^= 1;
        check_encrypted_image(&b).expect("framing still parses; auth is host_verify");
        assert_eq!(SYS06A_STATUS, "SYS06A_FRAMING_UNRESOLVED");
    }

    #[test]
    fn host_cryptography_verifier_kat_hmac_unresolved() {
        let script = owned().join("host_verify/sys06a_cryptography.py");
        assert!(script.is_file());
        if !oracle_dir().join("sys06_dev.nky").exists() {
            return;
        }
        let out = match std::process::Command::new("python").arg(&script).output() {
            Ok(o) => o,
            Err(_) => return,
        };
        let stdout = String::from_utf8_lossy(&out.stdout);
        let low = stdout.to_ascii_lowercase();
        assert!(!low.contains("key 0"), "verifier must not print nky key");
        assert!(!low.contains("startcbc"), "verifier must not print nky iv");
        if stdout.contains("KAT_RECOVER yes") {
            panic!("host verifier recovered KAT; update SYS06A_STATUS");
        }
        assert!(stdout.contains("KAT_RECOVER no"));
        assert!(stdout.contains("HMAC_VERIFY skipped"));
        assert!(stdout.contains("SYS06A_FRAMING_UNRESOLVED"));
        assert_eq!(out.status.code(), Some(2));
    }

    #[test]
    fn parse_all_encrypted_oracle_16k_window() {
        let p = oracle_dir().join("BOOT_SYS06_ORACLE.BIN");
        if !p.exists() {
            return;
        }
        let b = fs::read(&p).unwrap();
        let h = parse_all_encrypted(&b[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, b.len()).unwrap();
        assert_eq!(h.partition_count, 4);
        assert_eq!(h.source_offset, 0x1700);
        assert_eq!(h.stage1_len, 13380);
        assert_ne!(h.partitions[0].enc_words, h.partitions[0].data_words);
        assert_ne!(h.partitions[1].enc_words, h.partitions[1].data_words);
        assert_ne!(h.partitions[2].enc_words, h.partitions[2].data_words);
        assert_eq!(h.partitions[3].enc_words, h.partitions[3].data_words);
        assert_eq!(h.partitions[3].exec_addr, crate::UTILITY_ENTRY);
        assert_eq!(h.partitions[3].load_addr, 0x0010_0000);
        assert_eq!(h.partitions[0].data_off, 0x1700);
        assert_eq!(h.partitions[1].data_off, 0x4E80);
        assert_eq!(h.partitions[2].data_off, 0x52C0);
        assert_eq!(h.partitions[3].data_off, 0x3E0A40);
        assert_eq!(
            crate::bootimg::parse::parse_all(&b[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, b.len()),
            Err(crate::bootimg::parse::ParseError::Encrypted)
        );
    }

    #[test]
    fn parse_all_encrypted_rejects_plaintext_gold() {
        let gold = owned().join("../../fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN");
        let b = fs::read(&gold).unwrap();
        assert_eq!(
            parse_all_encrypted(&b[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, b.len()),
            Err(FrameError::BadEncStatus)
        );
        crate::bootimg::parse::parse_all(&b[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, b.len())
            .unwrap();
    }

    #[test]
    fn parse_all_encrypted_rejects_status_and_utility() {
        let p = oracle_dir().join("BOOT_SYS06_ORACLE.BIN");
        if !p.exists() {
            return;
        }
        let b = fs::read(&p).unwrap();
        let mut st = b[..HEADER_WINDOW_LEN].to_vec();
        st[0x28] = 0;
        assert_eq!(
            parse_all_encrypted(&st, HEADER_WINDOW_LEN, b.len()),
            Err(FrameError::BadEncStatus)
        );
        let mut u = b[..HEADER_WINDOW_LEN].to_vec();
        let ph3 = PH_OFFSET + 3 * PARTITION_HEADER_LEN;
        let mut words = [0u32; 16];
        for i in 0..16 {
            words[i] = u32le(&u, ph3 + i * 4).unwrap();
        }
        words[4] = 0x0010_0020;
        words[15] = crate::bootimg::parse::partition_header_checksum(&words);
        for i in 0..16 {
            u[ph3 + i * 4..ph3 + i * 4 + 4].copy_from_slice(&words[i].to_le_bytes());
        }
        assert_eq!(
            parse_all_encrypted(&u, HEADER_WINDOW_LEN, b.len()),
            Err(FrameError::BadOffset)
        );
    }
}
