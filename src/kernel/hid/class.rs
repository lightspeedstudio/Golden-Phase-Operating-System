//! HID class requests and config-descriptor interface binding.

use super::types::{DESC_HID, DESC_REPORT, HID_CLASS, HidError, MAX_HID_INTERFACES};

const DESC_CONFIG: u8 = 0x02;
const DESC_INTERFACE: u8 = 0x04;
const DESC_ENDPOINT: u8 = 0x05;
const XFER_INTERRUPT: u8 = 0x03;
const EP_DIR_IN: u8 = 0x80;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HidInterface {
    pub iface: u8,
    pub alt: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub ep_in: u8,
    pub ep_in_maxpkt: u16,
    pub ep_in_interval: u8,
    pub hid_bcd: u16,
    pub report_desc_len: u16,
}

pub fn setup_get_hid_descriptor(iface: u8, len: u16) -> [u8; 8] {
    [
        0x81,
        0x06,
        0x00,
        DESC_HID,
        iface,
        0,
        (len & 0xff) as u8,
        (len >> 8) as u8,
    ]
}

pub fn setup_get_report_descriptor(iface: u8, len: u16) -> [u8; 8] {
    [
        0x81,
        0x06,
        0x00,
        DESC_REPORT,
        iface,
        0,
        (len & 0xff) as u8,
        (len >> 8) as u8,
    ]
}

pub fn setup_set_protocol(iface: u8, boot: bool) -> [u8; 8] {
    [0x21, 0x0B, if boot { 0 } else { 1 }, 0, iface, 0, 0, 0]
}

pub fn setup_get_protocol(iface: u8) -> [u8; 8] {
    [0xA1, 0x03, 0, 0, iface, 0, 1, 0]
}

pub fn setup_set_idle(iface: u8, duration: u8, report_id: u8) -> [u8; 8] {
    [0x21, 0x0A, report_id, duration, iface, 0, 0, 0]
}

fn walk_ok(blob: &[u8]) -> Result<(), HidError> {
    if blob.is_empty() {
        return Err(HidError::Truncated);
    }
    let mut i = 0usize;
    while i < blob.len() {
        if i + 2 > blob.len() {
            return Err(HidError::Truncated);
        }
        let blen = blob[i] as usize;
        if blen < 2 {
            return Err(HidError::LengthOverflow);
        }
        if i.checked_add(blen).map(|e| e > blob.len()).unwrap_or(true) {
            return Err(HidError::Truncated);
        }
        i += blen;
    }
    Ok(())
}

/// Bind every HID interface that has an interrupt-IN endpoint in this config blob.
pub fn parse_config_hid_interfaces(
    blob: &[u8],
    out: &mut [HidInterface],
) -> Result<usize, HidError> {
    walk_ok(blob)?;
    if blob.len() >= 2 && blob[1] != DESC_CONFIG && blob[1] != DESC_INTERFACE {
        return Err(HidError::Truncated);
    }
    let mut n = 0usize;
    let mut cur = HidInterface::default();
    let mut in_hid = false;
    let mut have_iface = false;

    let flush = |in_hid: bool,
                 have_iface: bool,
                 cur: HidInterface,
                 out: &mut [HidInterface],
                 n: &mut usize|
     -> Result<(), HidError> {
        if have_iface && in_hid {
            if cur.ep_in == 0 {
                return Ok(());
            }
            if *n >= out.len() || *n >= MAX_HID_INTERFACES {
                return Ok(());
            }
            out[*n] = cur;
            *n += 1;
        }
        Ok(())
    };

    let mut i = 0usize;
    while i + 2 <= blob.len() {
        let blen = blob[i] as usize;
        if blen < 2 || i + blen > blob.len() {
            return Err(HidError::Truncated);
        }
        match blob[i + 1] {
            DESC_INTERFACE if blen >= 9 => {
                flush(in_hid, have_iface, cur, out, &mut n)?;
                cur = HidInterface {
                    iface: blob[i + 2],
                    alt: blob[i + 3],
                    subclass: blob[i + 6],
                    protocol: blob[i + 7],
                    ..HidInterface::default()
                };
                in_hid = blob[i + 5] == HID_CLASS;
                have_iface = true;
            }
            DESC_HID if blen >= 9 && in_hid => {
                cur.hid_bcd = blob[i + 2] as u16 | ((blob[i + 3] as u16) << 8);
                let mut off = 6usize;
                while off + 3 <= blen {
                    let dt = blob[i + off];
                    let len = blob[i + off + 1] as u16 | ((blob[i + off + 2] as u16) << 8);
                    if dt == DESC_REPORT {
                        cur.report_desc_len = len;
                    }
                    off += 3;
                }
            }
            DESC_ENDPOINT if blen >= 7 && in_hid => {
                let addr = blob[i + 2];
                let attr = blob[i + 3];
                if (attr & 0x03) == XFER_INTERRUPT && (addr & EP_DIR_IN) != 0 && cur.ep_in == 0 {
                    cur.ep_in = addr;
                    cur.ep_in_maxpkt = blob[i + 4] as u16 | ((blob[i + 5] as u16) << 8);
                    cur.ep_in_interval = blob[i + 6];
                }
            }
            _ => {}
        }
        i += blen;
    }
    flush(in_hid, have_iface, cur, out, &mut n)?;
    if n == 0 {
        return Err(HidError::NoHidInterface);
    }
    Ok(n)
}

pub fn endpoint_in_interface(blob: &[u8], iface: u8, ep_addr: u8) -> Result<bool, HidError> {
    let mut found_iface = false;
    let mut in_target = false;
    let mut i = 0usize;
    while i + 2 <= blob.len() {
        let blen = blob[i] as usize;
        if blen < 2 || i + blen > blob.len() {
            return Err(HidError::Truncated);
        }
        match blob[i + 1] {
            DESC_INTERFACE if blen >= 9 => {
                in_target = blob[i + 2] == iface;
                if in_target {
                    found_iface = true;
                }
            }
            DESC_ENDPOINT if blen >= 7 && in_target => {
                if blob[i + 2] == ep_addr {
                    return Ok(true);
                }
            }
            _ => {}
        }
        i += blen;
    }
    if !found_iface {
        return Err(HidError::NoHidInterface);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::hid::fixtures::{COMPOSITE_CONFIG, SINGLE_KBD_CONFIG};
    use crate::kernel::hid::types::{HID_PROTO_KEYBOARD, HID_SUBCLASS_BOOT};

    #[test]
    fn request_bytes() {
        assert_eq!(
            setup_get_hid_descriptor(1, 9),
            [0x81, 0x06, 0x00, 0x21, 1, 0, 9, 0]
        );
        assert_eq!(
            setup_get_report_descriptor(1, 63),
            [0x81, 0x06, 0x00, 0x22, 1, 0, 63, 0]
        );
        assert_eq!(setup_set_protocol(1, true), [0x21, 0x0B, 0, 0, 1, 0, 0, 0]);
        assert_eq!(setup_set_protocol(1, false), [0x21, 0x0B, 1, 0, 1, 0, 0, 0]);
        assert_eq!(setup_get_protocol(2), [0xA1, 0x03, 0, 0, 2, 0, 1, 0]);
        assert_eq!(setup_set_idle(0, 0, 0), [0x21, 0x0A, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn bind_and_reject_foreign_ep() {
        let mut ifaces = [HidInterface::default(); 4];
        let n = parse_config_hid_interfaces(SINGLE_KBD_CONFIG, &mut ifaces).unwrap();
        assert_eq!(n, 1);
        assert_eq!(ifaces[0].subclass, HID_SUBCLASS_BOOT);
        assert_eq!(ifaces[0].protocol, HID_PROTO_KEYBOARD);
        assert_eq!(ifaces[0].ep_in, 0x81);
        assert_eq!(
            endpoint_in_interface(SINGLE_KBD_CONFIG, 0, 0x83).unwrap(),
            false
        );
        assert!(parse_config_hid_interfaces(COMPOSITE_CONFIG, &mut ifaces).unwrap() == 2);
    }
}
