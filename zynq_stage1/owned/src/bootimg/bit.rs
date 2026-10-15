// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Independent Zynq-7000 encrypted-PL bitstream strip (UG470 + measured P2).
//!
//! Host-std only. Does not copy vendor source.

use super::frame::NOOP_WORD;
use super::stream::StreamError;

const SYNC: u32 = 0xAA99_5566;
const CMD_REG: u32 = 0x04;
const DESYNCH: u32 = 0x0D;
const REBOOT: u32 = 0x0E;
const TYPE1: u32 = 1;
const TYPE2: u32 = 2;
const OP_NOP: u32 = 0;

fn be16(b: &[u8], o: usize) -> Result<u16, StreamError> {
    let s = b.get(o..o + 2).ok_or(StreamError::Bitstream)?;
    Ok(u16::from_be_bytes([s[0], s[1]]))
}

fn be32(b: &[u8], o: usize) -> Result<u32, StreamError> {
    let s = b.get(o..o + 4).ok_or(StreamError::Bitstream)?;
    Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn skip_pascal(b: &[u8], mut o: usize) -> Result<usize, StreamError> {
    let n = be16(b, o)? as usize;
    o += 2;
    if n == 0 {
        return Ok(o);
    }
    o = o.checked_add(n).ok_or(StreamError::Bitstream)?;
    if o > b.len() {
        return Err(StreamError::Bitstream);
    }
    Ok(o)
}

/// `e` payload of an x4k `.bit` (PATH1_G64).
fn bit_e_payload(bit: &[u8]) -> Result<&[u8], StreamError> {
    if bit.first().copied() != Some(0) {
        return Err(StreamError::Bitstream);
    }
    let mut o = skip_pascal(bit, 0)?;
    o = o.checked_add(2).ok_or(StreamError::Bitstream)?;
    if bit.get(o).copied() != Some(b'a') {
        return Err(StreamError::Bitstream);
    }
    o += 1;
    o = skip_pascal(bit, o)?;
    if bit.get(o).copied() != Some(b'b') {
        return Err(StreamError::Bitstream);
    }
    o += 1;
    o = skip_pascal(bit, o)?;
    if bit.get(o).copied() != Some(b'c') {
        return Err(StreamError::Bitstream);
    }
    o += 1;
    o = skip_pascal(bit, o)?;
    if bit.get(o).copied() != Some(b'd') {
        return Err(StreamError::Bitstream);
    }
    o += 1;
    o = skip_pascal(bit, o)?;
    if bit.get(o).copied() != Some(b'e') {
        return Err(StreamError::Bitstream);
    }
    o += 1;
    let n = be32(bit, o)? as usize;
    o += 4;
    bit.get(o..o + n).ok_or(StreamError::Bitstream)
}

/// Encrypted-PL user payload: skip to sync, copy UG470 packets until DESYNCH, drop it, pad 32.
pub fn strip_encrypted_pl(bit: &[u8]) -> Result<Vec<u8>, StreamError> {
    let payload = bit_e_payload(bit)?;
    if payload.len() % 4 != 0 {
        return Err(StreamError::Unaligned);
    }
    let mut off = 0usize;
    loop {
        if off + 4 > payload.len() {
            return Err(StreamError::Bitstream);
        }
        let w = be32(payload, off)?;
        off += 4;
        if w == SYNC {
            break;
        }
    }
    let mut out = Vec::new();
    let mut found = false;
    while off + 4 <= payload.len() {
        let ins = be32(payload, off)?;
        off += 4;
        out.extend_from_slice(&ins.to_le_bytes());
        let typ = (ins >> 29) & 7;
        let op = (ins >> 27) & 3;
        let reg = (ins >> 13) & 0x1F;
        let count = if op == OP_NOP {
            0
        } else if typ == TYPE1 {
            ins & 0x7FF
        } else if typ == TYPE2 {
            ins & 0x07FF_FFFF
        } else {
            return Err(StreamError::Bitstream);
        } as usize;
        let mut val = 0u32;
        let mut j = 0;
        while j < count {
            if off + 4 > payload.len() {
                return Err(StreamError::Bitstream);
            }
            let x = be32(payload, off)?;
            off += 4;
            out.extend_from_slice(&x.to_le_bytes());
            if j == 0 {
                val = x;
            }
            j += 1;
        }
        if reg == CMD_REG && (val == DESYNCH || val == REBOOT) {
            found = true;
            break;
        }
    }
    if !found || out.len() < 8 {
        return Err(StreamError::Bitstream);
    }
    out.truncate(out.len() - 8);
    while out.len() % 32 != 0 {
        out.extend_from_slice(&NOOP_WORD.to_le_bytes());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sha256::sha256;

    fn hex32(h: [u8; 32]) -> String {
        h.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn path1_g64_strip_length() {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit");
        if !p.exists() {
            return;
        }
        let bit = std::fs::read(&p).unwrap();
        assert_eq!(
            hex32(sha256(&bit)),
            "d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829"
        );
        let plain = strip_encrypted_pl(&bit).unwrap();
        assert_eq!(plain.len(), 4_043_904);
        assert_eq!(plain.len() % 32, 0);
        assert_eq!(u32::from_le_bytes(plain[0..4].try_into().unwrap()), NOOP_WORD);
    }
}
