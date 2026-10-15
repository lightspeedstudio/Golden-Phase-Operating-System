// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Independent Zynq-7000 boot-image parser (UG585).
//!
//! Does not call packer internals. Header field `0x030` is Source Offset
//! (byte offset of Stage 1), not payload data. Partition ranges use `file_len`,
//! not the length of a truncated header window.

/// `XLNX` at byte `0x024`.
pub const IMAGE_IDENT_XLNX: u32 = 0x584C_4E58;
/// Width detection at `0x020`.
pub const WIDTH_DETECT: u32 = 0xAA99_5566;
/// Gold / UG585 image header table byte offset.
pub const IHT_OFFSET: usize = 0x8C0;
/// Gold partition-header table byte offset.
pub const PH_OFFSET: usize = 0xC80;
/// Boot header Source Offset field (UG585).
pub const OFF_SOURCE_OFFSET: usize = 0x030;
/// Length of Image (Stage 1 bytes).
pub const OFF_IMAGE_LEN: usize = 0x034;
/// Register-initialization table (unused address = `0xFFFFFFFF`).
pub const OFF_REG_INIT: usize = 0x0A0;
pub const PARTITION_HEADER_LEN: usize = 0x40;
pub const ATTR_PS: u32 = 0x10;
pub const ATTR_PL: u32 = 0x20;
pub const ATTR_RSA: u32 = 0x8000;
pub const APP_LOAD_MIN: u32 = 0x0010_0000;
pub const APP_LOAD_MAX: u32 = 0x1F00_0000;
pub const MAX_PARTITIONS: usize = 14;
/// Owned packer alignment for payloads after Stage 1.
pub const PAYLOAD_ALIGN: usize = 512;
/// ADMA header window. Partition headers must fit in this window.
pub const HEADER_WINDOW_LEN: usize = 16384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    Truncated,
    BadIdent,
    BadIht,
    BadChecksum,
    SourceOffset,
    Encrypted,
    VendorRsa,
    BadAc,
    BadRange,
    Overlap,
    NoBitstream,
    NoApplication,
    UtilityEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageHeaderTable {
    pub version: u32,
    pub partition_count: u32,
    pub partition_header_off: usize,
    pub image_header_off: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartitionHeader {
    pub enc_words: u32,
    pub data_words: u32,
    pub total_words: u32,
    pub load_addr: u32,
    pub exec_addr: u32,
    pub data_off: usize,
    pub attributes: u32,
}

impl PartitionHeader {
    pub fn byte_len(&self) -> usize {
        (self.data_words as usize).saturating_mul(4)
    }

    pub fn is_pl(&self) -> bool {
        (self.attributes & ATTR_PL) != 0
    }

    pub fn is_ps(&self) -> bool {
        (self.attributes & ATTR_PS) != 0
    }

    pub fn stored_len(&self) -> usize {
        if (self.attributes & ATTR_RSA) != 0 {
            (self.total_words as usize).saturating_mul(4)
        } else {
            self.byte_len()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedHeaders {
    pub source_offset: u32,
    pub stage1_len: u32,
    pub ident: u32,
    pub reg_init0: u32,
    pub iht: ImageHeaderTable,
    pub partitions: [PartitionHeader; MAX_PARTITIONS],
    pub partition_count: usize,
}

pub fn u32le(buf: &[u8], off: usize) -> Result<u32, ParseError> {
    let b = buf.get(off..off + 4).ok_or(ParseError::Truncated)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

pub fn partition_header_checksum(words: &[u32; 16]) -> u32 {
    let s: u32 = words[..15].iter().fold(0u32, |a, w| a.wrapping_add(*w));
    !s
}

fn parse_iht_at(header: &[u8], iht_off: usize) -> Result<ImageHeaderTable, ParseError> {
    let version = u32le(header, iht_off)?;
    let count = u32le(header, iht_off + 4)?;
    let ph_words = u32le(header, iht_off + 8)?;
    let ih_words = u32le(header, iht_off + 12)?;
    if count == 0 || count > MAX_PARTITIONS as u32 || (version >> 16) != 0x0102 {
        return Err(ParseError::BadIht);
    }
    Ok(ImageHeaderTable {
        version,
        partition_count: count,
        partition_header_off: (ph_words as usize).saturating_mul(4),
        image_header_off: (ih_words as usize).saturating_mul(4),
    })
}

pub fn parse_partition(header: &[u8], hdr_off: usize) -> Result<PartitionHeader, ParseError> {
    let mut words = [0u32; 16];
    for i in 0..16 {
        words[i] = u32le(header, hdr_off + i * 4)?;
    }
    if words[15] != partition_header_checksum(&words) {
        return Err(ParseError::BadChecksum);
    }
    Ok(PartitionHeader {
        enc_words: words[0],
        data_words: words[1],
        total_words: words[2],
        load_addr: words[3],
        exec_addr: words[4],
        data_off: (words[5] as usize).saturating_mul(4),
        attributes: words[6],
    })
}

/// Parse boot + image + partition headers. Payload bounds use `file_len`.
pub fn parse_headers(header: &[u8], file_len: usize) -> Result<ParsedHeaders, ParseError> {
    parse_headers_mode(header, file_len, false)
}

/// Later-partition RSA allowed. Partition 0 must not have `ATTR_RSA`.
pub fn parse_headers_rsa_later(
    header: &[u8],
    file_len: usize,
) -> Result<ParsedHeaders, ParseError> {
    parse_headers_mode(header, file_len, true)
}

fn parse_headers_mode(
    header: &[u8],
    file_len: usize,
    rsa_later: bool,
) -> Result<ParsedHeaders, ParseError> {
    let ident = u32le(header, 0x24)?;
    if ident != IMAGE_IDENT_XLNX {
        return Err(ParseError::BadIdent);
    }
    let width = u32le(header, 0x20)?;
    if width != WIDTH_DETECT {
        return Err(ParseError::BadIdent);
    }
    let source_offset = u32le(header, OFF_SOURCE_OFFSET)?;
    let stage1_len = u32le(header, OFF_IMAGE_LEN)?;
    let src_end = (source_offset as usize)
        .checked_add(stage1_len as usize)
        .ok_or(ParseError::Truncated)?;
    if src_end > file_len {
        return Err(ParseError::Truncated);
    }
    let iht_ptr = u32le(header, 0x98)? as usize;
    if iht_ptr != IHT_OFFSET {
        return Err(ParseError::BadIht);
    }
    let iht = parse_iht_at(header, IHT_OFFSET)?;
    let ph_ptr = u32le(header, 0x9C)? as usize;
    if ph_ptr != iht.partition_header_off {
        return Err(ParseError::BadIht);
    }
    let mut partitions = [PartitionHeader {
        enc_words: 0,
        data_words: 0,
        total_words: 0,
        load_addr: 0,
        exec_addr: 0,
        data_off: 0,
        attributes: 0,
    }; MAX_PARTITIONS];
    let n = iht.partition_count as usize;
    for i in 0..n {
        let p = parse_partition(header, iht.partition_header_off + i * PARTITION_HEADER_LEN)?;
        if p.enc_words != p.data_words {
            return Err(ParseError::Encrypted);
        }
        if (p.attributes & ATTR_RSA) != 0 {
            if !rsa_later || i == 0 {
                return Err(ParseError::VendorRsa);
            }
            let data = p.byte_len();
            if data == 0 || (data & 0x3F) != 0 {
                return Err(ParseError::BadAc);
            }
            if p.stored_len() != data.saturating_add(crate::rsa::AC_LEN) {
                return Err(ParseError::BadAc);
            }
        }
        let end = p
            .data_off
            .checked_add(p.stored_len())
            .ok_or(ParseError::Truncated)?;
        if end > file_len {
            return Err(ParseError::Truncated);
        }
        partitions[i] = p;
    }
    if n == 0 {
        return Err(ParseError::BadIht);
    }
    if partitions[0].data_off != source_offset as usize {
        return Err(ParseError::SourceOffset);
    }
    if partitions[0].byte_len() != stage1_len as usize {
        return Err(ParseError::SourceOffset);
    }
    let mut order: [usize; MAX_PARTITIONS] = [0; MAX_PARTITIONS];
    for i in 0..n {
        order[i] = i;
    }
    for a in 0..n {
        for b in a + 1..n {
            if partitions[order[b]].data_off < partitions[order[a]].data_off {
                order.swap(a, b);
            }
        }
    }
    for i in 0..n.saturating_sub(1) {
        let a = &partitions[order[i]];
        let b = &partitions[order[i + 1]];
        let a_end = a
            .data_off
            .checked_add(a.stored_len())
            .ok_or(ParseError::Truncated)?;
        if a_end > b.data_off {
            return Err(ParseError::Overlap);
        }
    }
    for i in 0..n {
        let p = &partitions[i];
        if p.is_ps() && p.load_addr >= APP_LOAD_MIN && p.load_addr > APP_LOAD_MAX {
            return Err(ParseError::BadRange);
        }
    }
    Ok(ParsedHeaders {
        source_offset,
        stage1_len,
        ident,
        reg_init0: u32le(header, OFF_REG_INIT)?,
        iht,
        partitions,
        partition_count: n,
    })
}

pub fn parse(boot: &[u8]) -> Result<ParsedHeaders, ParseError> {
    parse_headers(boot, boot.len())
}

/// Parse headers from a bounded window. Payloads are checked against FAT file length, not loaded.
pub fn parse_all(
    header_window: &[u8],
    header_window_len: usize,
    full_boot_bin_len: usize,
) -> Result<ParsedHeaders, ParseError> {
    if header_window_len == 0
        || header_window_len > HEADER_WINDOW_LEN
        || header_window_len > header_window.len()
    {
        return Err(ParseError::Truncated);
    }
    let hdr = header_window
        .get(..header_window_len)
        .ok_or(ParseError::Truncated)?;
    let h = parse_headers(hdr, full_boot_bin_len)?;
    let ph_bytes = h
        .partition_count
        .checked_mul(PARTITION_HEADER_LEN)
        .ok_or(ParseError::Truncated)?;
    let ph_end = h
        .iht
        .partition_header_off
        .checked_add(ph_bytes)
        .ok_or(ParseError::Truncated)?;
    if ph_end > header_window_len || IHT_OFFSET + 16 > header_window_len {
        return Err(ParseError::Truncated);
    }
    let (_, _, app) = loader_bitstream_app(&h)?;
    if app.exec_addr != crate::UTILITY_ENTRY {
        return Err(ParseError::UtilityEntry);
    }
    Ok(h)
}

/// Headers from a 16 KiB window. Later partitions may carry an RSA AC. No bitstream/utility demand.
pub fn parse_all_rsa_later(
    header_window: &[u8],
    header_window_len: usize,
    full_boot_bin_len: usize,
) -> Result<ParsedHeaders, ParseError> {
    if header_window_len == 0
        || header_window_len > HEADER_WINDOW_LEN
        || header_window_len > header_window.len()
    {
        return Err(ParseError::Truncated);
    }
    let hdr = header_window
        .get(..header_window_len)
        .ok_or(ParseError::Truncated)?;
    let h = parse_headers_rsa_later(hdr, full_boot_bin_len)?;
    let ph_bytes = h
        .partition_count
        .checked_mul(PARTITION_HEADER_LEN)
        .ok_or(ParseError::Truncated)?;
    let ph_end = h
        .iht
        .partition_header_off
        .checked_add(ph_bytes)
        .ok_or(ParseError::Truncated)?;
    if ph_end > header_window_len || IHT_OFFSET + 16 > header_window_len {
        return Err(ParseError::Truncated);
    }
    if h.partition_count < 2 {
        return Err(ParseError::BadIht);
    }
    Ok(h)
}

/// Loader + bitstream + utility. Later RSA KAT is allowed. Encrypted images stay rejected.
pub fn parse_all_product(
    header_window: &[u8],
    header_window_len: usize,
    full_boot_bin_len: usize,
) -> Result<ParsedHeaders, ParseError> {
    match parse_all(header_window, header_window_len, full_boot_bin_len) {
        Ok(h) => Ok(h),
        Err(ParseError::VendorRsa) => {
            let h = parse_all_rsa_later(header_window, header_window_len, full_boot_bin_len)?;
            if (h.partitions[0].attributes & ATTR_RSA) != 0 {
                return Err(ParseError::VendorRsa);
            }
            let (_, bit, app) = loader_bitstream_app(&h)?;
            if bit.enc_words != bit.data_words || app.enc_words != app.data_words {
                return Err(ParseError::Encrypted);
            }
            if (bit.attributes & ATTR_RSA) != 0 {
                return Err(ParseError::VendorRsa);
            }
            if app.exec_addr != crate::UTILITY_ENTRY {
                return Err(ParseError::UtilityEntry);
            }
            Ok(h)
        }
        Err(e) => Err(e),
    }
}

pub fn partition_bytes<'a>(boot: &'a [u8], p: &PartitionHeader) -> Result<&'a [u8], ParseError> {
    boot.get(p.data_off..p.data_off + p.byte_len())
        .ok_or(ParseError::Truncated)
}

/// First `AA 99 55 66` suffix. Matches working `zynq_bootimg::strip_bit_header`.
pub fn strip_bit_header(bytes: &[u8]) -> Option<&[u8]> {
    if bytes.len() < 4 {
        return None;
    }
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        if bytes[i] == 0xAA && bytes[i + 1] == 0x99 && bytes[i + 2] == 0x55 && bytes[i + 3] == 0x66 {
            return Some(&bytes[i..]);
        }
        i += 1;
    }
    None
}

pub fn loader_bitstream_app(
    h: &ParsedHeaders,
) -> Result<(PartitionHeader, PartitionHeader, PartitionHeader), ParseError> {
    let mut loader = None;
    let mut bit = None;
    let mut app = None;
    for i in 0..h.partition_count {
        let p = h.partitions[i];
        if p.is_pl() {
            bit = Some(p);
        } else if p.is_ps() && p.load_addr >= APP_LOAD_MIN {
            if p.exec_addr >= APP_LOAD_MIN {
                app = Some(p);
            }
        } else if p.is_ps() && p.load_addr == 0 {
            loader = Some(p);
        }
    }
    match (loader, bit, app) {
        (Some(l), Some(b), Some(a)) => Ok((l, b, a)),
        (_, None, _) => Err(ParseError::NoBitstream),
        _ => Err(ParseError::NoApplication),
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::elf32_le_entry;
    use crate::sha256::sha256;
    use crate::UTILITY_ENTRY;
    use std::fs;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn gold_boot() -> Vec<u8> {
        fs::read(
            repo_root().join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN"),
        )
        .expect("gold BOOT.BIN")
    }

    fn hex32(h: [u8; 32]) -> String {
        h.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn gold_fixture_hash_preserved() {
        let b = gold_boot();
        assert_eq!(b.len(), 6114836);
        assert_eq!(
            hex32(sha256(&b)),
            "7da748547d5016191643ddc4df555a8cf02c1d8adee3235f1dde766166ea0b85"
        );
    }

    #[test]
    fn gold_matches_recorded_headers() {
        let b = gold_boot();
        let h = parse(&b).expect("gold parse");
        assert_eq!(h.ident, IMAGE_IDENT_XLNX);
        assert_eq!(h.source_offset, 0x1700);
        assert_eq!(h.stage1_len, 0x1F714);
        assert_eq!(h.reg_init0, 0xFFFF_FFFF);
        assert_eq!(h.iht.version, 0x0102_0000);
        assert_eq!(h.iht.partition_count, 3);
        assert_eq!(h.iht.partition_header_off, 0xC80);
        assert_eq!(h.partition_count, 3);
        let p0 = h.partitions[0];
        let p1 = h.partitions[1];
        let p2 = h.partitions[2];
        assert_eq!(p0.data_off, 0x1700);
        assert_eq!(p0.byte_len(), 128788);
        assert_eq!(p0.load_addr, 0);
        assert_eq!(p0.exec_addr, 0);
        assert_eq!(p0.attributes, ATTR_PS);
        assert_eq!(p1.data_off, 0x20E40);
        assert_eq!(p1.byte_len(), 4045568);
        assert!(p1.is_pl());
        assert_eq!(p2.data_off, 0x3FC940);
        assert_eq!(p2.byte_len(), 0x1D84D4);
        assert_eq!(p2.load_addr, 0x0010_0000);
        assert_eq!(p2.exec_addr, UTILITY_ENTRY);
        let (l, bit, app) = loader_bitstream_app(&h).unwrap();
        assert_eq!(l.data_off, 0x1700);
        assert!(bit.is_pl());
        assert_eq!(app.exec_addr, 0x0010_0060);
        assert_eq!(p1.data_off % 64, 0);
        assert_ne!(p1.data_off % PAYLOAD_ALIGN, 0);
        assert_ne!(p2.data_off % PAYLOAD_ALIGN, 0);
        let hdr16 = &b[..0x1000];
        let from_window = parse_headers(hdr16, b.len()).expect("16KiB window + file_len");
        assert_eq!(from_window.source_offset, 0x1700);
        assert_eq!(from_window.partitions[2].data_off, 0x3FC940);
        assert!(parse_headers(hdr16, 0x2000).is_err());
    }

    #[test]
    fn gold_later_payloads_not_512() {
        let h = parse(&gold_boot()).unwrap();
        assert_ne!(h.partitions[1].data_off % 512, 0);
        assert_ne!(h.partitions[2].data_off % 512, 0);
    }

    #[test]
    fn path1_g64_bit_sync_is_unaligned() {
        let bit = fs::read(repo_root().join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit"))
            .unwrap();
        assert_eq!(bit.len(), 4_045_769);
        let raw = strip_bit_header(&bit).expect("sync");
        assert_eq!(raw.len(), 4_045_516);
        assert_eq!(&raw[..4], &[0xAA, 0x99, 0x55, 0x66]);
        assert_eq!(bit.len() - raw.len(), 253);
        assert_ne!(253 % 4, 0);
    }

    #[test]
    fn utility_elf_entry_readelf_value() {
        let elf = fs::read(
            repo_root().join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf"),
        )
        .unwrap();
        assert_eq!(elf32_le_entry(&elf), Some(UTILITY_ENTRY));
    }

    #[test]
    fn source_offset_is_not_length() {
        let mut b = gold_boot();
        let len = u32le(&b, OFF_IMAGE_LEN).unwrap();
        b[OFF_SOURCE_OFFSET..OFF_SOURCE_OFFSET + 4].copy_from_slice(&len.to_le_bytes());
        assert_eq!(parse(&b), Err(ParseError::SourceOffset));
    }

    #[test]
    fn negative_bad_ident() {
        let mut b = gold_boot();
        b[0x24] ^= 1;
        assert_eq!(parse(&b), Err(ParseError::BadIdent));
    }

    #[test]
    fn negative_checksum() {
        let mut b = gold_boot();
        b[0xC80] ^= 1;
        assert_eq!(parse(&b), Err(ParseError::BadChecksum));
    }

    #[test]
    fn negative_truncation_file_len() {
        let b = gold_boot();
        assert_eq!(
            parse_headers(&b, 0x1800),
            Err(ParseError::Truncated)
        );
    }

    #[test]
    fn negative_overlap() {
        let mut b = gold_boot();
        let mut words = [0u32; 16];
        for i in 0..16 {
            words[i] = u32le(&b, 0xCC0 + i * 4).unwrap();
        }
        words[5] = (0x1700 / 4) as u32;
        words[15] = partition_header_checksum(&words);
        for i in 0..16 {
            b[0xCC0 + i * 4..0xCC0 + i * 4 + 4].copy_from_slice(&words[i].to_le_bytes());
        }
        assert_eq!(parse(&b), Err(ParseError::Overlap));
    }

    #[test]
    fn negative_bad_range() {
        let mut b = gold_boot();
        let mut words = [0u32; 16];
        for i in 0..16 {
            words[i] = u32le(&b, 0xD00 + i * 4).unwrap();
        }
        words[3] = 0x2000_0000;
        words[15] = partition_header_checksum(&words);
        for i in 0..16 {
            b[0xD00 + i * 4..0xD00 + i * 4 + 4].copy_from_slice(&words[i].to_le_bytes());
        }
        assert_eq!(parse(&b), Err(ParseError::BadRange));
    }

    #[test]
    fn negative_encrypted_and_length() {
        let mut b = gold_boot();
        let mut words = [0u32; 16];
        for i in 0..16 {
            words[i] = u32le(&b, 0xC80 + i * 4).unwrap();
        }
        words[0] = words[1] + 1;
        words[15] = partition_header_checksum(&words);
        for i in 0..16 {
            b[0xC80 + i * 4..0xC80 + i * 4 + 4].copy_from_slice(&words[i].to_le_bytes());
        }
        assert_eq!(parse(&b), Err(ParseError::Encrypted));
        let mut b = gold_boot();
        let mut words = [0u32; 16];
        for i in 0..16 {
            words[i] = u32le(&b, 0xC80 + i * 4).unwrap();
        }
        words[1] = 0x1000_0000;
        words[0] = words[1];
        words[2] = words[1];
        words[15] = partition_header_checksum(&words);
        for i in 0..16 {
            b[0xC80 + i * 4..0xC80 + i * 4 + 4].copy_from_slice(&words[i].to_le_bytes());
        }
        b[OFF_IMAGE_LEN..OFF_IMAGE_LEN + 4]
            .copy_from_slice(&((0x1000_0000u32).wrapping_mul(4)).to_le_bytes());
        assert_eq!(parse(&b), Err(ParseError::Truncated));
    }

    fn sys04_boot() -> Vec<u8> {
        fs::read(
            repo_root().join("zynq_stage1/owned/build/BOOT_SYS04_ADMA.BIN"),
        )
        .expect("BOOT_SYS04_ADMA.BIN")
    }

    fn parse_all_16k(boot: &[u8]) -> ParsedHeaders {
        parse_all(&boot[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, boot.len()).expect("parse_all")
    }

    #[test]
    fn parse_all_gold_matches_working_firmware_table() {
        let b = gold_boot();
        let h = parse_all_16k(&b);
        assert_eq!(h.partition_count, 3);
        assert_eq!(h.source_offset, 0x1700);
        assert_eq!(h.stage1_len, 0x1F714);
        let p0 = h.partitions[0];
        let p1 = h.partitions[1];
        let p2 = h.partitions[2];
        assert_eq!(p0.data_off, 0x1700);
        assert_eq!(p0.byte_len(), 128788);
        assert_eq!(p0.load_addr, 0);
        assert_eq!(p0.exec_addr, 0);
        assert_eq!(p0.attributes, ATTR_PS);
        assert_eq!(p1.data_off, 0x20E40);
        assert_eq!(p1.byte_len(), 4045568);
        assert!(p1.is_pl());
        assert_eq!(p1.load_addr, 0);
        assert_eq!(p1.exec_addr, 0);
        assert_eq!(p2.data_off, 0x3FC940);
        assert_eq!(p2.byte_len(), 0x1D84D4);
        assert_eq!(p2.load_addr, 0x0010_0000);
        assert_eq!(p2.exec_addr, UTILITY_ENTRY);
        let (l, bit, app) = loader_bitstream_app(&h).unwrap();
        assert_eq!(l.data_off, p0.data_off);
        assert_eq!(bit.data_off, p1.data_off);
        assert_eq!(app.exec_addr, 0x0010_0060);
        assert_ne!(p1.data_off % PAYLOAD_ALIGN, 0);
        assert_ne!(p2.data_off % PAYLOAD_ALIGN, 0);
    }

    #[test]
    fn parse_all_sys04_matches_owned_packer_table() {
        let b = sys04_boot();
        assert_eq!(b.len(), 5_997_268);
        let h = parse_all_16k(&b);
        assert_eq!(h.partition_count, 3);
        assert_eq!(h.source_offset, 0x1700);
        assert_eq!(h.stage1_len, 10992);
        let p0 = h.partitions[0];
        let p1 = h.partitions[1];
        let p2 = h.partitions[2];
        assert_eq!(p0.data_off, 0x1700);
        assert_eq!(p0.byte_len(), 10992);
        assert_eq!(p0.load_addr, 0);
        assert_eq!(p0.exec_addr, 0);
        assert_eq!(p1.data_off, 0x4200);
        assert_eq!(p1.byte_len(), 4_045_772);
        assert!(p1.is_pl());
        assert_eq!(p1.data_off % PAYLOAD_ALIGN, 0);
        assert_eq!(p2.data_off, 0x3DFE00);
        assert_eq!(p2.byte_len(), 0x1D84D4);
        assert_eq!(p2.load_addr, 0x0010_0000);
        assert_eq!(p2.exec_addr, UTILITY_ENTRY);
        assert_eq!(p2.data_off % PAYLOAD_ALIGN, 0);
        let gold = parse_all_16k(&gold_boot());
        assert_ne!(p1.data_off, gold.partitions[1].data_off);
        assert_ne!(p1.byte_len(), gold.partitions[1].byte_len());
        assert_eq!(p2.exec_addr, gold.partitions[2].exec_addr);
        assert_eq!(p2.byte_len(), gold.partitions[2].byte_len());
        assert_eq!(p2.load_addr, gold.partitions[2].load_addr);
    }

    #[test]
    fn parse_all_rejects_short_window_and_short_file() {
        let b = gold_boot();
        assert_eq!(
            parse_all(&b[..0xC00], 0xC00, b.len()),
            Err(ParseError::Truncated)
        );
        assert_eq!(
            parse_all(&b[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, 0x1800),
            Err(ParseError::Truncated)
        );
    }

    #[test]
    fn parse_all_rejects_wrong_utility_entry() {
        let mut b = gold_boot();
        let mut words = [0u32; 16];
        for i in 0..16 {
            words[i] = u32le(&b, 0xD00 + i * 4).unwrap();
        }
        words[4] = 0x0010_0020;
        words[15] = partition_header_checksum(&words);
        for i in 0..16 {
            b[0xD00 + i * 4..0xD00 + i * 4 + 4].copy_from_slice(&words[i].to_le_bytes());
        }
        assert_eq!(
            parse_all(&b[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, b.len()),
            Err(ParseError::UtilityEntry)
        );
    }

    #[test]
    fn parse_all_rejects_ident_mutation() {
        let mut b = gold_boot();
        b[0x24] ^= 1;
        assert_eq!(
            parse_all(&b[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, b.len()),
            Err(ParseError::BadIdent)
        );
    }

    #[test]
    fn parse_all_product_matches_parse_all_on_gold() {
        let b = gold_boot();
        let a = parse_all(&b[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, b.len()).unwrap();
        let p = parse_all_product(&b[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, b.len()).unwrap();
        assert_eq!(a.partition_count, 3);
        assert_eq!(p.partition_count, 3);
        assert_eq!(a.partitions[2].exec_addr, p.partitions[2].exec_addr);
        assert_eq!(p.partitions[2].exec_addr, UTILITY_ENTRY);
        assert_eq!(
            parse_all_product(&b[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, b.len()),
            parse_all(&b[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, b.len())
        );
    }

    #[test]
    fn zynq_bootimg_rs_hash_unchanged() {
        let t = fs::read(repo_root().join("src/board/zynq_bootimg.rs")).unwrap();
        assert_eq!(
            hex32(sha256(&t)),
            "c96b4b13ac7b884b266c6f90b55bfd0facd2a28105446af2d1dd207e4e6ce999"
        );
    }

    #[test]
    fn oracle_authenticated_bootloader_is_vendor_rsa() {
        let w = include_bytes!("../../../../docs/boot/SYS07_ORACLE_FIRST_16K.bin");
        assert_eq!(parse_headers(w, 25024), Err(ParseError::VendorRsa));
        assert_eq!(
            parse_headers_rsa_later(w, 25024),
            Err(ParseError::VendorRsa)
        );
        assert_eq!(
            parse_all_rsa_later(w, HEADER_WINDOW_LEN, 25024),
            Err(ParseError::VendorRsa)
        );
    }
}
