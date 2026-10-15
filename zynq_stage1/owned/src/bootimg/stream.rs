// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Host-std Zynq-7000 encrypted partition stream (System 6 packer).
//!
//! Device rlib does not compile this module. Keys are never formatted.

use super::frame::{
    predict_enc_bytes, split_encrypted_blob, unenc_bytes, user_stream_bytes, FrameError,
    NOOP_TRAILER_LEN, NOOP_WORD, STREAM_HEADER_LEN, STREAM_IV_OFF, STREAM_LEN_OFF, STREAM_OVERHEAD,
};
use crate::sha256::{compress, initial_state};

/// Host packer only. Not silicon PASS. Not System 6 closed.
pub const SYS06_PACKER_STATUS: &str = "SYS06_PACKER_ORACLE_IMAGE";

const PLAIN_HEADER_LEN: usize = 140;
const IPAD: usize = 64;
const PAD1: usize = 64;
const GAP: usize = 256;
const OPAD: usize = 64;
const DIGEST: usize = 32;
const PAD2: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamError {
    Empty,
    Unaligned,
    Header,
    Length,
    Trailer,
    Auth,
    Bitstream,
}

#[derive(Clone, Copy)]
pub struct StreamKeys {
    pub aes: [u8; 32],
    pub iv: [u8; 16],
    pub mac: [u8; 32],
}

impl core::fmt::Debug for StreamKeys {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StreamKeys")
    }
}

fn from_hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
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

/// Parse a Bootgen `.nky`. Never formats key bytes.
pub fn keys_from_nky(text: &str) -> Option<StreamKeys> {
    let mut key = None;
    let mut iv = None;
    let mut mac = None;
    for line in text.lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() >= 3 && p[0] == "Key" && p[1] == "0" {
            key = hex_decode(
                &p[2]
                    .chars()
                    .filter(|c| c.is_ascii_hexdigit())
                    .collect::<String>(),
            );
        } else if p.len() >= 3 && p[0] == "Key" && p[1] == "StartCBC" {
            iv = hex_decode(
                &p[2]
                    .chars()
                    .filter(|c| c.is_ascii_hexdigit())
                    .collect::<String>(),
            );
        } else if p.len() >= 3 && p[0] == "Key" && p[1] == "HMAC" {
            let h: String = p[2..]
                .join("")
                .chars()
                .filter(|c| c.is_ascii_hexdigit())
                .collect();
            mac = hex_decode(&h);
        }
    }
    let k = key?;
    let i = iv?;
    let m = mac?;
    if k.len() != 32 || i.len() != 16 || m.len() != 32 {
        return None;
    }
    let mut aes = [0u8; 32];
    let mut ivb = [0u8; 16];
    let mut macb = [0u8; 32];
    aes.copy_from_slice(&k);
    ivb.copy_from_slice(&i);
    macb.copy_from_slice(&m);
    Some(StreamKeys {
        aes,
        iv: ivb,
        mac: macb,
    })
}

fn put_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn load_le(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn sbox_table() -> [u8; 256] {
    let mut s = [0u8; 256];
    s[0] = 0x63;
    let mut p = 1u8;
    let mut q = 1u8;
    loop {
        let hi = p & 0x80;
        p ^= p.wrapping_shl(1);
        if hi != 0 {
            p ^= 0x1b;
        }
        q ^= q.wrapping_shl(1);
        q ^= q.wrapping_shl(2);
        q ^= q.wrapping_shl(4);
        if q & 0x80 != 0 {
            q ^= 0x09;
        }
        let xformed = q
            ^ q.rotate_left(1)
            ^ q.rotate_left(2)
            ^ q.rotate_left(3)
            ^ q.rotate_left(4);
        s[p as usize] = xformed ^ 0x63;
        if p == 1 {
            break;
        }
    }
    s
}

fn sub_word(x: u32, sbox: &[u8; 256]) -> u32 {
    let b = x.to_be_bytes();
    u32::from_be_bytes([
        sbox[b[0] as usize],
        sbox[b[1] as usize],
        sbox[b[2] as usize],
        sbox[b[3] as usize],
    ])
}

fn expand_key(key: &[u8; 32], sbox: &[u8; 256]) -> [u32; 60] {
    let mut w = [0u32; 60];
    for i in 0..8 {
        w[i] = u32::from_be_bytes(key[i * 4..i * 4 + 4].try_into().unwrap());
    }
    let mut rcon = 1u8;
    for i in 8..60 {
        let mut t = w[i - 1];
        if i % 8 == 0 {
            t = sub_word(t.rotate_left(8), sbox) ^ ((rcon as u32) << 24);
            let hi = rcon & 0x80;
            rcon = rcon.wrapping_shl(1);
            if hi != 0 {
                rcon ^= 0x1b;
            }
        } else if i % 8 == 4 {
            t = sub_word(t, sbox);
        }
        w[i] = w[i - 8] ^ t;
    }
    w
}

fn add_round(state: &mut [u8; 16], w: &[u32; 60], round: usize) {
    for c in 0..4 {
        let k = w[round * 4 + c].to_be_bytes();
        for r in 0..4 {
            state[c * 4 + r] ^= k[r];
        }
    }
}

fn sub_bytes(state: &mut [u8; 16], sbox: &[u8; 256]) {
    for b in state.iter_mut() {
        *b = sbox[*b as usize];
    }
}

fn shift_rows(state: &mut [u8; 16]) {
    let t = *state;
    state[1] = t[5];
    state[5] = t[9];
    state[9] = t[13];
    state[13] = t[1];
    state[2] = t[10];
    state[6] = t[14];
    state[10] = t[2];
    state[14] = t[6];
    state[3] = t[15];
    state[7] = t[3];
    state[11] = t[7];
    state[15] = t[11];
}

fn inv_shift_rows(state: &mut [u8; 16]) {
    let t = *state;
    state[1] = t[13];
    state[5] = t[1];
    state[9] = t[5];
    state[13] = t[9];
    state[2] = t[10];
    state[6] = t[14];
    state[10] = t[2];
    state[14] = t[6];
    state[3] = t[7];
    state[7] = t[11];
    state[11] = t[15];
    state[15] = t[3];
}

fn xtime(a: u8) -> u8 {
    let x = a.wrapping_shl(1);
    if a & 0x80 != 0 {
        x ^ 0x1b
    } else {
        x
    }
}

fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        let hi = a & 0x80;
        a = a.wrapping_shl(1);
        if hi != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

fn mix_columns(state: &mut [u8; 16]) {
    for c in 0..4 {
        let i = c * 4;
        let a0 = state[i];
        let a1 = state[i + 1];
        let a2 = state[i + 2];
        let a3 = state[i + 3];
        state[i] = xtime(a0) ^ xtime(a1) ^ a1 ^ a2 ^ a3;
        state[i + 1] = a0 ^ xtime(a1) ^ xtime(a2) ^ a2 ^ a3;
        state[i + 2] = a0 ^ a1 ^ xtime(a2) ^ xtime(a3) ^ a3;
        state[i + 3] = xtime(a0) ^ a0 ^ a1 ^ a2 ^ xtime(a3);
    }
}

fn inv_mix_columns(state: &mut [u8; 16]) {
    for c in 0..4 {
        let i = c * 4;
        let a0 = state[i];
        let a1 = state[i + 1];
        let a2 = state[i + 2];
        let a3 = state[i + 3];
        state[i] = gf_mul(a0, 0x0e) ^ gf_mul(a1, 0x0b) ^ gf_mul(a2, 0x0d) ^ gf_mul(a3, 0x09);
        state[i + 1] = gf_mul(a0, 0x09) ^ gf_mul(a1, 0x0e) ^ gf_mul(a2, 0x0b) ^ gf_mul(a3, 0x0d);
        state[i + 2] = gf_mul(a0, 0x0d) ^ gf_mul(a1, 0x09) ^ gf_mul(a2, 0x0e) ^ gf_mul(a3, 0x0b);
        state[i + 3] = gf_mul(a0, 0x0b) ^ gf_mul(a1, 0x0d) ^ gf_mul(a2, 0x09) ^ gf_mul(a3, 0x0e);
    }
}

fn aes256_encrypt_block(w: &[u32; 60], sbox: &[u8; 256], block: &mut [u8; 16]) {
    add_round(block, w, 0);
    for round in 1..14 {
        sub_bytes(block, sbox);
        shift_rows(block);
        mix_columns(block);
        add_round(block, w, round);
    }
    sub_bytes(block, sbox);
    shift_rows(block);
    add_round(block, w, 14);
}

fn aes256_decrypt_block(w: &[u32; 60], inv: &[u8; 256], block: &mut [u8; 16]) {
    add_round(block, w, 14);
    for round in (1..14).rev() {
        inv_shift_rows(block);
        sub_bytes(block, inv);
        add_round(block, w, round);
        inv_mix_columns(block);
    }
    inv_shift_rows(block);
    sub_bytes(block, inv);
    add_round(block, w, 0);
}

fn words_from_be_block(block: &[u8; 16]) -> [u32; 4] {
    [
        u32::from_be_bytes(block[0..4].try_into().unwrap()),
        u32::from_be_bytes(block[4..8].try_into().unwrap()),
        u32::from_be_bytes(block[8..12].try_into().unwrap()),
        u32::from_be_bytes(block[12..16].try_into().unwrap()),
    ]
}

fn be_block_from_words(words: &[u32; 4]) -> [u8; 16] {
    let mut b = [0u8; 16];
    for i in 0..4 {
        b[i * 4..i * 4 + 4].copy_from_slice(&words[i].to_be_bytes());
    }
    b
}

fn zynq_encrypt(keys: &StreamKeys, plain: &[u8]) -> Result<Vec<u8>, StreamError> {
    if plain.len() % 16 != 0 {
        return Err(StreamError::Unaligned);
    }
    let sbox = sbox_table();
    let w = expand_key(&keys.aes, &sbox);
    let mut out = vec![0u8; plain.len()];
    let mut prev = [0u32; 4];
    let mut first = true;
    let mut i = 0;
    while i < plain.len() {
        let mut words = [0u32; 4];
        for c in 0..4 {
            let le = load_le(plain, i + c * 4);
            if first {
                let ivw = u32::from_be_bytes(keys.iv[c * 4..c * 4 + 4].try_into().unwrap());
                words[c] = (le ^ ivw).reverse_bits();
            } else {
                words[c] = prev[c] ^ le.reverse_bits();
            }
        }
        first = false;
        let mut block = be_block_from_words(&words);
        aes256_encrypt_block(&w, &sbox, &mut block);
        prev = words_from_be_block(&block);
        for c in 0..4 {
            put_u32(&mut out, i + c * 4, prev[c].reverse_bits());
        }
        i += 16;
    }
    Ok(out)
}

fn zynq_decrypt(keys: &StreamKeys, cipher: &[u8]) -> Result<Vec<u8>, StreamError> {
    if cipher.len() % 16 != 0 {
        return Err(StreamError::Unaligned);
    }
    let sbox = sbox_table();
    let mut inv = [0u8; 256];
    for i in 0..256 {
        inv[sbox[i] as usize] = i as u8;
    }
    let w = expand_key(&keys.aes, &sbox);
    let mut out = vec![0u8; cipher.len()];
    let mut prev = [0u32; 4];
    let mut first = true;
    let mut i = 0;
    while i < cipher.len() {
        let mut ct = [0u32; 4];
        for c in 0..4 {
            ct[c] = load_le(cipher, i + c * 4).reverse_bits();
        }
        let mut block = be_block_from_words(&ct);
        aes256_decrypt_block(&w, &inv, &mut block);
        let dec = words_from_be_block(&block);
        for c in 0..4 {
            let pt = if first {
                let ivw = u32::from_be_bytes(keys.iv[c * 4..c * 4 + 4].try_into().unwrap());
                dec[c].reverse_bits() ^ ivw
            } else {
                (prev[c] ^ dec[c]).reverse_bits()
            };
            put_u32(&mut out, i + c * 4, pt);
        }
        first = false;
        prev = ct;
        i += 16;
    }
    Ok(out)
}

fn io_pad(pad_byte: u8, mac: &[u8; 32]) -> [u8; 64] {
    let mut pad = [pad_byte; 64];
    for i in 0..8 {
        let kw = u32::from_be_bytes(mac[i * 4..i * 4 + 4].try_into().unwrap());
        let x = u32::from_le_bytes(pad[i * 4..i * 4 + 4].try_into().unwrap()) ^ kw;
        pad[i * 4..i * 4 + 4].copy_from_slice(&x.to_le_bytes());
    }
    pad
}

fn sha_pad(len: usize, pad_len: usize) -> Vec<u8> {
    let mut p = vec![0u8; pad_len];
    put_u32(&mut p, 0, 0x8000_0000);
    let bits = (len as u64).wrapping_mul(8);
    put_u32(&mut p, pad_len - 8, (bits >> 32) as u32);
    put_u32(&mut p, pad_len - 4, bits as u32);
    p
}

fn compress_le(state: &mut [u32; 8], data: &[u8]) {
    let mut i = 0;
    while i < data.len() {
        let mut block = [0u8; 64];
        for w in 0..16 {
            let v = load_le(data, i + w * 4);
            block[w * 4..w * 4 + 4].copy_from_slice(&v.to_be_bytes());
        }
        compress(state, &block);
        i += 64;
    }
}

fn store_hi_le(state: &[u32; 8]) -> [u8; 32] {
    let mut o = [0u8; 32];
    for i in 0..8 {
        o[i * 4..i * 4 + 4].copy_from_slice(&state[i].to_le_bytes());
    }
    o
}

fn auth_tag(ipad: &[u8], user: &[u8], pad1: &[u8], opad: &[u8], pad2: &[u8]) -> [u8; 32] {
    let mut st = initial_state();
    compress_le(&mut st, ipad);
    compress_le(&mut st, user);
    compress_le(&mut st, pad1);
    let first = store_hi_le(&st);
    let mut inner = [0u8; 64];
    inner[..32].copy_from_slice(&first);
    inner[32..].copy_from_slice(pad2);
    st = initial_state();
    compress_le(&mut st, opad);
    compress_le(&mut st, &inner);
    store_hi_le(&st)
}

fn build_user(payload: &[u8], is_pl: bool) -> Vec<u8> {
    let mut body = payload.to_vec();
    while body.len() % 4 != 0 {
        body.push(0);
    }
    let mut u = Vec::new();
    if !is_pl {
        let mut pre = [0u8; 8];
        put_u32(&mut pre, 0, 0x3001_E000);
        put_u32(
            &mut pre,
            4,
            0x5000_0000 | ((body.len() / 4) as u32 & 0x07FF_FFFF),
        );
        u.extend_from_slice(&pre);
    }
    u.extend_from_slice(&body);
    while u.len() % 64 != 0 {
        let mut n = [0u8; 4];
        put_u32(&mut n, 0, NOOP_WORD);
        u.extend_from_slice(&n);
    }
    u
}

fn plain_header(iv: &[u8; 16], stream_bytes: u32) -> [u8; PLAIN_HEADER_LEN] {
    let mut h = [0u8; PLAIN_HEADER_LEN];
    let words: [u32; 35] = [
        0xFFFF_FFFF, 0xFFFF_FFFF, 0x0000_00BB, 0x1122_0044, 0xFFFF_FFFF, 0xFFFF_FFFF,
        0xAA99_5566, NOOP_WORD, 0x3000_C001, 0x0000_0040, 0x3000_A001, 0x0000_0040, NOOP_WORD,
        NOOP_WORD, NOOP_WORD, NOOP_WORD, NOOP_WORD, NOOP_WORD, NOOP_WORD, NOOP_WORD, NOOP_WORD,
        NOOP_WORD, NOOP_WORD, NOOP_WORD, NOOP_WORD, NOOP_WORD, NOOP_WORD, NOOP_WORD, 0x3001_6004, 0,
        0, 0, 0, 0x3003_4001, stream_bytes / 4,
    ];
    for (i, w) in words.iter().enumerate() {
        put_u32(&mut h, i * 4, *w);
    }
    for i in 0..4 {
        h[STREAM_IV_OFF + i * 4] = iv[i * 4 + 3];
        h[STREAM_IV_OFF + i * 4 + 1] = iv[i * 4 + 2];
        h[STREAM_IV_OFF + i * 4 + 2] = iv[i * 4 + 1];
        h[STREAM_IV_OFF + i * 4 + 3] = iv[i * 4];
    }
    h
}

/// Encrypt one PS or PL partition. `payload` is the unencrypted image bytes.
pub fn encrypt_partition(
    payload: &[u8],
    is_pl: bool,
    keys: &StreamKeys,
) -> Result<Vec<u8>, StreamError> {
    if payload.is_empty() {
        return Err(StreamError::Empty);
    }
    let unenc = unenc_bytes(payload.len());
    let user = build_user(payload, is_pl);
    if user.len() != user_stream_bytes(unenc, is_pl) {
        return Err(StreamError::Unaligned);
    }
    let ipad = io_pad(0x36, &keys.mac);
    let opad = io_pad(0x5C, &keys.mac);
    let pad1 = sha_pad(IPAD + user.len(), PAD1);
    let pad2 = sha_pad(OPAD + DIGEST, PAD2);
    let tag = auth_tag(&ipad, &user, &pad1, &opad, &pad2);
    let mut aes_plain = Vec::with_capacity(IPAD + user.len() + STREAM_OVERHEAD);
    aes_plain.extend_from_slice(&ipad);
    aes_plain.extend_from_slice(&user);
    aes_plain.extend_from_slice(&pad1);
    aes_plain.extend_from_slice(&[0u8; GAP]);
    aes_plain.extend_from_slice(&opad);
    aes_plain.extend_from_slice(&[0u8; DIGEST]);
    aes_plain.extend_from_slice(&pad2);
    aes_plain.extend_from_slice(&tag);
    let cipher = zynq_encrypt(keys, &aes_plain)?;
    let hdr = plain_header(&keys.iv, cipher.len() as u32);
    let mut out = Vec::with_capacity(PLAIN_HEADER_LEN + cipher.len() + NOOP_TRAILER_LEN);
    out.extend_from_slice(&hdr);
    out.extend_from_slice(&cipher);
    for _ in 0..16 {
        let mut n = [0u8; 4];
        put_u32(&mut n, 0, NOOP_WORD);
        out.extend_from_slice(&n);
    }
    if out.len() != predict_enc_bytes(unenc, is_pl) {
        return Err(StreamError::Unaligned);
    }
    Ok(out)
}

pub(crate) fn decrypt_user(
    blob: &[u8],
    is_pl: bool,
    keys: &StreamKeys,
) -> Result<Vec<u8>, StreamError> {
    if blob.len() < PLAIN_HEADER_LEN + NOOP_TRAILER_LEN {
        return Err(StreamError::Unaligned);
    }
    let cipher = &blob[PLAIN_HEADER_LEN..blob.len() - NOOP_TRAILER_LEN];
    let plain = zynq_decrypt(keys, cipher)?;
    if plain.len() < IPAD {
        return Err(StreamError::Unaligned);
    }
    let user = &plain[IPAD..plain.len() - STREAM_OVERHEAD];
    let skip = if is_pl { 0 } else { 8 };
    if user.len() < skip {
        return Err(StreamError::Unaligned);
    }
    Ok(user[skip..].to_vec())
}

fn frame_err(e: FrameError) -> StreamError {
    match e {
        FrameError::BadStreamHeader => StreamError::Header,
        FrameError::BadLength => StreamError::Length,
        FrameError::BadTrailer => StreamError::Trailer,
        _ => StreamError::Unaligned,
    }
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut d = 0u8;
    let mut i = 0;
    while i < a.len() {
        d |= a[i] ^ b[i];
        i += 1;
    }
    d == 0
}

/// Decrypt one encrypted partition, check framing and the software tag, return `unenc` payload bytes.
pub fn open_encrypted_partition(
    blob: &[u8],
    is_pl: bool,
    keys: &StreamKeys,
    unenc: usize,
) -> Result<Vec<u8>, StreamError> {
    if unenc == 0 || unenc % 4 != 0 {
        return Err(StreamError::Unaligned);
    }
    split_encrypted_blob(blob).map_err(frame_err)?;
    if blob.len() != predict_enc_bytes(unenc, is_pl) {
        return Err(StreamError::Length);
    }
    let cipher = &blob[PLAIN_HEADER_LEN..blob.len() - NOOP_TRAILER_LEN];
    let plain = zynq_decrypt(keys, cipher)?;
    let user_n = user_stream_bytes(unenc, is_pl);
    let need = IPAD + user_n + STREAM_OVERHEAD;
    if plain.len() != need {
        return Err(StreamError::Length);
    }
    let ipad = &plain[0..IPAD];
    let user = &plain[IPAD..IPAD + user_n];
    let mut o = IPAD + user_n;
    let pad1 = &plain[o..o + PAD1];
    o += PAD1;
    let gap = &plain[o..o + GAP];
    o += GAP;
    let opad = &plain[o..o + OPAD];
    o += OPAD;
    let digest_gap = &plain[o..o + DIGEST];
    o += DIGEST;
    let pad2 = &plain[o..o + PAD2];
    o += PAD2;
    let tag = &plain[o..o + DIGEST];
    let expect_ipad = io_pad(0x36, &keys.mac);
    let expect_opad = io_pad(0x5C, &keys.mac);
    let expect_pad1 = sha_pad(IPAD + user_n, PAD1);
    let expect_pad2 = sha_pad(OPAD + DIGEST, PAD2);
    let expect_tag = auth_tag(&expect_ipad, user, &expect_pad1, &expect_opad, &expect_pad2);
    if !ct_eq(ipad, &expect_ipad)
        || !ct_eq(opad, &expect_opad)
        || !ct_eq(pad1, &expect_pad1)
        || !ct_eq(pad2, &expect_pad2)
        || !ct_eq(digest_gap, &[0u8; DIGEST])
        || !ct_eq(gap, &[0u8; GAP])
        || !ct_eq(tag, &expect_tag)
    {
        return Err(StreamError::Auth);
    }
    let skip = if is_pl { 0 } else { 8 };
    if user.len() < skip + unenc {
        return Err(StreamError::Unaligned);
    }
    if !is_pl {
        if load_le(user, 0) != 0x3001_E000 {
            return Err(StreamError::Auth);
        }
        if load_le(user, 4) != (0x5000_0000 | ((unenc / 4) as u32 & 0x07FF_FFFF)) {
            return Err(StreamError::Auth);
        }
    }
    let body = &user[skip..];
    let mut i = unenc;
    while i + 4 <= body.len() {
        if load_le(body, i) != NOOP_WORD {
            return Err(StreamError::Auth);
        }
        i += 4;
    }
    Ok(body[..unenc].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::frame::split_encrypted_blob;
    use crate::sha256::sha256;

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
    fn status_is_oracle_p2_not_closed() {
        assert_eq!(SYS06_PACKER_STATUS, "SYS06_PACKER_ORACLE_IMAGE");
        assert_ne!(SYS06_PACKER_STATUS, "SYS06_CLOSED");
    }

    #[test]
    fn fips197_aes256_block() {
        let key: [u8; 32] = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ];
        let mut block: [u8; 16] = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        let sbox = sbox_table();
        assert_eq!(sbox[0], 0x63);
        assert_eq!(sbox[1], 0x7c);
        assert_eq!(sbox[0x53], 0xed);
        let w = expand_key(&key, &sbox);
        aes256_encrypt_block(&w, &sbox, &mut block);
        assert_eq!(
            block,
            [
                0x8e, 0xa2, 0xb7, 0xca, 0x51, 0x67, 0x45, 0xbf, 0xea, 0xfc, 0x49, 0x90, 0x4b, 0x49,
                0x60, 0x89
            ]
        );
        let mut inv = [0u8; 256];
        for i in 0..256 {
            inv[sbox[i] as usize] = i as u8;
        }
        aes256_decrypt_block(&w, &inv, &mut block);
        assert_eq!(
            block,
            [
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff
            ]
        );
    }

    #[test]
    fn encrypt_length_and_trailer_ps_256() {
        let mut p = vec![0u8; 256];
        for i in 0..256 {
            p[i] = i as u8;
        }
        let blob = encrypt_partition(&p, false, &test_keys()).unwrap();
        assert_eq!(blob.len(), predict_enc_bytes(256, false));
        assert_eq!(blob.len(), 1068);
        let f = split_encrypted_blob(&blob).unwrap();
        assert_eq!(f.header_len, STREAM_HEADER_LEN);
        assert_eq!(f.trailer_off, blob.len() - NOOP_TRAILER_LEN);
        let mut i = f.trailer_off;
        while i < blob.len() {
            assert_eq!(load_le(&blob, i), NOOP_WORD);
            i += 4;
        }
        let len_words = load_le(&blob, STREAM_LEN_OFF) as usize;
        assert_eq!(len_words * 4, f.cipher_len + NOOP_TRAILER_LEN);
    }

    #[test]
    fn encrypt_length_pl_and_short_ps() {
        let k = test_keys();
        assert_eq!(encrypt_partition(&[1, 2, 3, 4], true, &k).unwrap().len(), predict_enc_bytes(4, true));
        assert_eq!(
            encrypt_partition(&[0u8; 15], false, &k).unwrap().len(),
            predict_enc_bytes(16, false)
        );
        assert_eq!(
            encrypt_partition(&[0u8; 17], false, &k).unwrap().len(),
            predict_enc_bytes(20, false)
        );
    }

    #[test]
    fn same_length_payloads_share_first_204() {
        let k = test_keys();
        let a = encrypt_partition(&[0u8; 256], false, &k).unwrap();
        let b = encrypt_partition(&[0xA5u8; 256], false, &k).unwrap();
        assert_eq!(first_diff(&a, &b), STREAM_HEADER_LEN);
        assert_eq!(&a[..STREAM_HEADER_LEN], &b[..STREAM_HEADER_LEN]);
    }

    #[test]
    fn decrypt_recovers_user_payload() {
        let k = test_keys();
        let mut p = vec![0u8; 256];
        for i in 0..256 {
            p[i] = i as u8;
        }
        let blob = encrypt_partition(&p, false, &k).unwrap();
        let got = decrypt_user(&blob, false, &k).unwrap();
        assert_eq!(&got[..256], p.as_slice());
        let pl = encrypt_partition(&p, true, &k).unwrap();
        let got_pl = decrypt_user(&pl, true, &k).unwrap();
        assert_eq!(&got_pl[..256], p.as_slice());
    }

    #[test]
    fn debug_does_not_print_key_bytes() {
        let s = format!("{:?}", test_keys());
        assert_eq!(s, "StreamKeys");
        assert!(!s.contains("aes"));
    }

    #[test]
    fn empty_refused() {
        assert_eq!(encrypt_partition(&[], false, &test_keys()), Err(StreamError::Empty));
    }

    #[test]
    fn oracle_p1_byte_identical_when_present() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("build/oracle");
        let boot_p = dir.join("BOOT_SYS06_ORACLE.BIN");
        let kat_p = dir.join("ddr_kat.bin");
        let nky_p = dir.join("sys06_dev.nky");
        if !boot_p.exists() || !kat_p.exists() || !nky_p.exists() {
            return;
        }
        let boot = std::fs::read(&boot_p).unwrap();
        let kat = std::fs::read(&kat_p).unwrap();
        let nky = std::fs::read_to_string(&nky_p).unwrap();
        let keys = keys_from_nky(&nky).expect("nky");
        let packed = encrypt_partition(&kat, false, &keys).unwrap();
        let h = super::super::frame::parse_allow_encrypted(&boot).unwrap();
        let p1 = h.partitions[1];
        let off = p1.data_off;
        let n = (p1.enc_words as usize) * 4;
        assert_eq!(packed.len(), n);
        assert_eq!(&boot[off..off + n], packed.as_slice());
        let _ = sha256(&packed);
    }

    fn elf32_pt_load(elf: &[u8]) -> Vec<u8> {
        let _ = crate::elf32_le_entry(elf).expect("elf");
        let phoff = u32::from_le_bytes(elf[0x1C..0x20].try_into().unwrap()) as usize;
        let phentsize = u16::from_le_bytes(elf[0x2A..0x2C].try_into().unwrap()) as usize;
        let phnum = u16::from_le_bytes(elf[0x2C..0x2E].try_into().unwrap()) as usize;
        let mut loads = Vec::new();
        let mut i = 0;
        while i < phnum {
            let o = phoff + i as usize * phentsize as usize;
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

    fn hex32(h: [u8; 32]) -> String {
        h.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn oracle_p0_byte_identical_fields_and_recover() {
        let owned = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = owned.join("build/oracle");
        let boot_p = dir.join("BOOT_SYS06_ORACLE.BIN");
        let nky_p = dir.join("sys06_dev.nky");
        let elf_p = owned.join("build/sys05_plat.elf");
        if !boot_p.exists() || !nky_p.exists() || !elf_p.exists() {
            return;
        }
        let elf = std::fs::read(&elf_p).unwrap();
        assert_eq!(
            hex32(sha256(&elf)),
            "f40f7e13a8f141afd404c407803f3de394c4e1ae082ca0ca112a932a53b248a2"
        );
        let stage1 = elf32_pt_load(&elf);
        assert_eq!(stage1.len(), 13380);
        let keys = keys_from_nky(&std::fs::read_to_string(&nky_p).unwrap()).expect("nky");
        let packed = encrypt_partition(&stage1, false, &keys).unwrap();
        assert_eq!(packed.len(), 14188);
        let boot = std::fs::read(&boot_p).unwrap();
        assert_eq!(super::super::parse::u32le(&boot, 0x034).unwrap(), 13380);
        assert_eq!(super::super::parse::u32le(&boot, 0x040).unwrap(), 14188);
        let h = super::super::frame::parse_allow_encrypted(&boot).unwrap();
        assert_eq!(h.source_offset, 0x1700);
        assert_eq!(h.stage1_len, 13380);
        let p0 = h.partitions[0];
        assert_eq!(p0.data_off, 0x1700);
        assert_eq!(p0.data_words, 13380 / 4);
        assert_eq!(p0.enc_words, 14188 / 4);
        assert_eq!(p0.total_words, 14188 / 4);
        assert_eq!(h.partitions[1].data_off, 0x4E80);
        let n = (p0.enc_words as usize) * 4;
        assert_eq!(&boot[p0.data_off..p0.data_off + n], packed.as_slice());
        let f = split_encrypted_blob(&packed).unwrap();
        assert_eq!(f.header_len, STREAM_HEADER_LEN);
        assert_eq!(f.trailer_off, packed.len() - NOOP_TRAILER_LEN);
        let rec = open_encrypted_partition(&packed, false, &keys, 13380).unwrap();
        assert_eq!(rec, stage1);
        let rec_oracle = open_encrypted_partition(
            &boot[p0.data_off..p0.data_off + n],
            false,
            &keys,
            13380,
        )
        .unwrap();
        assert_eq!(rec_oracle, stage1);
    }

    #[test]
    fn oracle_p0_rejects_modified_cipher_mac_header_length() {
        let owned = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = owned.join("build/oracle");
        let boot_p = dir.join("BOOT_SYS06_ORACLE.BIN");
        let nky_p = dir.join("sys06_dev.nky");
        let elf_p = owned.join("build/sys05_plat.elf");
        if !boot_p.exists() || !nky_p.exists() || !elf_p.exists() {
            return;
        }
        let keys = keys_from_nky(&std::fs::read_to_string(&nky_p).unwrap()).expect("nky");
        let boot = std::fs::read(&boot_p).unwrap();
        let h = super::super::frame::parse_allow_encrypted(&boot).unwrap();
        let p0 = h.partitions[0];
        let n = (p0.enc_words as usize) * 4;
        let blob = boot[p0.data_off..p0.data_off + n].to_vec();
        let mut cipher = blob.clone();
        cipher[STREAM_HEADER_LEN] ^= 1;
        assert_eq!(
            open_encrypted_partition(&cipher, false, &keys, 13380),
            Err(StreamError::Auth)
        );
        let mut mac = blob.clone();
        let tag_off = mac.len() - NOOP_TRAILER_LEN - 32;
        mac[tag_off] ^= 1;
        assert_eq!(
            open_encrypted_partition(&mac, false, &keys, 13380),
            Err(StreamError::Auth)
        );
        let mut hdr = blob.clone();
        hdr[24] ^= 1;
        assert_eq!(
            open_encrypted_partition(&hdr, false, &keys, 13380),
            Err(StreamError::Header)
        );
        let mut len = blob.clone();
        len[STREAM_LEN_OFF] ^= 1;
        assert_eq!(
            open_encrypted_partition(&len, false, &keys, 13380),
            Err(StreamError::Length)
        );
    }

    #[test]
    fn oracle_p2_path1_g64_byte_identical_and_recover() {
        let owned = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = owned.join("build/oracle");
        let boot_p = dir.join("BOOT_SYS06_ORACLE.BIN");
        let nky_p = dir.join("sys06_dev.nky");
        let bit_p = owned.join("../../fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit");
        if !boot_p.exists() || !nky_p.exists() || !bit_p.exists() {
            return;
        }
        let bit = std::fs::read(&bit_p).unwrap();
        let plain = super::super::bit::strip_encrypted_pl(&bit).unwrap();
        assert_eq!(plain.len(), 4_043_904);
        let keys = keys_from_nky(&std::fs::read_to_string(&nky_p).unwrap()).expect("nky");
        let packed = encrypt_partition(&plain, true, &keys).unwrap();
        assert_eq!(packed.len(), 4_044_652);
        let boot = std::fs::read(&boot_p).unwrap();
        let h = super::super::frame::parse_allow_encrypted(&boot).unwrap();
        let p2 = h.partitions[2];
        assert!(p2.is_pl());
        assert_eq!(p2.data_off, 0x52C0);
        assert_eq!(p2.data_words, 4_043_904 / 4);
        assert_eq!(p2.enc_words, 4_044_652 / 4);
        assert_eq!(h.partitions[3].data_off, 0x3E0A40);
        let n = (p2.enc_words as usize) * 4;
        assert_eq!(&boot[p2.data_off..p2.data_off + n], packed.as_slice());
        let rec = open_encrypted_partition(&packed, true, &keys, 4_043_904).unwrap();
        assert_eq!(rec, plain);
        let rec_oracle = open_encrypted_partition(
            &boot[p2.data_off..p2.data_off + n],
            true,
            &keys,
            4_043_904,
        )
        .unwrap();
        assert_eq!(rec_oracle, plain);
    }

    #[test]
    fn oracle_p2_rejects_modified_cipher_mac_header_length() {
        let owned = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = owned.join("build/oracle");
        let boot_p = dir.join("BOOT_SYS06_ORACLE.BIN");
        let nky_p = dir.join("sys06_dev.nky");
        if !boot_p.exists() || !nky_p.exists() {
            return;
        }
        let keys = keys_from_nky(&std::fs::read_to_string(&nky_p).unwrap()).expect("nky");
        let boot = std::fs::read(&boot_p).unwrap();
        let h = super::super::frame::parse_allow_encrypted(&boot).unwrap();
        let p2 = h.partitions[2];
        let n = (p2.enc_words as usize) * 4;
        let blob = boot[p2.data_off..p2.data_off + n].to_vec();
        let mut cipher = blob.clone();
        cipher[STREAM_HEADER_LEN] ^= 1;
        assert_eq!(
            open_encrypted_partition(&cipher, true, &keys, 4_043_904),
            Err(StreamError::Auth)
        );
        let mut mac = blob.clone();
        let tag_off = mac.len() - NOOP_TRAILER_LEN - 32;
        mac[tag_off] ^= 1;
        assert_eq!(
            open_encrypted_partition(&mac, true, &keys, 4_043_904),
            Err(StreamError::Auth)
        );
        let mut hdr = blob.clone();
        hdr[24] ^= 1;
        assert_eq!(
            open_encrypted_partition(&hdr, true, &keys, 4_043_904),
            Err(StreamError::Header)
        );
        let mut len = blob.clone();
        len[STREAM_LEN_OFF] ^= 1;
        assert_eq!(
            open_encrypted_partition(&len, true, &keys, 4_043_904),
            Err(StreamError::Length)
        );
    }
}
