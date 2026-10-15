// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! SD0 host: working init sequence, ADMA2 CMD18 transfer. PIO is degraded only.

pub mod bus;
pub mod fat;
pub mod sys04;
pub mod sys05;
pub mod sys06;
pub mod sys07;
pub mod sys08;
pub mod sys09;
pub mod sys10a;

use bus::{
    SdBus, ADMA_ERR, ADMA_SAR, ARG, BLK_CNT, BLK_SIZE, CLK, EISTER, EISTR, HOST, NISTER, NISTR,
    POWER, PRES, RESP0, RESP1, RESP2, RESP3, SWRST, TIMEOUT, XFER,
};

pub const PRES_CMDINHIBIT: u32 = 1 << 0;
pub const PRES_DATAINHIBIT: u32 = 1 << 1;
pub const NI_CMDCMP: u16 = 1 << 0;
pub const NI_TRFCMP: u16 = 1 << 1;
pub const NI_DMA: u16 = 1 << 3;
pub const NI_BRRR: u16 = 1 << 5;
pub const NI_ERR: u16 = 1 << 15;
pub const EI_ADMA: u16 = 1 << 9;

const RESP_NONE: u16 = 0;
const RESP_R48: u16 = 2;
const RESP_R48B: u16 = 3;
const RESP_R136: u16 = 1;
const CMD_CRC: u16 = 1 << 3;
const CMD_IDX: u16 = 1 << 4;
const DATA_PRESENT: u16 = 1 << 5;

const TM_DMA: u16 = 1 << 0;
const TM_BLKCNT_EN: u16 = 1 << 1;
const TM_ACMD12: u16 = 1 << 2;
const TM_READ: u16 = 1 << 4;
const TM_MULTI: u16 = 1 << 5;

const ADMA_VALID: u16 = 1;
const ADMA_END: u16 = 2;
const ADMA_TRAN: u16 = 0x20;

const WAIT_INHIBIT: u32 = 2_000_000;
const WAIT_NI: u32 = 2_000_000;
const WAIT_SWRST: u32 = 1_000_000;
const WAIT_CLK: u32 = 100_000;
pub const MAX_ADMA_DESC: usize = 8;
pub const READ_16K: usize = 16384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdError {
    NoCard,
    Timeout,
    CmdError,
    NotSdV2,
    NotReady,
    Adma,
    Fragmented,
    PioForbidden,
}

pub struct SdCard {
    pub rca: u16,
    pub hcc: bool,
    pub device_sectors: u32,
    pub csd_resp: [u32; 4],
    pub pio_used: bool,
}

fn wait_clear<B: SdBus>(bus: &mut B, mask: u32, spins: u32) -> Result<(), SdError> {
    for _ in 0..spins {
        if (bus.r32(PRES) & mask) == 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(SdError::Timeout)
}

fn wait_ni<B: SdBus>(bus: &mut B, mask: u16, spins: u32) -> Result<(), SdError> {
    for _ in 0..spins {
        let ni = bus.r16(NISTR);
        if (ni & NI_ERR) != 0 {
            let ei = bus.r16(EISTR);
            bus.w16(EISTR, 0xFFFF);
            bus.w16(NISTR, NI_ERR);
            if (ei & EI_ADMA) != 0 || bus.r16(ADMA_ERR) != 0 {
                return Err(SdError::Adma);
            }
            return Err(SdError::CmdError);
        }
        if (ni & mask) == mask {
            bus.w16(NISTR, mask);
            return Ok(());
        }
        if (ni & NI_DMA) != 0 {
            bus.w16(NISTR, NI_DMA);
            continue;
        }
        core::hint::spin_loop();
    }
    Err(SdError::Timeout)
}

fn cmd_raw<B: SdBus>(bus: &mut B, index: u8, arg: u32, flags: u16, xfer: u16) -> Result<u32, SdError> {
    wait_clear(bus, PRES_CMDINHIBIT | PRES_DATAINHIBIT, WAIT_INHIBIT)?;
    bus.w16(NISTR, 0xFFFF);
    bus.w16(EISTR, 0xFFFF);
    bus.w32(ARG, arg);
    let cmd = ((index as u16) << 8) | flags;
    bus.w32(XFER, (cmd as u32) << 16 | (xfer as u32));
    wait_ni(bus, NI_CMDCMP, WAIT_NI)?;
    Ok(bus.r32(RESP0))
}

fn cmd<B: SdBus>(bus: &mut B, index: u8, arg: u32, flags: u16) -> Result<u32, SdError> {
    cmd_raw(bus, index, arg, flags, 0)
}

fn set_clock_div<B: SdBus>(bus: &mut B, div: u16) {
    let mut c = bus.r16(CLK);
    c &= !0x0004;
    bus.w16(CLK, c);
    bus.delay(1000);
    c = (div << 8) | 0x0001;
    bus.w16(CLK, c);
    for _ in 0..WAIT_CLK {
        if (bus.r16(CLK) & 0x0002) != 0 {
            break;
        }
        core::hint::spin_loop();
    }
    bus.w16(CLK, c | 0x0004);
    bus.delay(10_000);
}

fn sdhci_r2_to_csd(resp0: u32, resp1: u32, resp2: u32, resp3: u32) -> [u32; 4] {
    [
        (resp3 << 8) | (resp2 >> 24),
        (resp2 << 8) | (resp1 >> 24),
        (resp1 << 8) | (resp0 >> 24),
        resp0 << 8,
    ]
}

fn csd_sectors(resp0: u32, resp1: u32, resp2: u32, resp3: u32) -> u32 {
    let csd = sdhci_r2_to_csd(resp0, resp1, resp2, resp3);
    let structure = (csd[0] >> 30) & 3;
    match structure {
        1 => {
            let c_size = ((csd[1] & 0x3F) << 16) | (csd[2] >> 16);
            c_size.saturating_add(1).saturating_mul(1024)
        }
        _ => 0,
    }
}

impl SdCard {
    fn acmd<B: SdBus>(&self, bus: &mut B, index: u8, arg: u32, flags: u16) -> Result<u32, SdError> {
        cmd(bus, 55, (self.rca as u32) << 16, RESP_R48 | CMD_CRC | CMD_IDX)?;
        cmd(bus, index, arg, flags)
    }

    pub fn init<B: SdBus>(bus: &mut B) -> Result<SdCard, SdError> {
        bus.w8(SWRST, 0x01);
        for _ in 0..WAIT_SWRST {
            if (bus.r8(SWRST) & 0x01) == 0 {
                break;
            }
            core::hint::spin_loop();
        }
        bus.w8(POWER, 0x0F);
        bus.delay(50_000);
        bus.w8(TIMEOUT, 0x0E);
        bus.w16(NISTER, 0x01FF);
        bus.w16(EISTER, 0xF3FF);
        bus.w8(HOST, 0x00);
        set_clock_div(bus, 128);

        let mut card = SdCard {
            rca: 0,
            hcc: false,
            device_sectors: 0,
            csd_resp: [0; 4],
            pio_used: false,
        };
        cmd(bus, 0, 0, RESP_NONE)?;
        bus.delay(10_000);
        let r7 = cmd(bus, 8, 0x0000_01AA, RESP_R48 | CMD_CRC | CMD_IDX)?;
        if (r7 & 0xFF) != 0xAA {
            return Err(SdError::NotSdV2);
        }
        let mut ready = false;
        for _ in 0..1000 {
            let ocr = card.acmd(bus, 41, 0x4030_0000, RESP_R48)?;
            if (ocr & (1 << 31)) != 0 {
                card.hcc = (ocr & (1 << 30)) != 0;
                ready = true;
                break;
            }
            bus.delay(50_000);
        }
        if !ready {
            return Err(SdError::NotReady);
        }
        cmd(bus, 2, 0, RESP_R136 | CMD_CRC)?;
        let rca_resp = cmd(bus, 3, 0, RESP_R48 | CMD_CRC | CMD_IDX)?;
        card.rca = (rca_resp >> 16) as u16;
        cmd(bus, 9, (card.rca as u32) << 16, RESP_R136 | CMD_CRC)?;
        let r0 = bus.r32(RESP0);
        let r1 = bus.r32(RESP1);
        let r2 = bus.r32(RESP2);
        let r3 = bus.r32(RESP3);
        card.csd_resp = [r0, r1, r2, r3];
        card.device_sectors = csd_sectors(r0, r1, r2, r3);
        cmd(
            bus,
            7,
            (card.rca as u32) << 16,
            RESP_R48B | CMD_CRC | CMD_IDX,
        )?;
        set_clock_div(bus, 4);
        card.acmd(bus, 6, 2, RESP_R48 | CMD_CRC | CMD_IDX)?;
        bus.w8(HOST, 0x12);
        cmd(bus, 16, 512, RESP_R48 | CMD_CRC | CMD_IDX)?;
        Ok(card)
    }

    pub fn block_addr(&self, lba: u32) -> u32 {
        if self.hcc {
            lba
        } else {
            lba.saturating_mul(512)
        }
    }

    /// Degraded PIO CMD17. Success path must not call this.
    pub fn read_block_pio_degraded<B: SdBus>(
        &mut self,
        _bus: &mut B,
        _lba: u32,
        _buf: &mut [u8; 512],
    ) -> Result<(), SdError> {
        self.pio_used = true;
        Err(SdError::PioForbidden)
    }

    pub fn read_blocks_adma<B: SdBus>(
        &mut self,
        bus: &mut B,
        lba: u32,
        dest_phys: u32,
        nbytes: u32,
        table_phys: u32,
    ) -> Result<(), SdError> {
        if nbytes == 0 || nbytes % 512 != 0 || dest_phys % 4 != 0 || table_phys % 4 != 0 {
            return Err(SdError::Adma);
        }
        if self.pio_used {
            return Err(SdError::PioForbidden);
        }
        let blocks = nbytes / 512;
        if blocks > 65535 {
            return Err(SdError::Adma);
        }
        let attr = ADMA_VALID | ADMA_END | ADMA_TRAN;
        let mut desc = [0u8; 8];
        desc[0..2].copy_from_slice(&attr.to_le_bytes());
        desc[2..4].copy_from_slice(&(nbytes as u16).to_le_bytes());
        desc[4..8].copy_from_slice(&dest_phys.to_le_bytes());
        bus.phys_write(table_phys, &desc);
        wait_clear(bus, PRES_CMDINHIBIT | PRES_DATAINHIBIT, WAIT_INHIBIT)?;
        bus.w16(BLK_SIZE, 512);
        bus.w16(BLK_CNT, blocks as u16);
        bus.w32(ADMA_SAR, table_phys);
        let xfer = TM_DMA | TM_BLKCNT_EN | TM_ACMD12 | TM_READ | TM_MULTI;
        let flags = RESP_R48 | CMD_CRC | CMD_IDX | DATA_PRESENT;
        cmd_raw(bus, 18, self.block_addr(lba), flags, xfer)?;
        wait_ni(bus, NI_TRFCMP, WAIT_NI)?;
        if (bus.r16(EISTR) & EI_ADMA) != 0 || bus.r16(ADMA_ERR) != 0 {
            return Err(SdError::Adma);
        }
        Ok(())
    }
}

/// Contiguous LBA run for one ADMA descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LbaRun {
    pub lba: u32,
    pub blocks: u32,
}

pub fn runs_to_adma(runs: &[LbaRun], dest_phys: u32, table: &mut [[u8; 8]]) -> Result<usize, SdError> {
    if runs.is_empty() || runs.len() > table.len() || runs.len() > MAX_ADMA_DESC {
        return Err(SdError::Fragmented);
    }
    let mut addr = dest_phys;
    for (i, run) in runs.iter().enumerate() {
        if run.blocks == 0 {
            return Err(SdError::Adma);
        }
        let nbytes = run.blocks.saturating_mul(512);
        if nbytes > 65535 {
            return Err(SdError::Adma);
        }
        let mut attr = ADMA_VALID | ADMA_TRAN;
        if i + 1 == runs.len() {
            attr |= ADMA_END;
        }
        table[i][0..2].copy_from_slice(&attr.to_le_bytes());
        table[i][2..4].copy_from_slice(&(nbytes as u16).to_le_bytes());
        table[i][4..8].copy_from_slice(&addr.to_le_bytes());
        addr = addr.saturating_add(nbytes);
    }
    Ok(runs.len())
}

/// Multi-run ADMA: one CMD18 per contiguous run would split Auto CMD12. One
/// descriptor list covers sequential dest; source LBAs must be one run or we
/// issue sequential CMD18 per run (each Auto CMD12).
pub fn read_runs_adma<B: SdBus>(
    card: &mut SdCard,
    bus: &mut B,
    runs: &[LbaRun],
    dest_phys: u32,
    table_phys: u32,
) -> Result<(), SdError> {
    if runs.len() > MAX_ADMA_DESC {
        return Err(SdError::Fragmented);
    }
    let mut addr = dest_phys;
    let mut slot = table_phys;
    for run in runs {
        let n = run.blocks * 512;
        card.read_blocks_adma(bus, run.lba, addr, n, slot)?;
        addr = addr.saturating_add(n);
        slot = slot.saturating_add(8);
    }
    Ok(())
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::bus::HostSd;
    use super::sys04::BOOT16_PHYS;
    use super::*;

    fn put_sec(h: &mut HostSd, lba: u32, data: &[u8]) {
        let mut s = [0u8; 512];
        s[..data.len()].copy_from_slice(data);
        h.sectors.insert(lba, s);
    }

    fn le16(v: u16) -> [u8; 2] {
        v.to_le_bytes()
    }
    fn le32(v: u32) -> [u8; 4] {
        v.to_le_bytes()
    }

    /// Working `SdCard::init` command list through CMD7 (card select).
    fn working_cmds_through_select(rca: u16) -> [(u8, u32, u16, u16); 8] {
        [
            (0, 0, 0, 0),
            (8, 0x0000_01AA, 0x1A, 0),
            (55, 0, 0x1A, 0),
            (41, 0x4030_0000, 0x02, 0),
            (2, 0, 0x09, 0),
            (3, 0, 0x1A, 0),
            (9, (rca as u32) << 16, 0x09, 0),
            (7, (rca as u32) << 16, 0x1B, 0),
        ]
    }

    #[test]
    fn init_matches_working_through_select() {
        let mut h = HostSd::new();
        let card = SdCard::init(&mut h).unwrap();
        assert_eq!(&h.cmds[..8], &working_cmds_through_select(0x1234));
        assert_eq!(card.rca, 0x1234);
        assert!(card.hcc);
        assert_eq!(card.device_sectors, 15_728_640);
        assert!(!card.pio_used);
        let host_writes: Vec<_> = h
            .trace
            .iter()
            .filter(|t| t.off == crate::sd::bus::HOST && t.width == 8)
            .map(|t| t.val)
            .collect();
        assert_eq!(host_writes.first().copied(), Some(0));
        assert_eq!(*host_writes.last().unwrap(), 0x12);
        assert_ne!(*host_writes.last().unwrap(), 0x02);
        assert!(h.cmds.iter().any(|c| c.0 == 16 && c.1 == 512));
        assert!(h.cmds[8..].iter().any(|c| c.0 == 6 && c.1 == 2));
    }

    fn fat32_with_boot(payload: &[u8]) -> HostSd {
        let mut h = HostSd::new();
        let mut bpb = [0u8; 512];
        bpb[0] = 0xEB;
        bpb[1] = 0x58;
        bpb[2] = 0x90;
        bpb[11..13].copy_from_slice(&le16(512));
        bpb[13] = 1;
        bpb[14..16].copy_from_slice(&le16(1));
        bpb[16] = 1;
        bpb[21] = 0xF8;
        bpb[32..36].copy_from_slice(&le32(128));
        bpb[36..40].copy_from_slice(&le32(1));
        bpb[44..48].copy_from_slice(&le32(2));
        bpb[82..90].copy_from_slice(b"FAT32   ");
        bpb[510] = 0x55;
        bpb[511] = 0xAA;
        put_sec(&mut h, 0, &bpb);
        let nclus = ((payload.len() + 511) / 512) as u32;
        let mut fat = [0u8; 512];
        fat[0..4].copy_from_slice(&0x0FFF_FFF8u32.to_le_bytes());
        fat[4..8].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());
        fat[8..12].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());
        for i in 0..nclus {
            let cl = 3 + i;
            let nxt = if i + 1 == nclus {
                0x0FFF_FFFF
            } else {
                cl + 1
            };
            let o = (cl * 4) as usize;
            fat[o..o + 4].copy_from_slice(&nxt.to_le_bytes());
        }
        put_sec(&mut h, 1, &fat);
        let mut root = [0u8; 512];
        root[0..11].copy_from_slice(b"BOOT    BIN");
        root[11] = 0x20;
        root[26..28].copy_from_slice(&le16(3));
        root[28..32].copy_from_slice(&le32(payload.len() as u32));
        put_sec(&mut h, 2, &root);
        for (i, chunk) in payload.chunks(512).enumerate() {
            put_sec(&mut h, 3 + i as u32, chunk);
        }
        h
    }

    #[test]
    fn adma_reads_first_16k_no_pio() {
        let payload = include_bytes!("../../../../docs/boot/SYS04_BOOT_FIRST_16K.bin");
        assert_eq!(payload.len(), 16384);
        let mut h = fat32_with_boot(payload);
        let size = crate::sd::sys04::load_boot_first_16k(&mut h).unwrap();
        assert_eq!(size, 16384);
        assert!(h.adma_used);
        assert_eq!(h.pio_buf_reads, 0);
        assert!(h.cmds.iter().any(|c| c.0 == 18));
        assert!(!h.cmds.iter().any(|c| c.0 == 17));
        let mut got = [0u8; 16384];
        h.phys_read(BOOT16_PHYS, &mut got);
        assert_eq!(&got, payload.as_slice());
        let sync = u32::from_le_bytes(got[0x20..0x24].try_into().unwrap());
        let ident = u32::from_le_bytes(got[0x24..0x28].try_into().unwrap());
        assert_eq!(sync, 0xAA99_5566);
        assert_eq!(ident, 0x584C_4E58);
        crate::sd::sys04::validate_boot16(&mut h, BOOT16_PHYS).unwrap();
        let src = include_str!("sys04.rs");
        assert!(src.contains("SD FAT BOOT.BIN PASS bytes="));
        assert!(src.contains("SD ADMA2 CMD18 PASS bytes=16384"));
        assert!(!src.contains("FAT PASS\n"));
        assert!(!src.contains("ADMA 16K PASS"));
        let mut digits = [0u8; 10];
        assert_eq!(crate::sd::sys04::fmt_dec_u32(16384, &mut digits), b"16384");
        let mut hex = [0u8; 8];
        assert_eq!(crate::sd::sys04::fmt_hex_u32(0xAA99_5566, &mut hex), b"aa995566");
        assert_eq!(crate::sd::sys04::fmt_hex_u32(0x6655_99AA, &mut hex), b"665599aa");
        assert_eq!(
            crate::sd::sys04::ADMA_PASS_LINE,
            "SD ADMA2 CMD18 PASS bytes=16384\n"
        );
    }

    #[test]
    fn fat_uart_bytes_is_directory_size() {
        let packed = std::fs::metadata(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/build/BOOT_SYS04_ADMA.BIN"
        ))
        .unwrap()
        .len() as u32;
        assert_eq!(packed, 5_997_268);
        let payload = include_bytes!("../../../../docs/boot/SYS04_BOOT_FIRST_16K.bin");
        let mut h = fat32_with_boot(payload);
        let mut root = h.sectors.get(&2).copied().unwrap();
        root[28..32].copy_from_slice(&packed.to_le_bytes());
        h.sectors.insert(2, root);
        let size = crate::sd::sys04::load_boot_first_16k(&mut h).unwrap();
        assert_eq!(size, packed);
        let mut digits = [0u8; 10];
        assert_eq!(crate::sd::sys04::fmt_dec_u32(size, &mut digits), b"5997268");
    }

    #[test]
    fn header_words_are_checked_at_separate_offsets() {
        let mut h = HostSd::new();
        let mut good = [0u8; 8];
        good[0..4].copy_from_slice(&0xAA99_5566u32.to_le_bytes());
        good[4..8].copy_from_slice(&0x584C_4E58u32.to_le_bytes());
        h.phys_write(BOOT16_PHYS + 0x20, &good);
        crate::sd::sys04::validate_boot16(&mut h, BOOT16_PHYS).unwrap();
        h.phys_write(BOOT16_PHYS + 0x20, &0u32.to_le_bytes());
        assert_eq!(
            crate::sd::sys04::validate_boot16(&mut h, BOOT16_PHYS),
            Err(SdError::Adma)
        );
        h.phys_write(BOOT16_PHYS + 0x20, &0xAA99_5566u32.to_le_bytes());
        h.phys_write(BOOT16_PHYS + 0x24, &0u32.to_le_bytes());
        assert_eq!(
            crate::sd::sys04::validate_boot16(&mut h, BOOT16_PHYS),
            Err(SdError::Adma)
        );
    }

    #[test]
    fn dirent_fixture_is_boot_bin() {
        let e = include_bytes!("../../../../docs/boot/SYS04_BOOT_DIRENT.bin");
        assert_eq!(&e[0..11], b"BOOT    BIN");
        assert_eq!(e[11], 0x20);
        let first = (u16::from_le_bytes([e[20], e[21]]) as u32) << 16
            | u16::from_le_bytes([e[26], e[27]]) as u32;
        let size = u32::from_le_bytes([e[28], e[29], e[30], e[31]]);
        assert_eq!(first, 14);
        assert_eq!(size, 5_992_148);
    }

    #[test]
    fn fragmented_over_max_refused() {
        let mut runs = [LbaRun { lba: 0, blocks: 1 }; 9];
        for i in 0..9 {
            runs[i].lba = i as u32 * 10;
        }
        let mut table = [[0u8; 8]; 8];
        assert_eq!(
            runs_to_adma(&runs, 0x2000_0000, &mut table),
            Err(SdError::Fragmented)
        );
    }

    #[test]
    fn degraded_pio_is_not_success_path() {
        let mut h = HostSd::new();
        let mut card = SdCard::init(&mut h).unwrap();
        let mut buf = [0u8; 512];
        assert_eq!(
            card.read_block_pio_degraded(&mut h, 0, &mut buf),
            Err(SdError::PioForbidden)
        );
        assert!(card.pio_used);
    }
}

