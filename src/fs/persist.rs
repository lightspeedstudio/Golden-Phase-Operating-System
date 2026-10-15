//! Bounded PhaseFS checkpoint image (little-endian).
//!
//! Fits in one 1 MiB checkpoint slot under the v1 inode/file bounds.

use crate::core::gphase_core::PhaseState;
use crate::fs::block::{BlockId, PhaseBlock};
use crate::fs::directory::DirectoryEntry;
use crate::fs::inode::{Inode, InodeId, InodeKind};
use crate::fs::journal::{CKPT_MAGIC, FORMAT_VERSION, MAX_FILE_BYTES, MAX_INODES, MAX_NAME};
use crate::fs::phasefs::{FsError, PhaseFs};
use crate::kernel::crypto::phase_hash::{ph_content, ph_root};
use crate::kernel::hash::StateHash;
use crate::platform::{HashMap, ToString, Vec};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistError {
    TooManyInodes,
    NameTooLong,
    FileTooBig,
    Truncated,
    BadMagic,
    BadVersion,
    Checksum,
    Fs(FsError),
}

impl From<FsError> for PersistError {
    fn from(value: FsError) -> Self {
        PersistError::Fs(value)
    }
}

fn sha256(bytes: &[u8]) -> StateHash {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

fn push_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn push_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn push_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn read_u16(data: &[u8], off: &mut usize) -> Result<u16, PersistError> {
    let s = *off;
    let e = s.checked_add(2).ok_or(PersistError::Truncated)?;
    let slice = data.get(s..e).ok_or(PersistError::Truncated)?;
    *off = e;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(data: &[u8], off: &mut usize) -> Result<u32, PersistError> {
    let s = *off;
    let e = s.checked_add(4).ok_or(PersistError::Truncated)?;
    let slice = data.get(s..e).ok_or(PersistError::Truncated)?;
    *off = e;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(data: &[u8], off: &mut usize) -> Result<u64, PersistError> {
    let s = *off;
    let e = s.checked_add(8).ok_or(PersistError::Truncated)?;
    let slice = data.get(s..e).ok_or(PersistError::Truncated)?;
    *off = e;
    let mut a = [0u8; 8];
    a.copy_from_slice(slice);
    Ok(u64::from_le_bytes(a))
}

fn read_bytes<'a>(data: &'a [u8], off: &mut usize, n: usize) -> Result<&'a [u8], PersistError> {
    let s = *off;
    let e = s.checked_add(n).ok_or(PersistError::Truncated)?;
    let slice = data.get(s..e).ok_or(PersistError::Truncated)?;
    *off = e;
    Ok(slice)
}

fn inode_leaf(inode: &Inode, data: &[u8]) -> StateHash {
    let parent = inode.parent.unwrap_or(u32::MAX);
    let mut buf = Vec::new();
    push_u32(&mut buf, inode.id);
    buf.push(inode.kind as u8);
    buf.push(inode.tag.as_u8());
    buf.push(inode.kairos_sector);
    buf.push(inode.name.len() as u8);
    buf.extend_from_slice(inode.name.as_bytes());
    push_u32(&mut buf, parent);
    push_u64(&mut buf, inode.chronos);
    push_u32(&mut buf, data.len() as u32);
    buf.extend_from_slice(data);
    ph_content(&buf)
}

/// PH-R over live inodes, sorted by id.
pub fn content_root(fs: &PhaseFs) -> StateHash {
    let mut ids: Vec<InodeId> = fs.iter_inodes().map(|i| i.id).collect();
    ids.sort_unstable();
    let mut leaves = Vec::new();
    for id in ids {
        let inode = fs.inode(id).expect("id from iter");
        let data = fs.primary_block_data(id).unwrap_or_default();
        leaves.push(inode_leaf(inode, &data));
    }
    ph_root(&leaves)
}

pub fn encode(fs: &PhaseFs) -> Result<Vec<u8>, PersistError> {
    let mut inodes: Vec<&Inode> = fs.iter_inodes().collect();
    inodes.sort_by_key(|i| i.id);
    if inodes.len() > MAX_INODES as usize {
        return Err(PersistError::TooManyInodes);
    }

    let mut body = Vec::new();
    for inode in &inodes {
        if inode.name.len() > MAX_NAME {
            return Err(PersistError::NameTooLong);
        }
        let data = fs.primary_block_data(inode.id).unwrap_or_default();
        if data.len() > MAX_FILE_BYTES {
            return Err(PersistError::FileTooBig);
        }
        push_u32(&mut body, inode.id);
        body.push(match inode.kind {
            InodeKind::File => 0,
            InodeKind::Directory => 1,
        });
        body.push(inode.tag.as_u8());
        body.push(inode.kairos_sector);
        body.push(inode.name.len() as u8);
        body.extend_from_slice(inode.name.as_bytes());
        push_u32(&mut body, inode.parent.unwrap_or(u32::MAX));
        push_u64(&mut body, inode.chronos);
        let nblocks = inode.blocks.len() as u16;
        push_u16(&mut body, nblocks);
        for b in &inode.blocks {
            push_u32(&mut body, *b);
        }
        push_u32(&mut body, data.len() as u32);
        body.extend_from_slice(&data);
        let owner = fs.primary_block_owner(inode.id).unwrap_or_default();
        if owner.len() > MAX_NAME {
            return Err(PersistError::NameTooLong);
        }
        push_u16(&mut body, owner.len() as u16);
        body.extend_from_slice(owner.as_bytes());
    }

    let mut groups: Vec<(InodeId, Vec<DirectoryEntry>)> = Vec::new();
    for inode in &inodes {
        if inode.kind != InodeKind::Directory {
            continue;
        }
        if let Some(ents) = fs.dir_entries(inode.id) {
            let mut e = ents.to_vec();
            e.sort_by(|a, b| a.name.cmp(&b.name));
            groups.push((inode.id, e));
        }
    }
    for (dir_id, ents) in &groups {
        push_u32(&mut body, *dir_id);
        push_u16(&mut body, ents.len() as u16);
        for ent in ents {
            if ent.name.len() > MAX_NAME {
                return Err(PersistError::NameTooLong);
            }
            body.push(ent.name.len() as u8);
            body.extend_from_slice(ent.name.as_bytes());
            push_u32(&mut body, ent.inode);
        }
    }

    let root = content_root(fs);
    let mut header = vec![0u8; 96];
    header[0..4].copy_from_slice(&CKPT_MAGIC);
    header[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    header[8..12].copy_from_slice(&fs.next_inode_id().to_le_bytes());
    header[12..16].copy_from_slice(&fs.next_block_id().to_le_bytes());
    header[16..20].copy_from_slice(&fs.root_id().to_le_bytes());
    header[20..24].copy_from_slice(&(inodes.len() as u32).to_le_bytes());
    header[24..28].copy_from_slice(&(groups.len() as u32).to_le_bytes());
    header[28..32].copy_from_slice(&(body.len() as u32).to_le_bytes());
    header[32..64].copy_from_slice(&root);

    let mut hashed = header.clone();
    hashed.extend_from_slice(&body);
    let sum = sha256(&hashed);
    header[64..96].copy_from_slice(&sum);

    let mut out = header;
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<PhaseFs, PersistError> {
    if bytes.len() < 96 {
        return Err(PersistError::Truncated);
    }
    if bytes[0..4] != CKPT_MAGIC {
        return Err(PersistError::BadMagic);
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != FORMAT_VERSION {
        return Err(PersistError::BadVersion);
    }
    let next_inode = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let next_block = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let root_id = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
    let inode_count = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
    let group_count = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
    let body_len = u32::from_le_bytes(bytes[28..32].try_into().unwrap()) as usize;
    if bytes.len() < 96 + body_len {
        return Err(PersistError::Truncated);
    }
    let mut chk = bytes[..96 + body_len].to_vec();
    for b in chk[64..96].iter_mut() {
        *b = 0;
    }
    let mut stored = [0u8; 32];
    stored.copy_from_slice(&bytes[64..96]);
    if sha256(&chk) != stored {
        return Err(PersistError::Checksum);
    }
    let body = &bytes[96..96 + body_len];
    let mut off = 0usize;

    let mut inodes: HashMap<InodeId, Inode> = HashMap::new();
    let mut blocks: HashMap<BlockId, PhaseBlock> = HashMap::new();

    for _ in 0..inode_count {
        let id = read_u32(body, &mut off)?;
        let kind_b = *read_bytes(body, &mut off, 1)?.first().ok_or(PersistError::Truncated)?;
        let tag_b = *read_bytes(body, &mut off, 1)?.first().ok_or(PersistError::Truncated)?;
        let sector = *read_bytes(body, &mut off, 1)?.first().ok_or(PersistError::Truncated)?;
        let name_len = *read_bytes(body, &mut off, 1)?.first().ok_or(PersistError::Truncated)? as usize;
        let name_b = read_bytes(body, &mut off, name_len)?;
        let name = core::str::from_utf8(name_b)
            .map_err(|_| PersistError::Truncated)?
            .to_string();
        let parent_raw = read_u32(body, &mut off)?;
        let chronos = read_u64(body, &mut off)?;
        let nblocks = read_u16(body, &mut off)? as usize;
        let mut block_ids = Vec::new();
        for _ in 0..nblocks {
            block_ids.push(read_u32(body, &mut off)?);
        }
        let data_len = read_u32(body, &mut off)? as usize;
        let data = read_bytes(body, &mut off, data_len)?.to_vec();
        let owner_len = read_u16(body, &mut off)? as usize;
        let owner_b = read_bytes(body, &mut off, owner_len)?;
        let owner = core::str::from_utf8(owner_b)
            .map_err(|_| PersistError::Truncated)?
            .to_string();

        let kind = if kind_b == 1 {
            InodeKind::Directory
        } else {
            InodeKind::File
        };
        let tag = PhaseState::from_u8(tag_b).ok_or(PersistError::Truncated)?;
        let parent = if parent_raw == u32::MAX {
            None
        } else {
            Some(parent_raw)
        };
        let inode = Inode {
            id,
            name,
            kind,
            tag,
            blocks: block_ids.clone(),
            parent,
            chronos,
            kairos_sector: sector,
        };
        inodes.insert(id, inode);

        if let Some(&bid) = block_ids.first() {
            let owner_path = if owner.is_empty() {
                None
            } else {
                Some(owner)
            };
            blocks.insert(
                bid,
                PhaseBlock {
                    id: bid,
                    tag,
                    data,
                    owner_path,
                },
            );
        }
    }

    let mut entries: HashMap<InodeId, Vec<DirectoryEntry>> = HashMap::new();
    for _ in 0..group_count {
        let dir_id = read_u32(body, &mut off)?;
        let n = read_u16(body, &mut off)? as usize;
        let mut list = Vec::new();
        for _ in 0..n {
            let name_len = *read_bytes(body, &mut off, 1)?.first().ok_or(PersistError::Truncated)? as usize;
            let name_b = read_bytes(body, &mut off, name_len)?;
            let name = core::str::from_utf8(name_b)
                .map_err(|_| PersistError::Truncated)?
                .to_string();
            let child = read_u32(body, &mut off)?;
            list.push(DirectoryEntry { name, inode: child });
        }
        entries.insert(dir_id, list);
    }

    Ok(PhaseFs::from_parts(
        blocks, inodes, entries, root_id, next_inode, next_block,
    ))
}
