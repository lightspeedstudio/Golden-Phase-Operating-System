//! Minimal FAT32 reader/**writer** (no alloc) — lets stage-0 read the kernel
//! image and lets the Golden Phase shell persist files to the boot SD card.
//!
//! Supports: MBR partition 0 (or a partitionless "superfloppy" card), FAT32
//! only, 512-byte sectors, 8.3 names in the **root directory**. Long-file-name
//! entries are skipped. Write path: create/overwrite/delete a root-dir file,
//! allocating a FAT cluster chain and mirroring all FAT copies. No sub-dirs,
//! no root-dir extension (assumes a free/reusable root slot exists).

use super::sd::{SdCard, SdError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatError {
    Io(SdError),
    /// No FAT32 filesystem found (bad MBR/BPB signature or not FAT32).
    NoFat32,
    NotFound,
    /// File does not fit in the caller's buffer.
    TooBig,
    /// FAT cluster chain is corrupt (loop or out-of-range entry).
    BadChain,
}

impl From<SdError> for FatError {
    fn from(e: SdError) -> Self {
        FatError::Io(e)
    }
}

/// A root-directory file entry (8.3 name + size), for [`Fat32::list_root`].
#[derive(Debug, Clone, Copy)]
pub struct RootEntry {
    pub name: [u8; 11],
    pub size: u32,
}

impl Default for RootEntry {
    fn default() -> Self {
        RootEntry {
            name: [0x20; 11],
            size: 0,
        }
    }
}

fn u16le(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}

fn u32le(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

pub struct Fat32 {
    /// Sectors per cluster.
    sec_per_clus: u32,
    /// LBA of the first FAT.
    fat_lba: u32,
    /// LBA of cluster 2.
    data_lba: u32,
    /// First cluster of the root directory.
    root_clus: u32,
    /// Total data clusters + 2 (upper bound for valid cluster numbers).
    max_clus: u32,
    /// Number of FAT copies (mirror writes to all).
    num_fats: u32,
    /// Sectors per FAT (stride between FAT copies).
    fat_sz32: u32,
}

impl Fat32 {
    /// Mount the FAT32 volume on partition 0 (falling back to a
    /// partitionless layout when sector 0 is itself a FAT BPB).
    pub fn mount(sd: &mut SdCard) -> Result<Fat32, FatError> {
        let mut sec = [0u8; 512];
        sd.read_block(0, &mut sec)?;
        if sec[510] != 0x55 || sec[511] != 0xAA {
            return Err(FatError::NoFat32);
        }

        // A BPB has a jump opcode and a sane bytes-per-sector; an MBR does not.
        let looks_like_bpb = (sec[0] == 0xEB || sec[0] == 0xE9) && u16le(&sec, 11) == 512;
        let part_lba = if looks_like_bpb {
            0
        } else {
            // MBR partition entry 0 at offset 446: start LBA at +8.
            u32le(&sec, 446 + 8)
        };

        if part_lba != 0 {
            sd.read_block(part_lba, &mut sec)?;
            if sec[510] != 0x55 || sec[511] != 0xAA {
                return Err(FatError::NoFat32);
            }
        }

        let bytes_per_sec = u16le(&sec, 11) as u32;
        let sec_per_clus = sec[13] as u32;
        let reserved = u16le(&sec, 14) as u32;
        let num_fats = sec[16] as u32;
        let root_ent_cnt = u16le(&sec, 17) as u32;
        let fat_sz32 = u32le(&sec, 36);
        let root_clus = u32le(&sec, 44);
        let tot_sec = {
            let t16 = u16le(&sec, 19) as u32;
            if t16 != 0 {
                t16
            } else {
                u32le(&sec, 32)
            }
        };

        // FAT32: 512-byte sectors, zero 16-bit root entries, 32-bit FAT size.
        if bytes_per_sec != 512 || root_ent_cnt != 0 || fat_sz32 == 0 || sec_per_clus == 0 {
            return Err(FatError::NoFat32);
        }

        let fat_lba = part_lba + reserved;
        let data_lba = fat_lba + num_fats * fat_sz32;
        let data_secs = tot_sec.saturating_sub(reserved + num_fats * fat_sz32);
        let max_clus = data_secs / sec_per_clus + 2;

        Ok(Fat32 {
            sec_per_clus,
            fat_lba,
            data_lba,
            root_clus,
            max_clus,
            num_fats,
            fat_sz32,
        })
    }

    fn cluster_lba(&self, clus: u32) -> u32 {
        self.data_lba + (clus - 2) * self.sec_per_clus
    }

    /// Next cluster in the chain, or `None` at end-of-chain.
    fn next_cluster(&self, sd: &mut SdCard, clus: u32) -> Result<Option<u32>, FatError> {
        let mut sec = [0u8; 512];
        let fat_offset = clus * 4;
        sd.read_block(self.fat_lba + fat_offset / 512, &mut sec)?;
        let entry = u32le(&sec, (fat_offset % 512) as usize) & 0x0FFF_FFFF;
        if entry >= 0x0FFF_FFF8 {
            return Ok(None);
        }
        if entry < 2 || entry >= self.max_clus {
            return Err(FatError::BadChain);
        }
        Ok(Some(entry))
    }

    /// Find `name83` (11-byte padded 8.3 form, e.g. `b"KAIROS  BIN"`) in the
    /// root directory. Returns (first cluster, file size in bytes).
    pub fn find_in_root(&self, sd: &mut SdCard, name83: &[u8; 11]) -> Result<(u32, u32), FatError> {
        let mut sec = [0u8; 512];
        let mut clus = self.root_clus;
        let mut hops = 0u32;
        loop {
            for s in 0..self.sec_per_clus {
                sd.read_block(self.cluster_lba(clus) + s, &mut sec)?;
                for e in 0..(512 / 32) {
                    let ent = &sec[e * 32..e * 32 + 32];
                    match ent[0] {
                        0x00 => return Err(FatError::NotFound), // end of directory
                        0xE5 => continue,                       // deleted
                        _ => {}
                    }
                    let attr = ent[11];
                    if attr & 0x0F == 0x0F {
                        continue; // long-file-name entry
                    }
                    if attr & 0x08 != 0 || attr & 0x10 != 0 {
                        continue; // volume label / directory
                    }
                    if &ent[0..11] == name83 {
                        let first = ((u16le(ent, 20) as u32) << 16) | u16le(ent, 26) as u32;
                        let size = u32le(ent, 28);
                        return Ok((first, size));
                    }
                }
            }
            match self.next_cluster(sd, clus)? {
                Some(next) => clus = next,
                None => return Err(FatError::NotFound),
            }
            hops += 1;
            if hops > self.max_clus {
                return Err(FatError::BadChain);
            }
        }
    }

    /// Load a root-directory file into `dest`. Returns the file size.
    ///
    /// # Safety
    /// `dest` must be valid for writes of at least `max_len` bytes (stage-0
    /// passes the DDR load window).
    pub unsafe fn load_root_file(
        &self,
        sd: &mut SdCard,
        name83: &[u8; 11],
        dest: *mut u8,
        max_len: usize,
    ) -> Result<usize, FatError> {
        let (first, size) = self.find_in_root(sd, name83)?;
        let size = size as usize;
        if size > max_len {
            return Err(FatError::TooBig);
        }

        let mut sec = [0u8; 512];
        let mut clus = first;
        let mut written = 0usize;
        let mut hops = 0u32;
        while written < size {
            for s in 0..self.sec_per_clus {
                if written >= size {
                    break;
                }
                sd.read_block(self.cluster_lba(clus) + s, &mut sec)?;
                let take = core::cmp::min(512, size - written);
                core::ptr::copy_nonoverlapping(sec.as_ptr(), dest.add(written), take);
                written += take;
            }
            if written < size {
                match self.next_cluster(sd, clus)? {
                    Some(next) => clus = next,
                    None => return Err(FatError::BadChain),
                }
                hops += 1;
                if hops > self.max_clus {
                    return Err(FatError::BadChain);
                }
            }
        }
        Ok(size)
    }

    // ── Write path ─────────────────────────────────────────────────────────

    /// Write a FAT entry for `clus` to all FAT copies (preserving the reserved
    /// top 4 bits). `value` is a 28-bit next-cluster / EOC (0x0FFFFFFF) / 0 (free).
    fn set_fat_entry(&self, sd: &mut SdCard, clus: u32, value: u32) -> Result<(), FatError> {
        let fat_offset = clus * 4;
        let sec_in_fat = fat_offset / 512;
        let idx = (fat_offset % 512) as usize;
        for f in 0..self.num_fats {
            let lba = self.fat_lba + f * self.fat_sz32 + sec_in_fat;
            let mut sec = [0u8; 512];
            sd.read_block(lba, &mut sec)?;
            let old = u32le(&sec, idx);
            let newv = (old & 0xF000_0000) | (value & 0x0FFF_FFFF);
            sec[idx..idx + 4].copy_from_slice(&newv.to_le_bytes());
            sd.write_block(lba, &sec)?;
        }
        Ok(())
    }

    /// Scan the FAT for a free cluster, mark it end-of-chain, and return it.
    fn alloc_cluster(&self, sd: &mut SdCard) -> Result<u32, FatError> {
        let mut sec = [0u8; 512];
        let mut cur_lba = u32::MAX;
        for clus in 2..self.max_clus {
            let fat_offset = clus * 4;
            let lba = self.fat_lba + fat_offset / 512;
            if lba != cur_lba {
                sd.read_block(lba, &mut sec)?;
                cur_lba = lba;
            }
            let idx = (fat_offset % 512) as usize;
            if (u32le(&sec, idx) & 0x0FFF_FFFF) == 0 {
                self.set_fat_entry(sd, clus, 0x0FFF_FFFF)?;
                return Ok(clus);
            }
        }
        Err(FatError::TooBig) // volume full
    }

    /// Free a cluster chain starting at `first`.
    fn free_chain(&self, sd: &mut SdCard, first: u32) -> Result<(), FatError> {
        if first < 2 || first >= self.max_clus {
            return Ok(());
        }
        let mut clus = first;
        let mut hops = 0u32;
        loop {
            let next = self.next_cluster(sd, clus)?;
            self.set_fat_entry(sd, clus, 0)?;
            match next {
                Some(n) => clus = n,
                None => break,
            }
            hops += 1;
            if hops > self.max_clus {
                return Err(FatError::BadChain);
            }
        }
        Ok(())
    }

    /// Find the root-dir slot for `name83`: returns (sector LBA, byte offset,
    /// existing first-cluster if the file already exists else None). Reuses a
    /// deleted/free slot for a new file.
    fn find_or_free_root_slot(
        &self,
        sd: &mut SdCard,
        name83: &[u8; 11],
    ) -> Result<(u32, usize, Option<u32>), FatError> {
        let mut sec = [0u8; 512];
        let mut clus = self.root_clus;
        let mut free_slot: Option<(u32, usize)> = None;
        let mut hops = 0u32;
        loop {
            for s in 0..self.sec_per_clus {
                let lba = self.cluster_lba(clus) + s;
                sd.read_block(lba, &mut sec)?;
                for e in 0..(512 / 32) {
                    let off = e * 32;
                    match sec[off] {
                        0x00 => {
                            let slot = free_slot.unwrap_or((lba, off));
                            return Ok((slot.0, slot.1, None));
                        }
                        0xE5 => {
                            if free_slot.is_none() {
                                free_slot = Some((lba, off));
                            }
                            continue;
                        }
                        _ => {}
                    }
                    if sec[off + 11] & 0x0F == 0x0F {
                        continue; // LFN
                    }
                    if &sec[off..off + 11] == name83 {
                        let first =
                            ((u16le(&sec, off + 20) as u32) << 16) | u16le(&sec, off + 26) as u32;
                        return Ok((lba, off, Some(first)));
                    }
                }
            }
            match self.next_cluster(sd, clus)? {
                Some(n) => clus = n,
                None => {
                    return free_slot.map(|s| (s.0, s.1, None)).ok_or(FatError::TooBig);
                    // root dir full (no extend)
                }
            }
            hops += 1;
            if hops > self.max_clus {
                return Err(FatError::BadChain);
            }
        }
    }

    /// Create or overwrite a root-directory file `name83` with `data`.
    pub fn write_root_file(
        &self,
        sd: &mut SdCard,
        name83: &[u8; 11],
        data: &[u8],
    ) -> Result<(), FatError> {
        let (slot_lba, slot_off, old_first) = self.find_or_free_root_slot(sd, name83)?;
        if let Some(first) = old_first {
            self.free_chain(sd, first)?;
        }

        // Allocate + write the cluster chain.
        let mut first_clus = 0u32;
        let mut prev_clus = 0u32;
        let mut off = 0usize;
        while off < data.len() {
            let clus = self.alloc_cluster(sd)?;
            if first_clus == 0 {
                first_clus = clus;
            } else {
                self.set_fat_entry(sd, prev_clus, clus)?;
            }
            for s in 0..self.sec_per_clus {
                if off >= data.len() {
                    break;
                }
                let take = core::cmp::min(512, data.len() - off);
                let mut blk = [0u8; 512];
                blk[..take].copy_from_slice(&data[off..off + take]);
                sd.write_block(self.cluster_lba(clus) + s, &blk)?;
                off += take;
            }
            prev_clus = clus;
        }

        // Write the directory entry.
        let mut sec = [0u8; 512];
        sd.read_block(slot_lba, &mut sec)?;
        let e = slot_off;
        for b in sec[e..e + 32].iter_mut() {
            *b = 0;
        }
        sec[e..e + 11].copy_from_slice(name83);
        sec[e + 11] = 0x20; // archive
        sec[e + 20..e + 22].copy_from_slice(&((first_clus >> 16) as u16).to_le_bytes());
        sec[e + 26..e + 28].copy_from_slice(&(first_clus as u16).to_le_bytes());
        sec[e + 28..e + 32].copy_from_slice(&(data.len() as u32).to_le_bytes());
        sd.write_block(slot_lba, &sec)?;
        Ok(())
    }

    /// Delete a root-directory file: free its chain + mark the entry deleted.
    pub fn delete_root_file(&self, sd: &mut SdCard, name83: &[u8; 11]) -> Result<(), FatError> {
        let (slot_lba, slot_off, old_first) = self.find_or_free_root_slot(sd, name83)?;
        let first = old_first.ok_or(FatError::NotFound)?;
        self.free_chain(sd, first)?;
        let mut sec = [0u8; 512];
        sd.read_block(slot_lba, &mut sec)?;
        sec[slot_off] = 0xE5;
        sd.write_block(slot_lba, &sec)?;
        Ok(())
    }

    /// List root-directory files into `out` (up to `out.len()` entries).
    /// Returns the count written.
    pub fn list_root(&self, sd: &mut SdCard, out: &mut [RootEntry]) -> Result<usize, FatError> {
        let mut sec = [0u8; 512];
        let mut clus = self.root_clus;
        let mut n = 0usize;
        let mut hops = 0u32;
        'outer: loop {
            for s in 0..self.sec_per_clus {
                sd.read_block(self.cluster_lba(clus) + s, &mut sec)?;
                for e in 0..(512 / 32) {
                    let off = e * 32;
                    match sec[off] {
                        0x00 => break 'outer,
                        0xE5 => continue,
                        _ => {}
                    }
                    let attr = sec[off + 11];
                    if attr & 0x0F == 0x0F || attr & 0x08 != 0 || attr & 0x10 != 0 {
                        continue; // LFN / volume label / directory
                    }
                    if n >= out.len() {
                        break 'outer;
                    }
                    let mut name = [0u8; 11];
                    name.copy_from_slice(&sec[off..off + 11]);
                    out[n] = RootEntry {
                        name,
                        size: u32le(&sec, off + 28),
                    };
                    n += 1;
                }
            }
            match self.next_cluster(sd, clus)? {
                Some(c) => clus = c,
                None => break,
            }
            hops += 1;
            if hops > self.max_clus {
                return Err(FatError::BadChain);
            }
        }
        Ok(n)
    }

    /// Read a root-directory file into `buf`; returns the byte count. Safe
    /// slice version of [`load_root_file`] for the shell.
    pub fn read_root_file(
        &self,
        sd: &mut SdCard,
        name83: &[u8; 11],
        buf: &mut [u8],
    ) -> Result<usize, FatError> {
        let (first, size) = self.find_in_root(sd, name83)?;
        let size = size as usize;
        if size > buf.len() {
            return Err(FatError::TooBig);
        }
        let mut sec = [0u8; 512];
        let mut clus = first;
        let mut written = 0usize;
        let mut hops = 0u32;
        while written < size {
            for s in 0..self.sec_per_clus {
                if written >= size {
                    break;
                }
                sd.read_block(self.cluster_lba(clus) + s, &mut sec)?;
                let take = core::cmp::min(512, size - written);
                buf[written..written + take].copy_from_slice(&sec[..take]);
                written += take;
            }
            if written < size {
                match self.next_cluster(sd, clus)? {
                    Some(n) => clus = n,
                    None => return Err(FatError::BadChain),
                }
                hops += 1;
                if hops > self.max_clus {
                    return Err(FatError::BadChain);
                }
            }
        }
        Ok(size)
    }
}

/// One MBR partition table entry (LBA addressing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MbrPartition {
    pub boot: u8,
    pub ptype: u8,
    pub start_lba: u32,
    pub sectors: u32,
}

/// Read the four MBR partition entries. Does not write the table.
#[cfg(feature = "phasefs-journal")]
pub fn read_mbr_partitions(sd: &mut SdCard) -> Result<[Option<MbrPartition>; 4], FatError> {
    let mut sec = [0u8; 512];
    sd.read_block(0, &mut sec)?;
    if sec[510] != 0x55 || sec[511] != 0xAA {
        return Err(FatError::NoFat32);
    }
    let looks_like_bpb = (sec[0] == 0xEB || sec[0] == 0xE9) && u16le(&sec, 11) == 512;
    if looks_like_bpb {
        return Ok([None, None, None, None]);
    }
    let mut out = [None; 4];
    for i in 0..4 {
        let e = 446 + i * 16;
        let ptype = sec[e + 4];
        let start = u32le(&sec, e + 8);
        let sectors = u32le(&sec, e + 12);
        if ptype != 0 && sectors != 0 {
            out[i] = Some(MbrPartition {
                boot: sec[e],
                ptype,
                start_lba: start,
                sectors,
            });
        }
    }
    Ok(out)
}
