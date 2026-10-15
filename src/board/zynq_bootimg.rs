// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Zynq-7000 boot image format (UG585 / BootROM `BOOT.BIN`).
//!
//! Host-testable. This is Golden Phase code for the on-disk layout BootROM
//! and Stage 1 share — not a translation of Xilinx `image_mover.c`.

/// `XLNX` image identification word at byte offset 0x24 (UG585 boot header).
pub const IMAGE_IDENT_XLNX: u32 = 0x584C_4E58;

/// Image header table offset in a Zynq-7000 `BOOT.BIN`.
pub const IHT_OFFSET: usize = 0x8C0;

/// Partition header size (16 little-endian words).
pub const PARTITION_HEADER_LEN: usize = 0x40;

/// Attribute: PS / executable partition.
pub const ATTR_PS: u32 = 0x10;

/// Attribute: PL / bitstream partition.
pub const ATTR_PL: u32 = 0x20;

/// Attribute: Xilinx RSA block present — we do not run vendor RSA.
pub const ATTR_RSA: u32 = 0x8000;

/// Application must live in DDR above the 1 MiB BootROM/OCM window (UG585).
pub const APP_LOAD_MIN: u32 = 0x0010_0000;
pub const APP_LOAD_MAX: u32 = 0x1F00_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootImgError {
    Truncated,
    BadIdent,
    BadIht,
    BadChecksum,
    NoBitstream,
    NoApplication,
    Encrypted,
    VendorRsa,
    BadRange,
    BadSignature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageHeaderTable {
    pub version: u32,
    pub partition_count: u32,
    pub partition_header_off: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartitionHeader {
    pub enc_words: u32,
    pub data_words: u32,
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

    pub fn is_encrypted(&self) -> bool {
        self.enc_words != self.data_words
    }

    pub fn has_vendor_rsa(&self) -> bool {
        (self.attributes & ATTR_RSA) != 0
    }
}

fn u32le(buf: &[u8], off: usize) -> Result<u32, BootImgError> {
    let b = buf.get(off..off + 4).ok_or(BootImgError::Truncated)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Header checksum: `~sum(words[0..15])` (UG585 partition header).
pub fn partition_header_checksum(words: &[u32; 16]) -> u32 {
    let s: u32 = words[..15].iter().fold(0u32, |a, w| a.wrapping_add(*w));
    !s
}

pub fn parse_iht(boot: &[u8]) -> Result<ImageHeaderTable, BootImgError> {
    let ident = u32le(boot, 0x24)?;
    if ident != IMAGE_IDENT_XLNX {
        return Err(BootImgError::BadIdent);
    }
    let version = u32le(boot, IHT_OFFSET)?;
    let count = u32le(boot, IHT_OFFSET + 4)?;
    let ph_words = u32le(boot, IHT_OFFSET + 8)?;
    if count == 0 || count > 14 || (version >> 16) != 0x0102 {
        return Err(BootImgError::BadIht);
    }
    Ok(ImageHeaderTable {
        version,
        partition_count: count,
        partition_header_off: (ph_words as usize).saturating_mul(4),
    })
}

pub fn parse_partition(boot: &[u8], hdr_off: usize) -> Result<PartitionHeader, BootImgError> {
    let mut words = [0u32; 16];
    for i in 0..16 {
        words[i] = u32le(boot, hdr_off + i * 4)?;
    }
    if words[15] != partition_header_checksum(&words) {
        return Err(BootImgError::BadChecksum);
    }
    Ok(PartitionHeader {
        enc_words: words[0],
        data_words: words[1],
        load_addr: words[3],
        exec_addr: words[4],
        data_off: (words[5] as usize).saturating_mul(4),
        attributes: words[6],
    })
}

pub fn parse_partitions(boot: &[u8]) -> Result<(ImageHeaderTable, PartitionHeader, PartitionHeader, PartitionHeader), BootImgError> {
    let iht = parse_iht(boot)?;
    if iht.partition_count < 3 {
        return Err(BootImgError::BadIht);
    }
    let base = iht.partition_header_off;
    let p0 = parse_partition(boot, base)?;
    let p1 = parse_partition(boot, base + PARTITION_HEADER_LEN)?;
    let p2 = parse_partition(boot, base + 2 * PARTITION_HEADER_LEN)?;
    Ok((iht, p0, p1, p2))
}

/// Loader (PS dest 0), PL bitstream, and PS application (load ≥ 1 MiB).
pub fn loader_bitstream_app(
    boot: &[u8],
) -> Result<(PartitionHeader, PartitionHeader, PartitionHeader), BootImgError> {
    let iht = parse_iht(boot)?;
    let mut loader = None;
    let mut bit = None;
    let mut app = None;
    for i in 0..iht.partition_count as usize {
        let p = parse_partition(boot, iht.partition_header_off + i * PARTITION_HEADER_LEN)?;
        let end = p.data_off.checked_add(p.byte_len()).ok_or(BootImgError::Truncated)?;
        if end > boot.len() {
            return Err(BootImgError::Truncated);
        }
        if p.is_encrypted() {
            return Err(BootImgError::Encrypted);
        }
        if p.has_vendor_rsa() {
            return Err(BootImgError::VendorRsa);
        }
        if p.is_pl() {
            bit = Some(p);
        } else if p.is_ps() && p.load_addr >= APP_LOAD_MIN {
            if p.load_addr > APP_LOAD_MAX {
                return Err(BootImgError::BadRange);
            }
            // Jump target has exec in DDR. Extra PT_LOAD segments are
            // data-only (exec 0); skip them here, copy via ps_ddr_partitions.
            if p.exec_addr >= APP_LOAD_MIN {
                app = Some(p);
            }
        } else if p.is_ps() && p.load_addr == 0 {
            loader = Some(p);
        }
    }
    match (loader, bit, app) {
        (Some(l), Some(b), Some(a)) => Ok((l, b, a)),
        (_, None, _) => Err(BootImgError::NoBitstream),
        (_, _, None) => Err(BootImgError::NoApplication),
        (None, _, _) => Err(BootImgError::NoApplication),
    }
}

/// PS payloads that load in DDR (application + extra PT_LOAD, e.g. exec=0 data).
pub const MAX_PS_DDR: usize = 4;

pub fn ps_ddr_partitions(
    boot: &[u8],
    out: &mut [PartitionHeader; MAX_PS_DDR],
) -> Result<usize, BootImgError> {
    let iht = parse_iht(boot)?;
    let mut n = 0usize;
    for i in 0..iht.partition_count as usize {
        let p = parse_partition(boot, iht.partition_header_off + i * PARTITION_HEADER_LEN)?;
        let end = p
            .data_off
            .checked_add(p.byte_len())
            .ok_or(BootImgError::Truncated)?;
        if end > boot.len() {
            return Err(BootImgError::Truncated);
        }
        if p.is_encrypted() {
            return Err(BootImgError::Encrypted);
        }
        if p.has_vendor_rsa() {
            return Err(BootImgError::VendorRsa);
        }
        if p.is_ps() && p.load_addr >= APP_LOAD_MIN {
            if p.load_addr > APP_LOAD_MAX {
                return Err(BootImgError::BadRange);
            }
            if n >= MAX_PS_DDR {
                return Err(BootImgError::BadRange);
            }
            out[n] = p;
            n += 1;
        }
    }
    Ok(n)
}

pub fn partition_bytes<'a>(boot: &'a [u8], p: &PartitionHeader) -> Result<&'a [u8], BootImgError> {
    boot.get(p.data_off..p.data_off + p.byte_len())
        .ok_or(BootImgError::Truncated)
}

/// Strip a Xilinx `.bit` header by finding the configuration sync word.
pub fn strip_bit_header(bytes: &[u8]) -> Option<&[u8]> {
    const SYNC: [u8; 4] = [0xAA, 0x99, 0x55, 0x66];
    if bytes.len() < 4 {
        return None;
    }
    if bytes.starts_with(&SYNC) {
        return Some(bytes);
    }
    bytes.windows(4).position(|w| w == SYNC).map(|i| &bytes[i..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hdr(data_words: u32, load: u32, exec: u32, off_words: u32, attr: u32) -> [u32; 16] {
        let mut w = [0u32; 16];
        w[0] = data_words;
        w[1] = data_words;
        w[2] = data_words;
        w[3] = load;
        w[4] = exec;
        w[5] = off_words;
        w[6] = attr;
        w[15] = partition_header_checksum(&w);
        w
    }

    fn push_u32(buf: &mut Vec<u8>, v: u32) {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    #[test]
    fn strip_finds_sync_word() {
        let mut b = vec![0, 1, 2, 3, 4];
        b.extend_from_slice(&[0xAA, 0x99, 0x55, 0x66, 0x20, 0x00]);
        let stripped = strip_bit_header(&b).unwrap();
        assert_eq!(stripped[0], 0xAA);
        assert_eq!(stripped.len(), 6);
    }

    #[test]
    fn parse_three_partitions() {
        let mut boot = vec![0u8; 0x1000];
        boot[0x24..0x28].copy_from_slice(&IMAGE_IDENT_XLNX.to_le_bytes());
        let ph_words = 0x320u32;
        boot[IHT_OFFSET..IHT_OFFSET + 4].copy_from_slice(&0x0102_0000u32.to_le_bytes());
        boot[IHT_OFFSET + 4..IHT_OFFSET + 8].copy_from_slice(&3u32.to_le_bytes());
        boot[IHT_OFFSET + 8..IHT_OFFSET + 12].copy_from_slice(&ph_words.to_le_bytes());
        let base = (ph_words as usize) * 4;
        assert_eq!(base, 0xC80);
        let parts = [
            hdr(4, 0, 0, 0x400, ATTR_PS),
            hdr(8, 0, 0, 0x401, ATTR_PL),
            hdr(4, 0x0010_0000, 0x0010_0060, 0x403, ATTR_PS),
        ];
        for (i, words) in parts.iter().enumerate() {
            let o = base + i * PARTITION_HEADER_LEN;
            for (j, w) in words.iter().enumerate() {
                boot[o + j * 4..o + j * 4 + 4].copy_from_slice(&w.to_le_bytes());
            }
        }
        boot.resize(0x2000, 0);
        let (loader, bit, app) = loader_bitstream_app(&boot).unwrap();
        assert!(loader.is_ps());
        assert!(bit.is_pl());
        assert_eq!(app.load_addr, 0x0010_0000);
        assert_eq!(app.exec_addr, 0x0010_0060);
    }

    #[test]
    fn encrypted_partition_is_rejected() {
        let mut boot = vec![0u8; 0x1000];
        boot[0x24..0x28].copy_from_slice(&IMAGE_IDENT_XLNX.to_le_bytes());
        let ph_words = 0x320u32;
        boot[IHT_OFFSET..IHT_OFFSET + 4].copy_from_slice(&0x0102_0000u32.to_le_bytes());
        boot[IHT_OFFSET + 4..IHT_OFFSET + 8].copy_from_slice(&3u32.to_le_bytes());
        boot[IHT_OFFSET + 8..IHT_OFFSET + 12].copy_from_slice(&ph_words.to_le_bytes());
        let base = (ph_words as usize) * 4;
        let mut enc = hdr(8, 0, 0, 0x401, ATTR_PL);
        enc[0] = 4;
        enc[15] = partition_header_checksum(&enc);
        let parts = [
            hdr(4, 0, 0, 0x400, ATTR_PS),
            enc,
            hdr(4, 0x0010_0000, 0x0010_0060, 0x403, ATTR_PS),
        ];
        for (i, words) in parts.iter().enumerate() {
            let o = base + i * PARTITION_HEADER_LEN;
            for (j, w) in words.iter().enumerate() {
                boot[o + j * 4..o + j * 4 + 4].copy_from_slice(&w.to_le_bytes());
            }
        }
        boot.resize(0x2000, 0);
        assert_eq!(loader_bitstream_app(&boot), Err(BootImgError::Encrypted));
    }

    #[test]
    fn checksum_rejects_tamper() {
        let mut w = hdr(1, 0, 0, 0, ATTR_PS);
        w[0] = 99;
        assert_ne!(w[15], partition_header_checksum(&w));
        let _ = push_u32;
    }
}
