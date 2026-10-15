// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Zynq-7000 RSA-2048 authentication certificate (UG585 / measured oracle).

use super::{
    rr_mod_n, sign_pkcs1_sha256, sign_pkcs1_sha256_digest, verify_pkcs1_sha256,
    verify_pkcs1_sha256_digest, Error, AC_LEN, AUTH_HDR, E_ZYNQ, KEY_LEN, PART_SIG_OFF,
    RSA2048_BYTES, SPK_OFF, SPK_SIG_OFF,
};
use crate::sha256::sha256_concat;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PubKey {
    pub n: [u8; RSA2048_BYTES],
    pub n_ext: [u8; RSA2048_BYTES],
    pub e: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthCert {
    pub header: u32,
    pub size: u32,
    pub ppk: PubKey,
    pub spk: PubKey,
    pub spk_sig: [u8; RSA2048_BYTES],
    pub part_sig: [u8; RSA2048_BYTES],
}

fn u32le(b: &[u8], off: usize) -> Result<u32, Error> {
    let s = b.get(off..off + 4).ok_or(Error::Cert)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn copy256(b: &[u8], off: usize) -> Result<[u8; RSA2048_BYTES], Error> {
    let s = b.get(off..off + RSA2048_BYTES).ok_or(Error::Cert)?;
    let mut o = [0u8; RSA2048_BYTES];
    o.copy_from_slice(s);
    Ok(o)
}

fn parse_key(ac: &[u8], off: usize) -> Result<PubKey, Error> {
    let n = copy256(ac, off)?;
    let n_ext = copy256(ac, off + RSA2048_BYTES)?;
    let e = u32le(ac, off + 2 * RSA2048_BYTES)?;
    let pad = ac
        .get(off + 2 * RSA2048_BYTES + 4..off + KEY_LEN)
        .ok_or(Error::Cert)?;
    if pad.iter().any(|b| *b != 0) {
        return Err(Error::Cert);
    }
    if e != E_ZYNQ {
        return Err(Error::Key);
    }
    let want = rr_mod_n(&n)?;
    if n_ext != want {
        return Err(Error::Key);
    }
    Ok(PubKey { n, n_ext, e })
}

/// Validate AC layout and `N_ext` without keeping the full certificate on the stack.
pub fn validate_cert(ac: &[u8]) -> Result<(), Error> {
    if ac.len() != AC_LEN {
        return Err(Error::Len);
    }
    let header = u32le(ac, 0)?;
    let size = u32le(ac, 4)?;
    if header != AUTH_HDR || size != AC_LEN as u32 {
        return Err(Error::Cert);
    }
    let _ = parse_key(ac, 0x40)?;
    let _ = parse_key(ac, SPK_OFF)?;
    Ok(())
}

/// Parse a 1728-byte Zynq AC. Checks header, size, exponent pad, and `N_ext = R² mod N`.
pub fn parse_cert(ac: &[u8]) -> Result<AuthCert, Error> {
    validate_cert(ac)?;
    let header = u32le(ac, 0)?;
    let size = u32le(ac, 4)?;
    Ok(AuthCert {
        header,
        size,
        ppk: parse_key(ac, 0x40)?,
        spk: parse_key(ac, SPK_OFF)?,
        spk_sig: copy256(ac, SPK_SIG_OFF)?,
        part_sig: copy256(ac, PART_SIG_OFF)?,
    })
}

pub fn verify_spk(ac: &[u8]) -> Result<(), Error> {
    validate_cert(ac)?;
    let n = copy256(ac, 0x40)?;
    let sig = copy256(ac, SPK_SIG_OFF)?;
    verify_pkcs1_sha256(&n, E_ZYNQ, &sig, &ac[SPK_OFF..SPK_SIG_OFF])
}

/// Later-partition verify: `SHA-256(partition ‖ AC without last 256 bytes)` under SPK.
pub fn verify_later_partition(payload: &[u8], ac: &[u8]) -> Result<(), Error> {
    if payload.is_empty() || (payload.len() & 0x3F) != 0 {
        return Err(Error::Len);
    }
    verify_spk(ac)?;
    let n = copy256(ac, SPK_OFF)?;
    let sig = copy256(ac, PART_SIG_OFF)?;
    let h = sha256_concat(payload, &ac[..PART_SIG_OFF]);
    verify_pkcs1_sha256_digest(&n, E_ZYNQ, &sig, &h)
}

fn put_u32(ac: &mut [u8], off: usize, v: u32) {
    ac[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_key(ac: &mut [u8], off: usize, n: &[u8; RSA2048_BYTES], e: u32) -> Result<(), Error> {
    let n_ext = rr_mod_n(n)?;
    ac[off..off + RSA2048_BYTES].copy_from_slice(n);
    ac[off + RSA2048_BYTES..off + 2 * RSA2048_BYTES].copy_from_slice(&n_ext);
    put_u32(ac, off + 2 * RSA2048_BYTES, e);
    Ok(())
}

/// Host: build a later-partition AC and sign with PSK/SSK (`d` little-endian).
pub fn sign_later_partition(
    ppk_n: &[u8; RSA2048_BYTES],
    psk_d: &[u8; RSA2048_BYTES],
    spk_n: &[u8; RSA2048_BYTES],
    ssk_d: &[u8; RSA2048_BYTES],
    payload: &[u8],
) -> Result<[u8; AC_LEN], Error> {
    if payload.is_empty() || (payload.len() & 0x3F) != 0 {
        return Err(Error::Len);
    }
    let mut ac = [0u8; AC_LEN];
    put_u32(&mut ac, 0, AUTH_HDR);
    put_u32(&mut ac, 4, AC_LEN as u32);
    put_key(&mut ac, 0x40, ppk_n, E_ZYNQ)?;
    put_key(&mut ac, SPK_OFF, spk_n, E_ZYNQ)?;
    let spk_sig = sign_pkcs1_sha256(ppk_n, psk_d, &ac[SPK_OFF..SPK_SIG_OFF])?;
    ac[SPK_SIG_OFF..PART_SIG_OFF].copy_from_slice(&spk_sig);
    let h = sha256_concat(payload, &ac[..PART_SIG_OFF]);
    let part_sig = sign_pkcs1_sha256_digest(spk_n, ssk_d, &h)?;
    ac[PART_SIG_OFF..AC_LEN].copy_from_slice(&part_sig);
    verify_later_partition(payload, &ac)?;
    Ok(ac)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> [u8; RSA2048_BYTES] {
        let t = if s.len() == 514 && s.starts_with("00") {
            &s[2..]
        } else {
            s
        };
        let mut be = [0u8; RSA2048_BYTES];
        let mut i = 0;
        while i < RSA2048_BYTES {
            be[i] = u8::from_str_radix(&t[i * 2..i * 2 + 2], 16).unwrap();
            i += 1;
        }
        let mut le = [0u8; RSA2048_BYTES];
        let mut i = 0;
        while i < RSA2048_BYTES {
            le[i] = be[RSA2048_BYTES - 1 - i];
            i += 1;
        }
        le
    }

    const WP_N: &str = "00b3510a2bcd4ce644c5b594ae5059e12b2f054b658d5da5959a2fdf1871b808bc3df3e628d2792e51aad5c124b43bda453dca5cde4bcf28e7bd4effba0cb4b742bbb6d5a013cb63d1aa3a89e02627ef5398b52c0cfd97d208abeb8d7c9bce0bbeb019a86ddb589beb29a5b74bf861075c677c81d430f030c265247af9d3c9140ccb65309d07e0adc1efd15cf17e7b055d7da3868e4648cc3a180f0ee7f8e1e7b18098a3391b4ce7161e98d57af8a947e201a463e2d6bbca8059e5706e9dfed8f4856465ffa712ed1aa18e888d12dc6aa09ce95ecfca83cc5b0b15db09c8647f5d524c0f2e7620a3416b9623cadc0f097af573261c98c8400aa12af38e43cad84d";
    const WP_D: &str = "1a502d0eea6c7b69e21d5839101f705456ed0ef852fb47fe21071f54c5f33c8ceb066c62d727e32d26c58137329f89d3195325b795264c195d85472f7507dbd0961d2951f935a26b34f0ac24d15490e1128a9b7138915bc7dbfa8fe396357131c543ae9c98507368d9ceb08c1c6198a3eda7aea185a0e976cd42c22d00f003d9f19d96ea4c9afcbfe1441ccc802cfb0689f59d804c6a4e4f404c15174745ed6cb8bc88ef0b33ba0d2a80e35e43bc90f350052e72016e75b00d357a381c9c0d467069ca660887c987766349fcc43460b4aa516bce079edd87ba164307b752c277ed9528ad3ba0bf1877349ed3b7966a6c240110409bf4d0fade0c68fdadd847fd";

    #[test]
    fn oracle_p1_cert_and_partition() {
        let ac = include_bytes!("../../../../docs/boot/SYS07_ORACLE_P1_AC.bin");
        let p1 = include_bytes!("../../../../docs/boot/SYS07_ORACLE_P1.bin");
        assert_eq!(ac.len(), AC_LEN);
        assert_eq!(p1.len(), 256);
        let c = parse_cert(ac).unwrap();
        assert_eq!(c.header, AUTH_HDR);
        assert_eq!(c.size, AC_LEN as u32);
        assert_eq!(c.ppk.e, E_ZYNQ);
        assert_eq!(c.spk.e, E_ZYNQ);
        assert_eq!(
            crate::sha256::sha256(&ac[SPK_OFF..SPK_SIG_OFF]),
            [
                0xbe, 0xed, 0xb7, 0x65, 0xae, 0xc9, 0x74, 0x83, 0x82, 0x93, 0x1a, 0x9b, 0x3a, 0x3d,
                0xbb, 0x12, 0xa9, 0x0e, 0x78, 0xf4, 0x7e, 0x59, 0x94, 0xf6, 0x9c, 0x7d, 0xe0, 0xc6,
                0xb9, 0xc1, 0x3d, 0xce
            ]
        );
        verify_spk(ac).unwrap();
        verify_later_partition(p1, ac).unwrap();
        let mut bad_hdr = *ac;
        bad_hdr[0] ^= 1;
        assert!(parse_cert(&bad_hdr).is_err());
        let mut bad_sig = *ac;
        bad_sig[PART_SIG_OFF] ^= 1;
        assert!(verify_later_partition(p1, &bad_sig).is_err());
        let mut bad_spk = *ac;
        bad_spk[SPK_SIG_OFF] ^= 1;
        assert!(verify_spk(&bad_spk).is_err());
        let mut bad_n = *ac;
        bad_n[0x40] ^= 1;
        assert!(parse_cert(&bad_n).is_err());
        let mut short = [0u8; 16];
        short.copy_from_slice(&ac[..16]);
        assert_eq!(parse_cert(&short), Err(Error::Len));
        let mut unaligned = [0u8; 257];
        unaligned[..256].copy_from_slice(p1);
        assert_eq!(
            verify_later_partition(&unaligned, ac),
            Err(Error::Len)
        );
        assert_eq!(verify_later_partition(&[], ac), Err(Error::Len));
    }

    #[test]
    fn sign_later_round_trip() {
        let n = unhex(WP_N);
        let d = unhex(WP_D);
        let mut p1 = [0u8; 256];
        let mut i = 0;
        while i < 256 {
            p1[i] = i as u8;
            i += 1;
        }
        let ac = sign_later_partition(&n, &d, &n, &d, &p1).unwrap();
        verify_later_partition(&p1, &ac).unwrap();
        let mut p1b = p1;
        p1b[0] ^= 1;
        assert!(verify_later_partition(&p1b, &ac).is_err());
    }
}
