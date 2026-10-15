// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! PKCS#1 / PKCS#8 RSA PEM → little-endian n, e, d. Host packer only.

use super::{Error, E_ZYNQ, RSA2048_BYTES};

fn der_len(buf: &[u8], i: usize) -> Result<(usize, usize), Error> {
    let first = *buf.get(i).ok_or(Error::Key)?;
    if first < 0x80 {
        return Ok((first as usize, i + 1));
    }
    let n = (first & 0x7F) as usize;
    if n == 0 || n > 3 || i + 1 + n > buf.len() {
        return Err(Error::Key);
    }
    let mut v = 0usize;
    let mut k = 0;
    while k < n {
        v = (v << 8) | (buf[i + 1 + k] as usize);
        k += 1;
    }
    Ok((v, i + 1 + n))
}

fn parse_int<'a>(buf: &'a [u8], i: usize) -> Result<(&'a [u8], usize), Error> {
    if *buf.get(i).ok_or(Error::Key)? != 0x02 {
        return Err(Error::Key);
    }
    let (ln, j) = der_len(buf, i + 1)?;
    let end = j.checked_add(ln).ok_or(Error::Key)?;
    if end > buf.len() {
        return Err(Error::Key);
    }
    Ok((&buf[j..end], end))
}

fn int_to_le256(raw: &[u8]) -> Result<[u8; RSA2048_BYTES], Error> {
    let mut p = raw;
    while p.len() > RSA2048_BYTES && p[0] == 0 {
        p = &p[1..];
    }
    if p.len() > RSA2048_BYTES {
        return Err(Error::Key);
    }
    let mut be = [0u8; RSA2048_BYTES];
    be[RSA2048_BYTES - p.len()..].copy_from_slice(p);
    let mut le = [0u8; RSA2048_BYTES];
    let mut i = 0;
    while i < RSA2048_BYTES {
        le[i] = be[RSA2048_BYTES - 1 - i];
        i += 1;
    }
    Ok(le)
}

fn b64_val(c: u8) -> Result<u8, Error> {
    match c {
        b'A'..=b'Z' => Ok(c - b'A'),
        b'a'..=b'z' => Ok(c - b'a' + 26),
        b'0'..=b'9' => Ok(c - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(Error::Key),
    }
}

fn decode_pem(text: &str) -> Result<Vec<u8>, Error> {
    let mut b64 = String::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("-----") {
            continue;
        }
        b64.push_str(t);
    }
    if b64.len() % 4 != 0 {
        return Err(Error::Key);
    }
    let mut out = Vec::new();
    let bytes = b64.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let pad2 = bytes[i + 2] == b'=';
        let pad3 = bytes[i + 3] == b'=';
        let a = b64_val(bytes[i])?;
        let b = b64_val(bytes[i + 1])?;
        out.push((a << 2) | (b >> 4));
        if !pad2 {
            let c = b64_val(bytes[i + 2])?;
            out.push((b << 4) | (c >> 2));
            if !pad3 {
                let d = b64_val(bytes[i + 3])?;
                out.push((c << 6) | d);
            }
        }
        i += 4;
    }
    Ok(out)
}

fn parse_pkcs1_rsa(der: &[u8]) -> Result<([u8; RSA2048_BYTES], u32, [u8; RSA2048_BYTES]), Error> {
    if *der.first().ok_or(Error::Key)? != 0x30 {
        return Err(Error::Key);
    }
    let (_, mut i) = der_len(der, 1)?;
    let (_ver, j) = parse_int(der, i)?;
    i = j;
    let (n_raw, j) = parse_int(der, i)?;
    i = j;
    let (e_raw, j) = parse_int(der, i)?;
    i = j;
    let (d_raw, _) = parse_int(der, i)?;
    let n = int_to_le256(n_raw)?;
    let e_be = int_to_le256(e_raw)?;
    let e = u32::from_le_bytes([e_be[0], e_be[1], e_be[2], e_be[3]]);
    if e != E_ZYNQ {
        return Err(Error::Key);
    }
    let d = int_to_le256(d_raw)?;
    Ok((n, e, d))
}

fn skip_seq_or_oid(der: &[u8], i: usize) -> Result<usize, Error> {
    let tag = *der.get(i).ok_or(Error::Key)?;
    let (ln, j) = der_len(der, i + 1)?;
    let end = j.checked_add(ln).ok_or(Error::Key)?;
    if end > der.len() {
        return Err(Error::Key);
    }
    let _ = tag;
    Ok(end)
}

/// Parse `BEGIN RSA PRIVATE KEY` or `BEGIN PRIVATE KEY` (PKCS#8 wrapping PKCS#1).
pub fn parse_rsa_private_pem(
    text: &str,
) -> Result<([u8; RSA2048_BYTES], u32, [u8; RSA2048_BYTES]), Error> {
    let der = decode_pem(text)?;
    if der.first() != Some(&0x30) {
        return Err(Error::Key);
    }
    let (_, i) = der_len(&der, 1)?;
    if der.get(i) == Some(&0x02) {
        return parse_pkcs1_rsa(&der);
    }
    // PKCS#8: SEQUENCE { INTEGER, AlgorithmIdentifier, OCTET STRING }
    let mut p = i;
    p = skip_seq_or_oid(&der, p)?;
    p = skip_seq_or_oid(&der, p)?;
    if *der.get(p).ok_or(Error::Key)? != 0x04 {
        return Err(Error::Key);
    }
    let (ln, j) = der_len(&der, p + 1)?;
    let inner = der.get(j..j + ln).ok_or(Error::Key)?;
    parse_pkcs1_rsa(inner)
}

pub fn parse_rsa_public_pem(text: &str) -> Result<([u8; RSA2048_BYTES], u32), Error> {
    let der = decode_pem(text)?;
    if der.first() != Some(&0x30) {
        return Err(Error::Key);
    }
    let (_, mut i) = der_len(&der, 1)?;
    i = skip_seq_or_oid(&der, i)?;
    if *der.get(i).ok_or(Error::Key)? != 0x03 {
        return Err(Error::Key);
    }
    let (ln, j) = der_len(&der, i + 1)?;
    let bits = der.get(j..j + ln).ok_or(Error::Key)?;
    if bits.first() != Some(&0x00) {
        return Err(Error::Key);
    }
    let inner = &bits[1..];
    if inner.first() != Some(&0x30) {
        return Err(Error::Key);
    }
    let (_, mut k) = der_len(inner, 1)?;
    let (n_raw, t) = parse_int(inner, k)?;
    k = t;
    let (e_raw, _) = parse_int(inner, k)?;
    let n = int_to_le256(n_raw)?;
    let e_be = int_to_le256(e_raw)?;
    let e = u32::from_le_bytes([e_be[0], e_be[1], e_be[2], e_be[3]]);
    if e != E_ZYNQ {
        return Err(Error::Key);
    }
    Ok((n, e))
}
