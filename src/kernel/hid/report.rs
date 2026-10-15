//! HID report-descriptor parser (short items only). No allocation.

use super::types::{
    HidError, MAX_REPORT_DESC, MAX_REPORT_FIELDS, PAGE_GENERIC_DESKTOP, USAGE_KEYBOARD, USAGE_MOUSE,
};

const ITEM_MAIN: u8 = 0;
const ITEM_GLOBAL: u8 = 1;
const ITEM_LOCAL: u8 = 2;

const MAIN_INPUT: u8 = 8;
const MAIN_OUTPUT: u8 = 9;
const MAIN_COLLECTION: u8 = 10;
const MAIN_FEATURE: u8 = 11;
const MAIN_END_COLLECTION: u8 = 12;

const G_USAGE_PAGE: u8 = 0;
const G_LOGICAL_MIN: u8 = 1;
const G_LOGICAL_MAX: u8 = 2;
const G_REPORT_SIZE: u8 = 7;
const G_REPORT_ID: u8 = 8;
const G_REPORT_COUNT: u8 = 9;
const G_PUSH: u8 = 10;
const G_POP: u8 = 11;

const L_USAGE: u8 = 0;
const L_USAGE_MIN: u8 = 1;
const L_USAGE_MAX: u8 = 2;

const FLAG_CONST: u8 = 1 << 0;
const FLAG_VARIABLE: u8 = 1 << 1;
const FLAG_RELATIVE: u8 = 1 << 2;

const MAX_REPORT_IDS: usize = 8;
const MAX_PUSH: usize = 4;
const MAX_COLLECT: usize = 8;
const MAX_LOCAL_USAGES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportField {
    pub report_id: u8,
    pub bit_offset: u16,
    pub bit_size: u8,
    pub count: u16,
    pub usage_page: u16,
    pub usage_min: u16,
    pub usage_max: u16,
    pub logical_min: i32,
    pub logical_max: i32,
    pub flags: u8,
    pub app_usage: u16,
}

impl ReportField {
    pub fn is_const(self) -> bool {
        (self.flags & FLAG_CONST) != 0
    }
    pub fn is_variable(self) -> bool {
        (self.flags & FLAG_VARIABLE) != 0
    }
    pub fn is_relative(self) -> bool {
        (self.flags & FLAG_RELATIVE) != 0
    }
    pub fn signed(self) -> bool {
        self.logical_min < 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParsedReportDesc {
    pub fields: [ReportField; MAX_REPORT_FIELDS],
    pub n_fields: u8,
    pub has_report_ids: bool,
    pub max_input_bits: u16,
    pub app_usage: u16,
}

impl ParsedReportDesc {
    pub const fn empty() -> Self {
        Self {
            fields: [ReportField {
                report_id: 0,
                bit_offset: 0,
                bit_size: 0,
                count: 0,
                usage_page: 0,
                usage_min: 0,
                usage_max: 0,
                logical_min: 0,
                logical_max: 0,
                flags: 0,
                app_usage: 0,
            }; MAX_REPORT_FIELDS],
            n_fields: 0,
            has_report_ids: false,
            max_input_bits: 0,
            app_usage: 0,
        }
    }

    pub fn input_byte_len(&self, report_id: u8) -> usize {
        let mut bits = 0u16;
        for i in 0..self.n_fields as usize {
            let f = self.fields[i];
            if f.report_id != report_id {
                continue;
            }
            let end = f.bit_offset.saturating_add(f.bit_size as u16 * f.count);
            if end > bits {
                bits = end;
            }
        }
        let mut n = ((bits + 7) / 8) as usize;
        if self.has_report_ids {
            n += 1;
        }
        n
    }
}

#[derive(Clone, Copy)]
struct Globals {
    usage_page: u16,
    logical_min: i32,
    logical_max: i32,
    report_size: u32,
    report_count: u32,
    report_id: u8,
}

impl Globals {
    const fn new() -> Self {
        Self {
            usage_page: 0,
            logical_min: 0,
            logical_max: 0,
            report_size: 0,
            report_count: 0,
            report_id: 0,
        }
    }
}

fn item_size(prefix: u8) -> usize {
    match prefix & 0x03 {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 4,
    }
}

fn read_unsigned(data: &[u8]) -> u32 {
    let mut v = 0u32;
    for (i, b) in data.iter().enumerate() {
        v |= (*b as u32) << (8 * i);
    }
    v
}

fn read_signed(data: &[u8]) -> i32 {
    if data.is_empty() {
        return 0;
    }
    let u = read_unsigned(data);
    let bits = data.len() * 8;
    if bits >= 32 {
        return u as i32;
    }
    let sign = 1u32 << (bits - 1);
    if (u & sign) != 0 {
        (u | (!0u32 << bits)) as i32
    } else {
        u as i32
    }
}

fn width_ok(size: u32, signed: bool, min: i32, max: i32) -> Result<(), HidError> {
    if size == 0 || size > 32 {
        return Err(if signed {
            HidError::ImpossibleSignedWidth
        } else {
            HidError::InvalidSizeCount
        });
    }
    if signed {
        if size > 32 {
            return Err(HidError::ImpossibleSignedWidth);
        }
        let lo = if size == 32 {
            i32::MIN
        } else {
            -(1i32 << (size - 1))
        };
        let hi = if size == 32 {
            i32::MAX
        } else {
            (1i32 << (size - 1)) - 1
        };
        if min < lo || max > hi {
            return Err(HidError::ImpossibleSignedWidth);
        }
    }
    Ok(())
}

/// Parse a HID report descriptor. Rejects long items and overflow.
pub fn parse_report_descriptor(desc: &[u8]) -> Result<ParsedReportDesc, HidError> {
    if desc.is_empty() {
        return Err(HidError::Truncated);
    }
    if desc.len() > MAX_REPORT_DESC {
        return Err(HidError::DescriptorTooLarge);
    }

    let mut out = ParsedReportDesc::empty();
    let mut g = Globals::new();
    let mut push_stack = [Globals::new(); MAX_PUSH];
    let mut push_n = 0usize;

    let mut local_usage = [0u16; MAX_LOCAL_USAGES];
    let mut local_n = 0usize;
    let mut usage_min = 0u16;
    let mut usage_max = 0u16;
    let mut have_usage_range = false;

    let mut collect_usage = [0u16; MAX_COLLECT];
    let mut collect_n = 0usize;
    let mut app_usage = 0u16;

    let mut id_slots = [0u8; MAX_REPORT_IDS];
    let mut id_bits = [0u16; MAX_REPORT_IDS];
    let mut n_ids = 0usize;

    fn bit_slot(
        id: u8,
        id_slots: &mut [u8; MAX_REPORT_IDS],
        id_bits: &mut [u16; MAX_REPORT_IDS],
        n_ids: &mut usize,
    ) -> Result<usize, HidError> {
        for i in 0..*n_ids {
            if id_slots[i] == id {
                return Ok(i);
            }
        }
        if *n_ids >= MAX_REPORT_IDS {
            return Err(HidError::InvalidSizeCount);
        }
        let i = *n_ids;
        id_slots[i] = id;
        id_bits[i] = 0;
        *n_ids += 1;
        Ok(i)
    }

    let mut i = 0usize;
    while i < desc.len() {
        let prefix = desc[i];
        if prefix == 0xFE {
            return Err(HidError::UnsupportedLongItem);
        }
        let size = item_size(prefix);
        if i + 1 + size > desc.len() {
            return Err(HidError::Truncated);
        }
        let data = &desc[i + 1..i + 1 + size];
        let tag = (prefix >> 4) & 0x0F;
        let kind = (prefix >> 2) & 0x03;
        i += 1 + size;

        match kind {
            ITEM_GLOBAL => match tag {
                G_USAGE_PAGE => g.usage_page = read_unsigned(data) as u16,
                G_LOGICAL_MIN => g.logical_min = read_signed(data),
                G_LOGICAL_MAX => g.logical_max = read_signed(data),
                G_REPORT_SIZE => g.report_size = read_unsigned(data),
                G_REPORT_COUNT => g.report_count = read_unsigned(data),
                G_REPORT_ID => {
                    let id = read_unsigned(data) as u8;
                    if id == 0 {
                        return Err(HidError::UnknownReportId);
                    }
                    g.report_id = id;
                    out.has_report_ids = true;
                    let _ = bit_slot(id, &mut id_slots, &mut id_bits, &mut n_ids)?;
                }
                G_PUSH => {
                    if push_n >= MAX_PUSH {
                        return Err(HidError::NestedCollection);
                    }
                    push_stack[push_n] = g;
                    push_n += 1;
                }
                G_POP => {
                    if push_n == 0 {
                        return Err(HidError::Truncated);
                    }
                    push_n -= 1;
                    g = push_stack[push_n];
                }
                _ => {}
            },
            ITEM_LOCAL => match tag {
                L_USAGE => {
                    let raw = read_unsigned(data);
                    let usage = if data.len() == 4 {
                        g.usage_page = (raw >> 16) as u16;
                        (raw & 0xFFFF) as u16
                    } else {
                        raw as u16
                    };
                    if local_n < MAX_LOCAL_USAGES {
                        local_usage[local_n] = usage;
                        local_n += 1;
                    }
                    if !have_usage_range {
                        usage_min = usage;
                        usage_max = usage;
                    }
                }
                L_USAGE_MIN => {
                    usage_min = read_unsigned(data) as u16;
                    have_usage_range = true;
                }
                L_USAGE_MAX => {
                    usage_max = read_unsigned(data) as u16;
                    have_usage_range = true;
                }
                _ => {}
            },
            ITEM_MAIN => {
                match tag {
                    MAIN_COLLECTION => {
                        if collect_n >= MAX_COLLECT {
                            return Err(HidError::NestedCollection);
                        }
                        let u = if local_n > 0 { local_usage[0] } else { usage_min };
                        collect_usage[collect_n] = u;
                        collect_n += 1;
                        if data.first().copied().unwrap_or(0) == 0x01 && app_usage == 0 {
                            app_usage = u;
                            out.app_usage = u;
                        }
                    }
                    MAIN_END_COLLECTION => {
                        if collect_n == 0 {
                            return Err(HidError::Truncated);
                        }
                        collect_n -= 1;
                    }
                    MAIN_INPUT => {
                        let flags = data.first().copied().unwrap_or(0);
                        let bits = g
                            .report_size
                            .checked_mul(g.report_count)
                            .ok_or(HidError::InvalidSizeCount)?;
                        if bits > 4096 {
                            return Err(HidError::InvalidSizeCount);
                        }
                        if g.report_count > 0 {
                            width_ok(g.report_size, g.logical_min < 0, g.logical_min, g.logical_max)?;
                        }
                        let slot = bit_slot(g.report_id, &mut id_slots, &mut id_bits, &mut n_ids)?;
                        let off = id_bits[slot];
                        let next = off
                            .checked_add(bits as u16)
                            .ok_or(HidError::InvalidSizeCount)?;
                        id_bits[slot] = next;
                        if next > out.max_input_bits {
                            out.max_input_bits = next;
                        }
                        if out.n_fields as usize >= MAX_REPORT_FIELDS {
                            return Err(HidError::InvalidSizeCount);
                        }
                        let (umin, umax) = if local_n > 1 {
                            (local_usage[0], local_usage[local_n - 1])
                        } else if have_usage_range || local_n == 1 {
                            (usage_min, if have_usage_range { usage_max } else { usage_min })
                        } else {
                            (0, 0)
                        };
                        let app = if collect_n > 0 {
                            collect_usage[0]
                        } else {
                            app_usage
                        };
                        out.fields[out.n_fields as usize] = ReportField {
                            report_id: g.report_id,
                            bit_offset: off,
                            bit_size: g.report_size as u8,
                            count: g.report_count as u16,
                            usage_page: g.usage_page,
                            usage_min: umin,
                            usage_max: umax,
                            logical_min: g.logical_min,
                            logical_max: g.logical_max,
                            flags,
                            app_usage: app,
                        };
                        out.n_fields += 1;
                    }
                    MAIN_OUTPUT | MAIN_FEATURE => {
                        let bits = g
                            .report_size
                            .checked_mul(g.report_count)
                            .ok_or(HidError::InvalidSizeCount)?;
                        if bits > 4096 {
                            return Err(HidError::InvalidSizeCount);
                        }
                    }
                    _ => {}
                }
                local_n = 0;
                have_usage_range = false;
                usage_min = 0;
                usage_max = 0;
            }
            _ => return Err(HidError::UnsupportedLongItem),
        }
    }

    let _ = (PAGE_GENERIC_DESKTOP, USAGE_KEYBOARD, USAGE_MOUSE);
    Ok(out)
}

fn bit_get(report: &[u8], bit_offset: u32, bit_size: u32) -> Result<u32, HidError> {
    if bit_size == 0 || bit_size > 32 {
        return Err(HidError::ImpossibleSignedWidth);
    }
    let start = bit_offset / 8;
    let end_bit = bit_offset + bit_size;
    let end = ((end_bit + 7) / 8) as usize;
    if end > report.len() {
        return Err(HidError::ReportTooShort);
    }
    let mut v = 0u32;
    for b in 0..bit_size {
        let abs = bit_offset + b;
        let byte = report[(abs / 8) as usize];
        if (byte & (1 << (abs % 8))) != 0 {
            v |= 1 << b;
        }
    }
    let _ = start;
    Ok(v)
}

pub fn extract_unsigned(report: &[u8], bit_offset: u16, bit_size: u8) -> Result<u32, HidError> {
    bit_get(report, bit_offset as u32, bit_size as u32)
}

pub fn extract_signed(report: &[u8], bit_offset: u16, bit_size: u8) -> Result<i32, HidError> {
    let u = bit_get(report, bit_offset as u32, bit_size as u32)?;
    let bits = bit_size as u32;
    if bits == 0 || bits > 32 {
        return Err(HidError::ImpossibleSignedWidth);
    }
    if bits == 32 {
        return Ok(u as i32);
    }
    let sign = 1u32 << (bits - 1);
    if (u & sign) != 0 {
        Ok((u | (!0u32 << bits)) as i32)
    } else {
        Ok(u as i32)
    }
}

pub fn field_value(report: &[u8], field: &ReportField, index: u16) -> Result<i32, HidError> {
    if index >= field.count {
        return Err(HidError::ReportTooShort);
    }
    let off = field
        .bit_offset
        .checked_add(field.bit_size as u16 * index)
        .ok_or(HidError::InvalidSizeCount)?;
    let v = if field.signed() {
        extract_signed(report, off, field.bit_size)?
    } else {
        extract_unsigned(report, off, field.bit_size)? as i32
    };
    if v < field.logical_min || v > field.logical_max {
        return Err(HidError::ImpossibleSignedWidth);
    }
    Ok(v)
}

/// Payload after optional report-ID byte.
pub fn payload_and_id<'a>(parsed: &ParsedReportDesc, report: &'a [u8]) -> Result<(u8, &'a [u8]), HidError> {
    if parsed.has_report_ids {
        if report.is_empty() {
            return Err(HidError::ReportTooShort);
        }
        let id = report[0];
        let mut known = false;
        for i in 0..parsed.n_fields as usize {
            if parsed.fields[i].report_id == id {
                known = true;
                break;
            }
        }
        if !known {
            return Err(HidError::UnknownReportId);
        }
        Ok((id, &report[1..]))
    } else {
        Ok((0, report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::hid::fixtures;

    #[test]
    fn boot_keyboard_fields() {
        let p = parse_report_descriptor(fixtures::BOOT_KEYBOARD_DESC).unwrap();
        assert!(!p.has_report_ids);
        assert!(p.n_fields >= 2);
        assert_eq!(p.app_usage, USAGE_KEYBOARD);
        assert_eq!(p.input_byte_len(0), 8);
    }

    #[test]
    fn truncated_rejected() {
        assert_eq!(
            parse_report_descriptor(&[0x05]).unwrap_err(),
            HidError::Truncated
        );
    }

    #[test]
    fn long_item_rejected() {
        assert_eq!(
            parse_report_descriptor(&[0xFE, 0x02, 0x00, 0x00, 0x00]).unwrap_err(),
            HidError::UnsupportedLongItem
        );
    }

    #[test]
    fn oversized_field_rejected() {
        assert_eq!(
            parse_report_descriptor(fixtures::OVERSIZE_FIELD_DESC).unwrap_err(),
            HidError::ImpossibleSignedWidth
        );
    }

    #[test]
    fn size_count_overflow_rejected() {
        assert_eq!(
            parse_report_descriptor(fixtures::SIZE_COUNT_OVERFLOW_DESC).unwrap_err(),
            HidError::InvalidSizeCount
        );
    }
}
