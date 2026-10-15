// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Owned RSA-2048 PKCS#1 v1.5 SHA-256. No bigint/crypto crates.
//!
//! Zynq AC integers are little-endian. Recovered EM is big-endian PKCS#1.

use crate::sha256::sha256;

pub mod cert;
#[cfg(feature = "std")]
pub mod pem;

pub const RSA2048_BYTES: usize = 256;
pub const AC_LEN: usize = 0x6C0;
pub const AUTH_HDR: u32 = 0x0000_0101;
pub const SPK_OFF: usize = 0x280;
pub const SPK_SIG_OFF: usize = 0x4C0;
pub const PART_SIG_OFF: usize = 0x5C0;
pub const KEY_LEN: usize = 0x240;
pub const E_ZYNQ: u32 = 0x0001_0001;

pub const DIGESTINFO: [u8; 19] = [
    0x30, 0x31, 0x30, 0x0D, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05,
    0x00, 0x04, 0x20,
];

const LIMBS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Key,
    Pad,
    Sig,
    Cert,
    Len,
}

type Limbs = [u32; LIMBS];

fn from_le(b: &[u8; RSA2048_BYTES]) -> Limbs {
    let mut o = [0u32; LIMBS];
    let mut i = 0;
    while i < LIMBS {
        let off = i * 4;
        o[i] = u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]);
        i += 1;
    }
    o
}

fn to_le(l: &Limbs) -> [u8; RSA2048_BYTES] {
    let mut o = [0u8; RSA2048_BYTES];
    let mut i = 0;
    while i < LIMBS {
        o[i * 4..i * 4 + 4].copy_from_slice(&l[i].to_le_bytes());
        i += 1;
    }
    o
}

fn to_be(l: &Limbs) -> [u8; RSA2048_BYTES] {
    let mut o = [0u8; RSA2048_BYTES];
    let mut i = 0;
    while i < LIMBS {
        o[i * 4..i * 4 + 4].copy_from_slice(&l[LIMBS - 1 - i].to_be_bytes());
        i += 1;
    }
    o
}

fn cmp(a: &Limbs, b: &Limbs) -> i8 {
    let mut i = LIMBS;
    while i > 0 {
        i -= 1;
        if a[i] < b[i] {
            return -1;
        }
        if a[i] > b[i] {
            return 1;
        }
    }
    0
}

fn sub(a: &Limbs, b: &Limbs) -> Limbs {
    let mut out = [0u32; LIMBS];
    let mut br = 0u32;
    let mut i = 0;
    while i < LIMBS {
        let (x, b1) = a[i].overflowing_sub(b[i]);
        let (y, b2) = x.overflowing_sub(br);
        out[i] = y;
        br = u32::from(b1) | u32::from(b2);
        i += 1;
    }
    out
}

fn bitlen(n: &Limbs) -> u32 {
    let mut i = LIMBS;
    while i > 0 {
        i -= 1;
        if n[i] != 0 {
            return (i as u32) * 32 + (32 - n[i].leading_zeros());
        }
    }
    0
}

fn n0inv(n0: u32) -> u32 {
    let mut y = 1u32;
    y = y.wrapping_mul(2u32.wrapping_sub(n0.wrapping_mul(y)));
    y = y.wrapping_mul(2u32.wrapping_sub(n0.wrapping_mul(y)));
    y = y.wrapping_mul(2u32.wrapping_sub(n0.wrapping_mul(y)));
    y = y.wrapping_mul(2u32.wrapping_sub(n0.wrapping_mul(y)));
    y = y.wrapping_mul(2u32.wrapping_sub(n0.wrapping_mul(y)));
    y.wrapping_neg()
}

fn mont_mul(a: &Limbs, b: &Limbs, n: &Limbs, n0i: u32) -> Limbs {
    let mut t = [0u32; LIMBS + 2];
    let mut i = 0;
    while i < LIMBS {
        let mut c: u128 = 0;
        let mut j = 0;
        while j < LIMBS {
            let p = (t[j] as u128) + (a[j] as u128) * (b[i] as u128) + c;
            t[j] = p as u32;
            c = p >> 32;
            j += 1;
        }
        let p = (t[LIMBS] as u128) + c;
        t[LIMBS] = p as u32;
        t[LIMBS + 1] = (p >> 32) as u32;

        let m = t[0].wrapping_mul(n0i);
        c = 0;
        j = 0;
        while j < LIMBS {
            let p = (t[j] as u128) + (m as u128) * (n[j] as u128) + c;
            t[j] = p as u32;
            c = p >> 32;
            j += 1;
        }
        let p = (t[LIMBS] as u128) + c;
        t[LIMBS] = p as u32;
        t[LIMBS + 1] = ((t[LIMBS + 1] as u128) + (p >> 32)) as u32;

        j = 0;
        while j <= LIMBS {
            t[j] = t[j + 1];
            j += 1;
        }
        t[LIMBS + 1] = 0;
        i += 1;
    }
    let mut out = [0u32; LIMBS];
    out.copy_from_slice(&t[..LIMBS]);
    if t[LIMBS] != 0 || cmp(&out, n) >= 0 {
        out = sub(&out, n);
        if t[LIMBS] > 1 || cmp(&out, n) >= 0 {
            out = sub(&out, n);
        }
    }
    out
}

fn one() -> Limbs {
    let mut o = [0u32; LIMBS];
    o[0] = 1;
    o
}

/// `R^2 mod n` with `R = 2^{bitlen(n)}` (Bootgen / UG585 modulus extension).
pub fn rr_mod_n(n_le: &[u8; RSA2048_BYTES]) -> Result<[u8; RSA2048_BYTES], Error> {
    let n = from_le(n_le);
    if (n[0] & 1) == 0 {
        return Err(Error::Key);
    }
    let bits = bitlen(&n);
    if bits < 2 || bits > 2048 {
        return Err(Error::Key);
    }
    let mut x = [0u32; LIMBS + 1];
    x[0] = 1;
    let mut k = 0u32;
    while k < bits * 2 {
        let mut c = 0u32;
        let mut i = 0;
        while i < LIMBS + 1 {
            let v = (x[i] << 1) | c;
            c = x[i] >> 31;
            x[i] = v;
            i += 1;
        }
        let mut ge = x[LIMBS] != 0;
        if !ge {
            let mut tmp = [0u32; LIMBS];
            tmp.copy_from_slice(&x[..LIMBS]);
            ge = cmp(&tmp, &n) >= 0;
        }
        if ge {
            let mut br = 0u32;
            let mut i = 0;
            while i < LIMBS {
                let (a, b1) = x[i].overflowing_sub(n[i]);
                let (b, b2) = a.overflowing_sub(br);
                x[i] = b;
                br = u32::from(b1) | u32::from(b2);
                i += 1;
            }
            x[LIMBS] = x[LIMBS].wrapping_sub(br);
        }
        k += 1;
    }
    if x[LIMBS] != 0 {
        return Err(Error::Key);
    }
    let mut out = [0u32; LIMBS];
    out.copy_from_slice(&x[..LIMBS]);
    Ok(to_le(&out))
}

fn modexp(base: &Limbs, exp: &Limbs, n: &Limbs, rr: &Limbs) -> Limbs {
    let n0i = n0inv(n[0]);
    let one_m = mont_mul(&one(), rr, n, n0i);
    let base_m = mont_mul(base, rr, n, n0i);
    let mut acc = one_m;
    let mut i = LIMBS;
    while i > 0 {
        i -= 1;
        let mut b = 32u32;
        while b > 0 {
            b -= 1;
            acc = mont_mul(&acc, &acc, n, n0i);
            if ((exp[i] >> b) & 1) != 0 {
                acc = mont_mul(&acc, &base_m, n, n0i);
            }
        }
    }
    mont_mul(&acc, &one(), n, n0i)
}

fn exp_limbs(e: u32) -> Limbs {
    let mut o = [0u32; LIMBS];
    o[0] = e;
    o
}

fn pkcs1_em(hash: &[u8; 32]) -> [u8; RSA2048_BYTES] {
    let mut em = [0u8; RSA2048_BYTES];
    em[0] = 0x00;
    em[1] = 0x01;
    let mut i = 2;
    while i < 204 {
        em[i] = 0xFF;
        i += 1;
    }
    em[204] = 0x00;
    em[205..224].copy_from_slice(&DIGESTINFO);
    em[224..256].copy_from_slice(hash);
    em
}

fn em_ok(em: &[u8; RSA2048_BYTES], hash: &[u8; 32]) -> bool {
    if em[0] != 0x00 || em[1] != 0x01 || em[204] != 0x00 {
        return false;
    }
    let mut i = 2;
    while i < 204 {
        if em[i] != 0xFF {
            return false;
        }
        i += 1;
    }
    if em[205..224] != DIGESTINFO {
        return false;
    }
    em[224..256] == *hash
}

fn be_to_limbs(em: &[u8; RSA2048_BYTES]) -> Limbs {
    let mut le = [0u8; RSA2048_BYTES];
    let mut i = 0;
    while i < RSA2048_BYTES {
        le[i] = em[RSA2048_BYTES - 1 - i];
        i += 1;
    }
    from_le(&le)
}

pub fn verify_pkcs1_sha256_digest(
    n_le: &[u8; RSA2048_BYTES],
    e: u32,
    sig_le: &[u8; RSA2048_BYTES],
    hash: &[u8; 32],
) -> Result<(), Error> {
    if e != E_ZYNQ {
        return Err(Error::Key);
    }
    let n = from_le(n_le);
    if (n[0] & 1) == 0 || bitlen(&n) < 2047 {
        return Err(Error::Key);
    }
    let s = from_le(sig_le);
    if cmp(&s, &n) >= 0 || cmp(&s, &one()) < 0 {
        return Err(Error::Sig);
    }
    let rr = from_le(&rr_mod_n(n_le)?);
    let m = modexp(&s, &exp_limbs(e), &n, &rr);
    let em = to_be(&m);
    if !em_ok(&em, hash) {
        return Err(Error::Pad);
    }
    Ok(())
}

/// PKCS#1 v1.5 SHA-256 verify. `n` and `sig` are Zynq little-endian.
pub fn verify_pkcs1_sha256(
    n_le: &[u8; RSA2048_BYTES],
    e: u32,
    sig_le: &[u8; RSA2048_BYTES],
    msg: &[u8],
) -> Result<(), Error> {
    verify_pkcs1_sha256_digest(n_le, e, sig_le, &sha256(msg))
}

pub fn sign_pkcs1_sha256_digest(
    n_le: &[u8; RSA2048_BYTES],
    d_le: &[u8; RSA2048_BYTES],
    hash: &[u8; 32],
) -> Result<[u8; RSA2048_BYTES], Error> {
    let n = from_le(n_le);
    let d = from_le(d_le);
    if (n[0] & 1) == 0 || bitlen(&n) < 2047 {
        return Err(Error::Key);
    }
    if cmp(&d, &n) >= 0 || cmp(&d, &one()) < 0 {
        return Err(Error::Key);
    }
    let em = pkcs1_em(hash);
    let m = be_to_limbs(&em);
    if cmp(&m, &n) >= 0 {
        return Err(Error::Pad);
    }
    let rr = from_le(&rr_mod_n(n_le)?);
    let s = modexp(&m, &d, &n, &rr);
    Ok(to_le(&s))
}

/// PKCS#1 v1.5 SHA-256 sign. `n` and `d` are little-endian. Output is Zynq LE.
pub fn sign_pkcs1_sha256(
    n_le: &[u8; RSA2048_BYTES],
    d_le: &[u8; RSA2048_BYTES],
    msg: &[u8],
) -> Result<[u8; RSA2048_BYTES], Error> {
    sign_pkcs1_sha256_digest(n_le, d_le, &sha256(msg))
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
        assert_eq!(t.len(), 512);
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

    fn unhex_be_sig(s: &str) -> [u8; RSA2048_BYTES] {
        unhex(s)
    }

    #[test]
    fn digestinfo_and_ff_count() {
        let em = pkcs1_em(&[0u8; 32]);
        assert_eq!(&em[205..224], &DIGESTINFO);
        let mut n = 0usize;
        let mut i = 0;
        while i < 256 {
            if em[i] == 0xFF {
                n += 1;
            }
            i += 1;
        }
        assert_eq!(n, 202);
        assert_eq!(&em[..4], &[0x00, 0x01, 0xFF, 0xFF]);
    }

    #[test]
    fn oracle_spk_public_op_matches_python() {
        let ac = include_bytes!("../../../../docs/boot/SYS07_ORACLE_P1_AC.bin");
        let mut n = [0u8; RSA2048_BYTES];
        n.copy_from_slice(&ac[0x40..0x140]);
        let mut sig = [0u8; RSA2048_BYTES];
        sig.copy_from_slice(&ac[0x4C0..0x5C0]);
        let n0 = from_le(&n)[0];
        assert_eq!(n0.wrapping_mul(n0inv(n0)), u32::MAX);
        let nl = from_le(&n);
        let s = from_le(&sig);
        let rr = from_le(&rr_mod_n(&n).unwrap());
        assert_eq!(to_le(&rr), {
            let mut ext = [0u8; RSA2048_BYTES];
            ext.copy_from_slice(&ac[0x140..0x240]);
            ext
        });
        let m = modexp(&s, &exp_limbs(E_ZYNQ), &nl, &rr);
        let em = to_be(&m);
        let h = sha256(&ac[SPK_OFF..SPK_SIG_OFF]);
        assert!(
            em_ok(&em, &h),
            "em[0]={:02x} em[1]={:02x} em[204]={:02x} di_ok={} h_ok={}",
            em[0],
            em[1],
            em[204],
            em[205..224] == DIGESTINFO,
            em[224..256] == h
        );
        verify_pkcs1_sha256_digest(&n, E_ZYNQ, &sig, &h).unwrap();
    }

    #[test]
    fn wycheproof_tcid3_valid() {
        let n = unhex("00a2b451a07d0aa5f96e455671513550514a8a5b462ebef717094fa1fee82224e637f9746d3f7cafd31878d80325b6ef5a1700f65903b469429e89d6eac8845097b5ab393189db92512ed8a7711a1253facd20f79c15e8247f3d3e42e46e48c98e254a2fe9765313a03eff8f17e1a029397a1fa26a8dce26f490ed81299615d9814c22da610428e09c7d9658594266f5c021d0fceca08d945a12be82de4d1ece6b4c03145b5d3495d4ed5411eb878daf05fd7afc3e09ada0f1126422f590975a1969816f48698bcbba1b4d9cae79d460d8f9f85e7975005d9bc22c4e5ac0f7c1a45d12569a62807d3b9a02e5a530e773066f453d1f5b4c2e9cf7820283f742b9d5");
        let sig = unhex_be_sig("264491e844c119f14e425c03282139a558dcdaeb82a4628173cd407fd319f9076eaebc0dd87a1c22e4d17839096886d58a9d5b7f7aeb63efec56c45ac7bead4203b6886e1faa90e028ec0ae094d46bf3f97efdd19045cfbc25a1abda2432639f9876405c0d68f8edbf047c12a454f7681d5d5a2b54bd3723d193dbad4338baad753264006e2d08931c4b8bb79aa1c9cad10eb6605f87c5831f6e2b08e002f9c6f21141f5841d92727dd3e1d99c36bc560da3c9067df99fcaf818941f72588be33032bad22caf6704223bb114d575b6d02d9d222b580005d930e8f40cce9f672eebb634a20177d84351627964b83f2053d736a84ab1a005f63bd5ba943de6205c");
        verify_pkcs1_sha256(&n, E_ZYNQ, &sig, b"Test").unwrap();
    }

    #[test]
    fn wycheproof_round_trip() {
        let n = unhex("00b3510a2bcd4ce644c5b594ae5059e12b2f054b658d5da5959a2fdf1871b808bc3df3e628d2792e51aad5c124b43bda453dca5cde4bcf28e7bd4effba0cb4b742bbb6d5a013cb63d1aa3a89e02627ef5398b52c0cfd97d208abeb8d7c9bce0bbeb019a86ddb589beb29a5b74bf861075c677c81d430f030c265247af9d3c9140ccb65309d07e0adc1efd15cf17e7b055d7da3868e4648cc3a180f0ee7f8e1e7b18098a3391b4ce7161e98d57af8a947e201a463e2d6bbca8059e5706e9dfed8f4856465ffa712ed1aa18e888d12dc6aa09ce95ecfca83cc5b0b15db09c8647f5d524c0f2e7620a3416b9623cadc0f097af573261c98c8400aa12af38e43cad84d");
        let d = unhex("1a502d0eea6c7b69e21d5839101f705456ed0ef852fb47fe21071f54c5f33c8ceb066c62d727e32d26c58137329f89d3195325b795264c195d85472f7507dbd0961d2951f935a26b34f0ac24d15490e1128a9b7138915bc7dbfa8fe396357131c543ae9c98507368d9ceb08c1c6198a3eda7aea185a0e976cd42c22d00f003d9f19d96ea4c9afcbfe1441ccc802cfb0689f59d804c6a4e4f404c15174745ed6cb8bc88ef0b33ba0d2a80e35e43bc90f350052e72016e75b00d357a381c9c0d467069ca660887c987766349fcc43460b4aa516bce079edd87ba164307b752c277ed9528ad3ba0bf1877349ed3b7966a6c240110409bf4d0fade0c68fdadd847fd");
        let msg = b"SYS07 round-trip";
        let sig = sign_pkcs1_sha256(&n, &d, msg).unwrap();
        verify_pkcs1_sha256(&n, E_ZYNQ, &sig, msg).unwrap();
        let mut bad = sig;
        bad[0] ^= 1;
        assert_eq!(verify_pkcs1_sha256(&n, E_ZYNQ, &bad, msg), Err(Error::Pad));
        assert_eq!(verify_pkcs1_sha256(&n, E_ZYNQ, &sig, b"nope"), Err(Error::Pad));
        assert_eq!(
            verify_pkcs1_sha256(&n, 3, &sig, msg),
            Err(Error::Key)
        );
    }

    #[test]
    fn malformed_pkcs1_fails() {
        let n = unhex("00b3510a2bcd4ce644c5b594ae5059e12b2f054b658d5da5959a2fdf1871b808bc3df3e628d2792e51aad5c124b43bda453dca5cde4bcf28e7bd4effba0cb4b742bbb6d5a013cb63d1aa3a89e02627ef5398b52c0cfd97d208abeb8d7c9bce0bbeb019a86ddb589beb29a5b74bf861075c677c81d430f030c265247af9d3c9140ccb65309d07e0adc1efd15cf17e7b055d7da3868e4648cc3a180f0ee7f8e1e7b18098a3391b4ce7161e98d57af8a947e201a463e2d6bbca8059e5706e9dfed8f4856465ffa712ed1aa18e888d12dc6aa09ce95ecfca83cc5b0b15db09c8647f5d524c0f2e7620a3416b9623cadc0f097af573261c98c8400aa12af38e43cad84d");
        assert_eq!(
            verify_pkcs1_sha256(&n, E_ZYNQ, &[0u8; 256], b"x"),
            Err(Error::Sig)
        );
        assert_eq!(
            verify_pkcs1_sha256(&n, E_ZYNQ, &n, b"x"),
            Err(Error::Sig)
        );
        let mut even = n;
        even[0] &= !1;
        assert_eq!(
            verify_pkcs1_sha256(&even, E_ZYNQ, &[1u8; 256], b"x"),
            Err(Error::Key)
        );
    }
}
