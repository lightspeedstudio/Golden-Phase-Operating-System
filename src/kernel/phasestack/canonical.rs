//! Length-prefixed little-endian canonical encoding (PhaseHash tuple discipline).

use crate::kernel::phasestack::types::PhaseStackError;
use crate::platform::Vec;

pub fn put_u8(out: &mut Vec<u8>, v: u8) {
    out.push(v);
}

pub fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

pub fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

pub fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

pub fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_u32(out, bytes.len() as u32);
    out.extend_from_slice(bytes);
}

pub fn take<'a>(buf: &'a [u8], off: &mut usize, n: usize) -> Result<&'a [u8], PhaseStackError> {
    let end = off.checked_add(n).ok_or(PhaseStackError::Overflow)?;
    if end > buf.len() {
        return Err(PhaseStackError::Overflow);
    }
    let slice = &buf[*off..end];
    *off = end;
    Ok(slice)
}

pub fn take_u8(buf: &[u8], off: &mut usize) -> Result<u8, PhaseStackError> {
    Ok(take(buf, off, 1)?[0])
}

pub fn take_u16(buf: &[u8], off: &mut usize) -> Result<u16, PhaseStackError> {
    Ok(u16::from_le_bytes(take(buf, off, 2)?.try_into().unwrap()))
}

pub fn take_u32(buf: &[u8], off: &mut usize) -> Result<u32, PhaseStackError> {
    Ok(u32::from_le_bytes(take(buf, off, 4)?.try_into().unwrap()))
}

pub fn take_u64(buf: &[u8], off: &mut usize) -> Result<u64, PhaseStackError> {
    Ok(u64::from_le_bytes(take(buf, off, 8)?.try_into().unwrap()))
}

pub fn take_fixed<const N: usize>(
    buf: &[u8],
    off: &mut usize,
) -> Result<[u8; N], PhaseStackError> {
    let s = take(buf, off, N)?;
    let mut a = [0u8; N];
    a.copy_from_slice(s);
    Ok(a)
}

pub fn take_lp_bytes(buf: &[u8], off: &mut usize, max: usize) -> Result<Vec<u8>, PhaseStackError> {
    let len = take_u32(buf, off)? as usize;
    if len > max {
        return Err(PhaseStackError::Oversize);
    }
    Ok(take(buf, off, len)?.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_prefix_roundtrip() {
        let mut out = Vec::new();
        put_u16(&mut out, 7);
        put_bytes(&mut out, b"abc");
        let mut off = 0;
        assert_eq!(take_u16(&out, &mut off).unwrap(), 7);
        assert_eq!(take_lp_bytes(&out, &mut off, 8).unwrap(), b"abc");
        assert_eq!(off, out.len());
    }

    #[test]
    fn overflow_rejected() {
        let buf = [1u8, 2];
        let mut off = 0;
        assert_eq!(take_u32(&buf, &mut off), Err(PhaseStackError::Overflow));
    }
}
