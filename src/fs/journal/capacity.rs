//! SD card capacity from SDHCI 136-bit CMD9 (CSD) response registers.
//!
//! SDHCI stores R2 with CRC stripped and right-aligned by 8 bits. Linux
//! `sdhci_read_rsp_136` reconstructs CID/CSD as:
//!   csd[0] = (RESP3 << 8) | (RESP2 >> 24)   // bits 127:96
//!   csd[1] = (RESP2 << 8) | (RESP1 >> 24)
//!   csd[2] = (RESP1 << 8) | (RESP0 >> 24)
//!   csd[3] =  RESP0 << 8
//! Reading CSD_STRUCTURE from RESP3[31:30] without this shift yields 0 on
//! Zynq (observed `device_sectors=0`).

/// Reconstruct MMC `cmd->resp[0..3]` from raw SDHCI RESP0..RESP3.
pub fn sdhci_r2_to_csd(resp0: u32, resp1: u32, resp2: u32, resp3: u32) -> [u32; 4] {
    [
        (resp3 << 8) | (resp2 >> 24),
        (resp2 << 8) | (resp1 >> 24),
        (resp1 << 8) | (resp0 >> 24),
        resp0 << 8,
    ]
}

/// 512-byte sector count from SDHCI CMD9 response registers. `0` = unknown.
pub fn sdhci_csd_sectors(resp0: u32, resp1: u32, resp2: u32, resp3: u32) -> u32 {
    let csd = sdhci_r2_to_csd(resp0, resp1, resp2, resp3);
    csd_sectors_from_mmc_resp(csd)
}

fn csd_sectors_from_mmc_resp(csd: [u32; 4]) -> u32 {
    let structure = (csd[0] >> 30) & 3;
    match structure {
        1 => {
            // CSD v2: C_SIZE [69:48], capacity = (C_SIZE+1) * 512 KiB.
            let c_size = ((csd[1] & 0x3F) << 16) | (csd[2] >> 16);
            let sectors = (c_size as u64)
                .checked_add(1)
                .and_then(|n| n.checked_mul(1024))
                .filter(|&n| n > 0 && n <= u32::MAX as u64);
            match sectors {
                Some(n) => n as u32,
                None => 0,
            }
        }
        0 => {
            // CSD v1: READ_BL_LEN [83:80], C_SIZE [73:62], C_SIZE_MULT [49:47].
            let read_bl_len = (csd[1] >> 16) & 0xF;
            if read_bl_len >= 32 {
                return 0;
            }
            let c_size = ((csd[1] & 0x3FF) << 2) | (csd[2] >> 30);
            let c_size_mult = (csd[2] >> 15) & 0x7;
            let blocknr = (c_size as u64)
                .checked_add(1)
                .and_then(|n| n.checked_mul(1u64 << (c_size_mult + 2)));
            let block_len = 1u64 << read_bl_len;
            match blocknr.and_then(|n| n.checked_mul(block_len)) {
                Some(bytes) => u32::try_from(bytes / 512).unwrap_or(0),
                None => 0,
            }
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unshifted_resp3_msb_is_not_csd_structure() {
        // The old parser used resp3>>30. On SDHCI that field is not CSD[127:126].
        let resp3 = 0x0040_0000;
        assert_eq!((resp3 >> 30) & 3, 0);
        let sectors = sdhci_csd_sectors(0, 0x003B_FF00, 0, resp3);
        assert_eq!(sectors, 15_360 * 1024);
        assert_eq!(sectors, 15_728_640);
    }

    #[test]
    fn eight_gib_card_fits_measured_p1() {
        let device = sdhci_csd_sectors(0, 0x003B_FF00, 0, 0x0040_0000);
        let p1_start = 15_706_112u32;
        let p1_len = 16_384u32;
        let p1_end = p1_start.checked_add(p1_len).unwrap();
        assert_eq!(p1_end, 15_722_496);
        assert!(p1_end <= device);
        assert_eq!(device, 15_728_640);
    }

    #[test]
    fn unknown_structure_is_zero() {
        // csd[0] structure bits 11b after shift: resp3 = 0x00C00000
        assert_eq!(sdhci_csd_sectors(0, 0, 0, 0x00C0_0000), 0);
    }
}
