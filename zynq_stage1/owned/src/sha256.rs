// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! FIPS 180-4 SHA-256 for artifact digests only.
//!
//! This is not a boot crypto engine. It does not implement AES, HMAC, RSA,
//! or Ed25519.

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
    0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
    0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
    0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
    0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
    0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
    0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
    0xc67178f2,
];

fn rotr(x: u32, n: u32) -> u32 {
    x.rotate_right(n)
}

pub(crate) fn initial_state() -> [u32; 8] {
    [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ]
}

pub(crate) fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        let o = i * 4;
        w[i] = u32::from_be_bytes([block[o], block[o + 1], block[o + 2], block[o + 3]]);
    }
    for i in 16..64 {
        let s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
        let s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }
    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];
    for i in 0..64 {
        let s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
        let ch = (e & f) ^ ((!e) & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

fn finish(state: &mut [u32; 8], tail: &[u8], total_len: usize) -> [u8; 32] {
    let mut block = [0u8; 64];
    let rem = tail.len();
    block[..rem].copy_from_slice(tail);
    block[rem] = 0x80;
    let bit_len = (total_len as u64).wrapping_mul(8);
    if rem >= 56 {
        compress(state, &block);
        block = [0u8; 64];
    }
    block[56..64].copy_from_slice(&bit_len.to_be_bytes());
    compress(state, &block);
    let mut out = [0u8; 32];
    for (j, w) in state.iter().enumerate() {
        out[j * 4..j * 4 + 4].copy_from_slice(&w.to_be_bytes());
    }
    out
}

fn absorb(state: &mut [u32; 8], data: &[u8], mut i: usize) -> usize {
    while i + 64 <= data.len() {
        let mut block = [0u8; 64];
        block.copy_from_slice(&data[i..i + 64]);
        compress(state, &block);
        i += 64;
    }
    i
}

/// SHA-256 of `data`.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state = initial_state();
    let i = absorb(&mut state, data, 0);
    finish(&mut state, &data[i..], data.len())
}

/// SHA-256 of `a ‖ b` without a combined allocation.
pub fn sha256_concat(a: &[u8], b: &[u8]) -> [u8; 32] {
    let mut state = initial_state();
    let i = absorb(&mut state, a, 0);
    if i < a.len() {
        let rem = a.len() - i;
        let mut block = [0u8; 64];
        block[..rem].copy_from_slice(&a[i..]);
        let take = core::cmp::min(64 - rem, b.len());
        block[rem..rem + take].copy_from_slice(&b[..take]);
        let filled = rem + take;
        if filled == 64 {
            compress(&mut state, &block);
            let j = absorb(&mut state, b, take);
            finish(&mut state, &b[j..], a.len() + b.len())
        } else {
            finish(&mut state, &block[..filled], a.len() + b.len())
        }
    } else {
        let j = absorb(&mut state, b, 0);
        finish(&mut state, &b[j..], a.len() + b.len())
    }
}

pub fn sha256_hex(data: &[u8]) -> [u8; 64] {
    let d = sha256(data);
    let mut hex = [0u8; 64];
    const DIG: &[u8; 16] = b"0123456789abcdef";
    for i in 0..32 {
        hex[i * 2] = DIG[(d[i] >> 4) as usize];
        hex[i * 2 + 1] = DIG[(d[i] & 0xF) as usize];
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_empty() {
        let h = sha256(b"");
        assert_eq!(
            h,
            [
                0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
                0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
                0x78, 0x52, 0xb8, 0x55
            ]
        );
    }

    #[test]
    fn sha256_abc() {
        let hex = sha256_hex(b"abc");
        assert_eq!(
            core::str::from_utf8(&hex).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_concat_matches() {
        let a = b"abc";
        let b = b"def";
        let mut both = [0u8; 6];
        both[..3].copy_from_slice(a);
        both[3..].copy_from_slice(b);
        assert_eq!(sha256_concat(a, b), sha256(&both));
        assert_eq!(sha256_concat(b"", b"abc"), sha256(b"abc"));
        let long_a = [0xA5u8; 70];
        let long_b = [0x5Au8; 80];
        let mut v = [0u8; 150];
        v[..70].copy_from_slice(&long_a);
        v[70..].copy_from_slice(&long_b);
        assert_eq!(sha256_concat(&long_a, &long_b), sha256(&v));
    }
}
