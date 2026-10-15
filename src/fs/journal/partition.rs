//! MBR slot-1 window checks. Never treats slot 0 as the journal.

use super::p6_plan::P6_PLAN_SECTORS;
use super::{JournalError, REGION_SECTORS};

/// One MBR entry as seen by the journal (not the FAT mount path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MbrSlot {
    pub start_lba: u32,
    pub sectors: u32,
    pub ptype: u8,
}

/// Accepted non-FS type for the sacrificial journal partition.
pub const JOURNAL_PTYPE_DA: u8 = 0xDA;

fn ranges_overlap(a0: u32, a_len: u32, b0: u32, b_len: u32) -> bool {
    let a1 = match a0.checked_add(a_len) {
        Some(v) => v,
        None => return true,
    };
    let b1 = match b0.checked_add(b_len) {
        Some(v) => v,
        None => return true,
    };
    a0 < b1 && b0 < a1
}

fn is_forbidden_fs_type(ptype: u8) -> bool {
    matches!(
        ptype,
        0x01 | 0x04 | 0x05 | 0x06 | 0x07 | 0x0B | 0x0C | 0x0E | 0x0F | 0x83 | 0xEE | 0xEF
    )
}

/// Select MBR slot 1 as the journal window.
///
/// `device_sectors` must be the CSD-derived 512-byte sector count. Zero is
/// refused: a corrupted MBR must not be allowed to describe LBAs past the
/// card when capacity is unknown.
pub fn validate_journal_window(
    slots: [Option<MbrSlot>; 4],
    device_sectors: u32,
) -> Result<MbrSlot, JournalError> {
    let p1 = slots[1].ok_or(JournalError::NoPartition)?;
    if p1.start_lba == 0 {
        return Err(JournalError::PartitionOutOfRange);
    }
    if p1.sectors < REGION_SECTORS.saturating_add(P6_PLAN_SECTORS) {
        return Err(JournalError::PartitionTooSmall);
    }
    let end = p1
        .start_lba
        .checked_add(p1.sectors)
        .ok_or(JournalError::PartitionOutOfRange)?;
    if device_sectors == 0 {
        return Err(JournalError::CapacityUnknown);
    }
    if end > device_sectors {
        return Err(JournalError::PartitionOutOfRange);
    }
    if is_forbidden_fs_type(p1.ptype) {
        return Err(JournalError::BadPartition);
    }
    for (i, slot) in slots.iter().enumerate() {
        if i == 1 {
            continue;
        }
        let Some(other) = slot else {
            continue;
        };
        if ranges_overlap(p1.start_lba, p1.sectors, other.start_lba, other.sectors) {
            return Err(JournalError::PartitionOverlap);
        }
    }
    Ok(p1)
}

/// Absolute LBA for a relative journal LBA. WAL/checkpoints occupy
/// `0..REGION_SECTORS`. The P6 crash-plan sector is `REGION_SECTORS`.
pub fn abs_journal_lba(p1_start: u32, rel: u32) -> Result<u32, JournalError> {
    if rel >= REGION_SECTORS.saturating_add(P6_PLAN_SECTORS) {
        return Err(JournalError::PartitionOutOfRange);
    }
    p1_start
        .checked_add(rel)
        .ok_or(JournalError::PartitionOutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(start: u32, sectors: u32, ptype: u8) -> MbrSlot {
        MbrSlot {
            start_lba: start,
            sectors,
            ptype,
        }
    }

    #[test]
    fn rejects_missing_slot1() {
        let slots = [Some(slot(2048, 100_000, 0x0C)), None, None, None];
        assert_eq!(
            validate_journal_window(slots, 0),
            Err(JournalError::NoPartition)
        );
    }

    #[test]
    fn rejects_slot0_as_journal() {
        let slots = [Some(slot(2048, 8192, JOURNAL_PTYPE_DA)), None, None, None];
        assert_eq!(
            validate_journal_window(slots, 0),
            Err(JournalError::NoPartition)
        );
    }

    #[test]
    fn rejects_start_lba_zero() {
        let slots = [None, Some(slot(0, 8192, JOURNAL_PTYPE_DA)), None, None];
        assert_eq!(
            validate_journal_window(slots, 0),
            Err(JournalError::PartitionOutOfRange)
        );
    }

    #[test]
    fn rejects_undersized() {
        let slots = [None, Some(slot(8192, 100, JOURNAL_PTYPE_DA)), None, None];
        assert_eq!(
            validate_journal_window(slots, 0),
            Err(JournalError::PartitionTooSmall)
        );
    }

    #[test]
    fn rejects_overlap_with_p0() {
        let slots = [
            Some(slot(2048, 8192, 0x0C)),
            Some(slot(4096, 8192, JOURNAL_PTYPE_DA)),
            None,
            None,
        ];
        assert_eq!(
            validate_journal_window(slots, 1_000_000),
            Err(JournalError::PartitionOverlap)
        );
    }

    #[test]
    fn rejects_fat_type_on_p1() {
        let slots = [None, Some(slot(8192, 8192, 0x0C)), None, None];
        assert_eq!(
            validate_journal_window(slots, 1_000_000),
            Err(JournalError::BadPartition)
        );
    }

    #[test]
    fn rejects_gpt_protective() {
        let slots = [None, Some(slot(8192, 8192, 0xEE)), None, None];
        assert_eq!(
            validate_journal_window(slots, 1_000_000),
            Err(JournalError::BadPartition)
        );
    }

    #[test]
    fn rejects_overflow_length() {
        let slots = [
            None,
            Some(slot(u32::MAX - 10, u32::MAX, JOURNAL_PTYPE_DA)),
            None,
            None,
        ];
        assert_eq!(
            validate_journal_window(slots, 0),
            Err(JournalError::PartitionOutOfRange)
        );
    }

    #[test]
    fn rejects_past_device_end() {
        let slots = [None, Some(slot(1000, 8192, JOURNAL_PTYPE_DA)), None, None];
        assert_eq!(
            validate_journal_window(slots, 2000),
            Err(JournalError::PartitionOutOfRange)
        );
    }

    #[test]
    fn accepts_da_after_fat() {
        let slots = [
            Some(slot(2048, 2048, 0x0C)),
            Some(slot(4096, 8192, JOURNAL_PTYPE_DA)),
            None,
            None,
        ];
        let w = validate_journal_window(slots, 1_000_000).unwrap();
        assert_eq!(w.start_lba, 4096);
        assert_eq!(w.sectors, 8192);
    }

    #[test]
    fn abs_lba_stays_in_region() {
        assert!(abs_journal_lba(4096, REGION_SECTORS + 1).is_err());
        assert_eq!(
            abs_journal_lba(4096, REGION_SECTORS).unwrap(),
            4096 + REGION_SECTORS
        );
        assert_eq!(abs_journal_lba(4096, 0).unwrap(), 4096);
        assert_eq!(
            abs_journal_lba(4096, REGION_SECTORS - 1).unwrap(),
            4096 + REGION_SECTORS - 1
        );
    }

    #[test]
    fn rejects_unknown_capacity() {
        let slots = [None, Some(slot(4096, 8192, JOURNAL_PTYPE_DA)), None, None];
        assert_eq!(
            validate_journal_window(slots, 0),
            Err(JournalError::CapacityUnknown)
        );
    }

    #[test]
    fn measured_p1_fits_8gib_csd() {
        let p1_start = 15_706_112u32;
        let p1_len = 16_384u32;
        let p1_end = p1_start.checked_add(p1_len).unwrap();
        assert_eq!(p1_end, 15_722_496);
        let slots = [
            Some(slot(2048, 15_704_064, 0x0C)),
            Some(slot(p1_start, p1_len, JOURNAL_PTYPE_DA)),
            None,
            None,
        ];
        let w = validate_journal_window(slots, 15_728_640).unwrap();
        assert_eq!(w.start_lba, p1_start);
        assert_eq!(w.sectors, p1_len);
        assert!(validate_journal_window(slots, p1_end - 1).is_err());
    }
}
