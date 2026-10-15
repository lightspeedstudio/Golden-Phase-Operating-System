// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Read-only FAT16/FAT32 root `BOOT.BIN` lookup and cluster-run walk.

use super::{LbaRun, SdBus, SdCard, SdError, MAX_ADMA_DESC};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatError {
    Io(SdError),
    NoFat,
    NotFound,
    TooSmall,
    BadChain,
    Fragmented,
}

impl From<SdError> for FatError {
    fn from(e: SdError) -> Self {
        FatError::Io(e)
    }
}

fn u16le(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}
fn u32le(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

pub const BOOT_NAME83: [u8; 11] = *b"BOOT    BIN";

pub struct FatVol {
    pub part_lba: u32,
    pub sec_per_clus: u32,
    pub fat_lba: u32,
    pub data_lba: u32,
    pub root_clus: u32,
    pub root_ent_cnt: u32,
    pub fat16: bool,
    pub fat_sz: u32,
    pub max_clus: u32,
}

fn read_sec<B: SdBus>(
    card: &mut SdCard,
    bus: &mut B,
    lba: u32,
    table_phys: u32,
    scratch_phys: u32,
    out: &mut [u8; 512],
) -> Result<(), FatError> {
    card.read_blocks_adma(bus, lba, scratch_phys, 512, table_phys)?;
    bus.phys_read(scratch_phys, out);
    Ok(())
}

impl FatVol {
    pub fn mount<B: SdBus>(
        card: &mut SdCard,
        bus: &mut B,
        table_phys: u32,
        scratch_phys: u32,
    ) -> Result<FatVol, FatError> {
        let mut sec = [0u8; 512];
        read_sec(card, bus, 0, table_phys, scratch_phys, &mut sec)?;
        if sec[510] != 0x55 || sec[511] != 0xAA {
            return Err(FatError::NoFat);
        }
        let looks_bpb = (sec[0] == 0xEB || sec[0] == 0xE9) && u16le(&sec, 11) == 512;
        let part_lba = if looks_bpb {
            0
        } else {
            u32le(&sec, 446 + 8)
        };
        if part_lba != 0 {
            read_sec(card, bus, part_lba, table_phys, scratch_phys, &mut sec)?;
            if sec[510] != 0x55 || sec[511] != 0xAA {
                return Err(FatError::NoFat);
            }
        }
        let bytes_per_sec = u16le(&sec, 11) as u32;
        let sec_per_clus = sec[13] as u32;
        let reserved = u16le(&sec, 14) as u32;
        let num_fats = sec[16] as u32;
        let root_ent_cnt = u16le(&sec, 17) as u32;
        let fat_sz16 = u16le(&sec, 22) as u32;
        let fat_sz32 = u32le(&sec, 36);
        let tot_sec = {
            let t16 = u16le(&sec, 19) as u32;
            if t16 != 0 {
                t16
            } else {
                u32le(&sec, 32)
            }
        };
        if bytes_per_sec != 512 || sec_per_clus == 0 || num_fats == 0 {
            return Err(FatError::NoFat);
        }
        let fat16 = root_ent_cnt != 0;
        let fat_sz = if fat16 { fat_sz16 } else { fat_sz32 };
        if fat_sz == 0 {
            return Err(FatError::NoFat);
        }
        let fat_lba = part_lba + reserved;
        let root_secs = (root_ent_cnt * 32 + 511) / 512;
        let data_lba = fat_lba + num_fats * fat_sz + root_secs;
        let root_clus = if fat16 { 0 } else { u32le(&sec, 44) };
        if !fat16 && root_clus < 2 {
            return Err(FatError::NoFat);
        }
        let data_secs = tot_sec.saturating_sub(reserved + num_fats * fat_sz + root_secs);
        let max_clus = data_secs / sec_per_clus + 2;
        Ok(FatVol {
            part_lba,
            sec_per_clus,
            fat_lba,
            data_lba,
            root_clus,
            root_ent_cnt,
            fat16,
            fat_sz,
            max_clus,
        })
    }

    fn cluster_lba(&self, clus: u32) -> u32 {
        if self.fat16 && clus == 0 {
            self.fat_lba + self.fat_sz
        } else {
            self.data_lba + (clus - 2) * self.sec_per_clus
        }
    }

    fn fat_next<B: SdBus>(
        &self,
        card: &mut SdCard,
        bus: &mut B,
        clus: u32,
        table_phys: u32,
        scratch_phys: u32,
    ) -> Result<Option<u32>, FatError> {
        let mut sec = [0u8; 512];
        if self.fat16 {
            let off = clus * 2;
            read_sec(
                card,
                bus,
                self.fat_lba + off / 512,
                table_phys,
                scratch_phys,
                &mut sec,
            )?;
            let entry = u16le(&sec, (off % 512) as usize) as u32;
            if entry >= 0xFFF8 {
                return Ok(None);
            }
            if entry < 2 {
                return Err(FatError::BadChain);
            }
            return Ok(Some(entry));
        }
        let off = clus * 4;
        read_sec(
            card,
            bus,
            self.fat_lba + off / 512,
            table_phys,
            scratch_phys,
            &mut sec,
        )?;
        let entry = u32le(&sec, (off % 512) as usize) & 0x0FFF_FFFF;
        if entry >= 0x0FFF_FFF8 {
            return Ok(None);
        }
        if entry < 2 || entry >= self.max_clus {
            return Err(FatError::BadChain);
        }
        Ok(Some(entry))
    }

    pub fn find_boot_bin<B: SdBus>(
        &self,
        card: &mut SdCard,
        bus: &mut B,
        table_phys: u32,
        scratch_phys: u32,
    ) -> Result<(u32, u32), FatError> {
        let mut sec = [0u8; 512];
        if self.fat16 {
            let root_lba = self.fat_lba + self.fat_sz;
            let root_secs = (self.root_ent_cnt * 32 + 511) / 512;
            for s in 0..root_secs {
                read_sec(card, bus, root_lba + s, table_phys, scratch_phys, &mut sec)?;
                if let Some(x) = scan_dir_sec(&sec) {
                    return Ok(x);
                }
            }
            return Err(FatError::NotFound);
        }
        let mut clus = self.root_clus;
        let mut hops = 0u32;
        loop {
            for s in 0..self.sec_per_clus {
                read_sec(
                    card,
                    bus,
                    self.cluster_lba(clus) + s,
                    table_phys,
                    scratch_phys,
                    &mut sec,
                )?;
                if let Some(x) = scan_dir_sec(&sec) {
                    return Ok(x);
                }
            }
            match self.fat_next(card, bus, clus, table_phys, scratch_phys)? {
                Some(n) => clus = n,
                None => return Err(FatError::NotFound),
            }
            hops += 1;
            if hops > self.max_clus {
                return Err(FatError::BadChain);
            }
        }
    }

    pub fn file_runs<B: SdBus>(
        &self,
        card: &mut SdCard,
        bus: &mut B,
        first: u32,
        nbytes: u32,
        table_phys: u32,
        scratch_phys: u32,
        out: &mut [LbaRun],
    ) -> Result<usize, FatError> {
        let need_blocks = (nbytes + 511) / 512;
        let mut clus = first;
        let mut blocks_left = need_blocks;
        let mut nrun = 0usize;
        let mut hops = 0u32;
        while blocks_left > 0 {
            let lba = self.cluster_lba(clus);
            let take = core::cmp::min(self.sec_per_clus, blocks_left);
            if nrun > 0 {
                let prev = &mut out[nrun - 1];
                if prev.lba + prev.blocks == lba {
                    prev.blocks += take;
                } else {
                    if nrun >= out.len() || nrun >= MAX_ADMA_DESC {
                        return Err(FatError::Fragmented);
                    }
                    out[nrun] = LbaRun { lba, blocks: take };
                    nrun += 1;
                }
            } else {
                out[0] = LbaRun { lba, blocks: take };
                nrun = 1;
            }
            blocks_left -= take;
            if blocks_left > 0 {
                match self.fat_next(card, bus, clus, table_phys, scratch_phys)? {
                    Some(n) => clus = n,
                    None => return Err(FatError::BadChain),
                }
            }
            hops += 1;
            if hops > self.max_clus {
                return Err(FatError::BadChain);
            }
        }
        Ok(nrun)
    }
}

fn scan_dir_sec(sec: &[u8; 512]) -> Option<(u32, u32)> {
    for e in 0..16 {
        let ent = &sec[e * 32..e * 32 + 32];
        match ent[0] {
            0x00 => return None,
            0xE5 => continue,
            _ => {}
        }
        let attr = ent[11];
        if attr & 0x0F == 0x0F || attr & 0x08 != 0 || attr & 0x10 != 0 {
            continue;
        }
        if &ent[0..11] == &BOOT_NAME83 {
            let first = ((u16le(ent, 20) as u32) << 16) | u16le(ent, 26) as u32;
            let size = u32le(ent, 28);
            return Some((first, size));
        }
    }
    None
}
