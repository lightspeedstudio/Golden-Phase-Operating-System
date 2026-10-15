//! Superblock and record encoding.

use super::{
    CKPT_SECTORS, FORMAT_VERSION, HEADER_LEN, MAX_PAYLOAD, REC_MAGIC, SB_MAGIC, WAL_SECTORS,
};
use crate::kernel::crypto::phase_hash::ph_content;
use crate::kernel::hash::StateHash;
use crate::platform::Vec;
use sha2::{Digest, Sha256};

use super::JournalError;

pub fn sha256(bytes: &[u8]) -> StateHash {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

#[derive(Debug, Clone, Copy)]
pub struct Superblock {
    pub generation: u64,
    pub head: u32,
    pub tail: u32,
    pub used_sectors: u32,
    pub live_slot: u8,
    pub next_txn_id: u64,
    pub checkpoint_generation: u64,
    pub fs_root: StateHash,
    pub prev_record_root: StateHash,
}

pub fn encode_superblock(sb: &Superblock) -> [u8; 512] {
    let mut s = [0u8; 512];
    s[0..4].copy_from_slice(&SB_MAGIC);
    s[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    s[6..8].copy_from_slice(&1u16.to_le_bytes());
    s[8..16].copy_from_slice(&sb.generation.to_le_bytes());
    s[16..20].copy_from_slice(&sb.head.to_le_bytes());
    s[20..24].copy_from_slice(&sb.tail.to_le_bytes());
    s[24..28].copy_from_slice(&sb.used_sectors.to_le_bytes());
    s[28] = sb.live_slot;
    s[32..40].copy_from_slice(&sb.next_txn_id.to_le_bytes());
    s[40..48].copy_from_slice(&sb.checkpoint_generation.to_le_bytes());
    s[48..80].copy_from_slice(&sb.fs_root);
    s[80..112].copy_from_slice(&sb.prev_record_root);
    s[112..116].copy_from_slice(&WAL_SECTORS.to_le_bytes());
    s[116..120].copy_from_slice(&CKPT_SECTORS.to_le_bytes());
    let sum = sha256(&s[0..480]);
    s[480..512].copy_from_slice(&sum);
    s
}

pub fn decode_superblock(sec: &[u8; 512]) -> Result<Superblock, JournalError> {
    if sec[0..4] != SB_MAGIC {
        return Err(JournalError::BadMagic);
    }
    let version = u16::from_le_bytes([sec[4], sec[5]]);
    if version != FORMAT_VERSION {
        return Err(JournalError::BadVersion);
    }
    let flags = u16::from_le_bytes([sec[6], sec[7]]);
    if flags & 1 == 0 {
        return Err(JournalError::Unformatted);
    }
    let mut chk = *sec;
    for b in chk[480..512].iter_mut() {
        *b = 0;
    }
    if sha256(&chk[0..480]) != sec[480..512] {
        return Err(JournalError::Checksum);
    }
    let wal = u32::from_le_bytes(sec[112..116].try_into().unwrap());
    let ckpt = u32::from_le_bytes(sec[116..120].try_into().unwrap());
    if wal != WAL_SECTORS || ckpt != CKPT_SECTORS {
        return Err(JournalError::BadVersion);
    }
    let live_slot = sec[28];
    if live_slot > 1 {
        return Err(JournalError::BadVersion);
    }
    let head = u32::from_le_bytes(sec[16..20].try_into().unwrap());
    let tail = u32::from_le_bytes(sec[20..24].try_into().unwrap());
    let used_sectors = u32::from_le_bytes(sec[24..28].try_into().unwrap());
    if head >= WAL_SECTORS || tail >= WAL_SECTORS || used_sectors > WAL_SECTORS {
        return Err(JournalError::BadVersion);
    }
    let mut fs_root = [0u8; 32];
    fs_root.copy_from_slice(&sec[48..80]);
    let mut prev = [0u8; 32];
    prev.copy_from_slice(&sec[80..112]);
    Ok(Superblock {
        generation: u64::from_le_bytes(sec[8..16].try_into().unwrap()),
        head,
        tail,
        used_sectors,
        live_slot,
        next_txn_id: u64::from_le_bytes(sec[32..40].try_into().unwrap()),
        checkpoint_generation: u64::from_le_bytes(sec[40..48].try_into().unwrap()),
        fs_root,
        prev_record_root: prev,
    })
}

#[derive(Debug, Clone)]
pub struct RawRecord {
    pub op_type: u16,
    pub txn_id: u64,
    pub record_seq: u64,
    pub target: u32,
    pub payload: Vec<u8>,
    pub prev_root: StateHash,
}

pub fn encode_record(rec: &RawRecord) -> Result<Vec<u8>, JournalError> {
    if rec.payload.len() > MAX_PAYLOAD as usize {
        return Err(JournalError::PayloadTooBig);
    }
    let mut header = [0u8; HEADER_LEN];
    header[0..4].copy_from_slice(&REC_MAGIC);
    header[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    header[6..8].copy_from_slice(&rec.op_type.to_le_bytes());
    header[8..16].copy_from_slice(&rec.txn_id.to_le_bytes());
    header[16..24].copy_from_slice(&rec.record_seq.to_le_bytes());
    header[24..28].copy_from_slice(&rec.target.to_le_bytes());
    header[28..32].copy_from_slice(&(rec.payload.len() as u32).to_le_bytes());
    let ph = ph_content(&rec.payload);
    header[32..64].copy_from_slice(&ph);
    header[64..96].copy_from_slice(&rec.prev_root);
    let mut hashed = header.to_vec();
    hashed.extend_from_slice(&rec.payload);
    let sum = sha256(&hashed);
    header[96..128].copy_from_slice(&sum);
    let mut out = header.to_vec();
    out.extend_from_slice(&rec.payload);
    let pad = (512 - (out.len() % 512)) % 512;
    out.resize(out.len() + pad, 0);
    Ok(out)
}

pub fn decode_record_header(
    sec: &[u8; 512],
) -> Result<(u16, u64, u64, u32, u32, StateHash, StateHash), JournalError> {
    if sec[0..4] != REC_MAGIC {
        return Err(JournalError::TornRecord);
    }
    let version = u16::from_le_bytes([sec[4], sec[5]]);
    if version != FORMAT_VERSION {
        return Err(JournalError::BadVersion);
    }
    let op_type = u16::from_le_bytes([sec[6], sec[7]]);
    let txn_id = u64::from_le_bytes(sec[8..16].try_into().unwrap());
    let seq = u64::from_le_bytes(sec[16..24].try_into().unwrap());
    let target = u32::from_le_bytes(sec[24..28].try_into().unwrap());
    let payload_len = u32::from_le_bytes(sec[28..32].try_into().unwrap());
    if payload_len > MAX_PAYLOAD {
        return Err(JournalError::PayloadTooBig);
    }
    let mut ph = [0u8; 32];
    ph.copy_from_slice(&sec[32..64]);
    let mut prev = [0u8; 32];
    prev.copy_from_slice(&sec[64..96]);
    Ok((op_type, txn_id, seq, target, payload_len, ph, prev))
}

pub fn record_sectors(payload_len: u32) -> u32 {
    let total = HEADER_LEN + payload_len as usize;
    ((total + 511) / 512) as u32
}

pub fn verify_record(header_sec: &[u8; 512], payload: &[u8]) -> Result<(), JournalError> {
    let mut header = [0u8; HEADER_LEN];
    header.copy_from_slice(&header_sec[..HEADER_LEN]);
    let stored: [u8; 32] = header[96..128].try_into().unwrap();
    for b in header[96..128].iter_mut() {
        *b = 0;
    }
    let mut hashed = header.to_vec();
    hashed.extend_from_slice(payload);
    if sha256(&hashed) != stored {
        return Err(JournalError::Checksum);
    }
    let mut stored_ph = [0u8; 32];
    stored_ph.copy_from_slice(&header_sec[32..64]);
    if ph_content(payload) != stored_ph {
        return Err(JournalError::Checksum);
    }
    Ok(())
}
