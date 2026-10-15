//! Byte-exact HID fixtures. Host tests and `usb hid-neg` share these bytes.

use super::class::{parse_config_hid_interfaces, HidInterface};
use super::decode::{decode_report, DecodeState};
use super::queue::InputQueue;
use super::report::parse_report_descriptor;
use super::types::{HidError, InputEvent, MAX_HID_INTERFACES};

/// HID 1.11 Appendix B.1 boot keyboard report descriptor (63 bytes).
pub const BOOT_KEYBOARD_DESC: &[u8] = &[
    0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0x05, 0x07, 0x19, 0xE0, 0x29, 0xE7, 0x15, 0x00, 0x25, 0x01,
    0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x95, 0x01, 0x75, 0x08, 0x81, 0x01, 0x95, 0x05, 0x75, 0x01,
    0x05, 0x08, 0x19, 0x01, 0x29, 0x05, 0x91, 0x02, 0x95, 0x01, 0x75, 0x03, 0x91, 0x01, 0x95, 0x06,
    0x75, 0x08, 0x15, 0x00, 0x25, 0x65, 0x05, 0x07, 0x19, 0x00, 0x29, 0x65, 0x81, 0x00, 0xC0,
];

/// Same keyboard with Report ID 1 (report-protocol style).
pub const REPORT_ID_KEYBOARD_DESC: &[u8] = &[
    0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0x85, 0x01, 0x05, 0x07, 0x19, 0xE0, 0x29, 0xE7, 0x15, 0x00,
    0x25, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x95, 0x01, 0x75, 0x08, 0x81, 0x01, 0x95, 0x06,
    0x75, 0x08, 0x15, 0x00, 0x25, 0x65, 0x05, 0x07, 0x19, 0x00, 0x29, 0x65, 0x81, 0x00, 0xC0,
];

/// HID 1.11 Appendix B.2 relative mouse (buttons, X, Y, wheel).
pub const RELATIVE_MOUSE_DESC: &[u8] = &[
    0x05, 0x01, 0x09, 0x02, 0xA1, 0x01, 0x09, 0x01, 0xA1, 0x00, 0x05, 0x09, 0x19, 0x01, 0x29, 0x03,
    0x15, 0x00, 0x25, 0x01, 0x95, 0x03, 0x75, 0x01, 0x81, 0x02, 0x95, 0x01, 0x75, 0x05, 0x81, 0x01,
    0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x38, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x03,
    0x81, 0x06, 0xC0, 0xC0,
];

/// Two-interface config: boot keyboard IF0 EP 0x81, boot mouse IF1 EP 0x82.
pub const COMPOSITE_CONFIG: &[u8] = &[
    0x09, 0x02, 0x3B, 0x00, 0x02, 0x01, 0x00, 0xA0, 0x32, // config, total 59
    0x09, 0x04, 0x00, 0x00, 0x01, 0x03, 0x01, 0x01, 0x00, // IF0 HID boot kbd
    0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 0x3F, 0x00, // HID desc, report 63
    0x07, 0x05, 0x81, 0x03, 0x08, 0x00, 0x0A, // EP 0x81 INT IN 8 10ms
    0x09, 0x04, 0x01, 0x00, 0x01, 0x03, 0x01, 0x02, 0x00, // IF1 HID boot mouse
    0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 0x32, 0x00, // HID desc, report 50
    0x07, 0x05, 0x82, 0x03, 0x08, 0x00, 0x0A, // EP 0x82 INT IN
];

/// Config that names EP 0x81 on IF0 (bind of 0x83 must fail).
pub const SINGLE_KBD_CONFIG: &[u8] = &[
    0x09, 0x02, 0x22, 0x00, 0x01, 0x01, 0x00, 0xA0, 0x32,
    0x09, 0x04, 0x00, 0x00, 0x01, 0x03, 0x01, 0x01, 0x00,
    0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 0x3F, 0x00,
    0x07, 0x05, 0x81, 0x03, 0x08, 0x00, 0x0A,
];

/// Signed 64-bit field — cannot fit in i32 extract / HID short size rules.
pub const OVERSIZE_FIELD_DESC: &[u8] = &[
    0x05, 0x01, 0x09, 0x02, 0xA1, 0x01, 0x09, 0x30, 0x15, 0x80, 0x25, 0x7F, 0x75, 0x40, 0x95, 0x01,
    0x81, 0x06, 0xC0,
];

/// Report Size 255 × Count 255 exceeds 4096-bit cap.
pub const SIZE_COUNT_OVERFLOW_DESC: &[u8] = &[
    0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0x75, 0xFF, 0x95, 0xFF, 0x81, 0x02, 0xC0,
];

pub const TRUNCATED_DESC: &[u8] = &[0x05, 0x01, 0x09];

pub const LONG_ITEM_DESC: &[u8] = &[0x05, 0x01, 0xFE, 0x01, 0x00, 0x00, 0xC0];

#[derive(Clone, Copy)]
pub struct NegCase {
    pub name: &'static str,
    pub result: Result<(), HidError>,
}

/// Software negatives for host tests and board `usb hid-neg`. Not hardware PASS.
pub fn run_negative_suite(out: &mut [NegCase]) -> usize {
    let mut n = 0usize;
    let push = |name: &'static str, result: Result<(), HidError>, out: &mut [NegCase], n: &mut usize| {
        if *n < out.len() {
            out[*n] = NegCase { name, result };
            *n += 1;
        }
    };

    push(
        "truncated",
        parse_report_descriptor(TRUNCATED_DESC).map(|_| ()),
        out,
        &mut n,
    );
    push(
        "long-item",
        parse_report_descriptor(LONG_ITEM_DESC).map(|_| ()),
        out,
        &mut n,
    );
    push(
        "oversize-field",
        parse_report_descriptor(OVERSIZE_FIELD_DESC).map(|_| ()),
        out,
        &mut n,
    );
    push(
        "size-count-overflow",
        parse_report_descriptor(SIZE_COUNT_OVERFLOW_DESC).map(|_| ()),
        out,
        &mut n,
    );

    let parsed = parse_report_descriptor(REPORT_ID_KEYBOARD_DESC).unwrap();
    let mut st = DecodeState::new();
    let mut ev = [InputEvent::MouseWheel { delta: 0 }; 8];
    push(
        "unknown-report-id",
        decode_report(&parsed, &[0x02, 0, 0, 0, 0, 0, 0, 0, 0], &mut st, &mut ev).map(|_| ()),
        out,
        &mut n,
    );
    push(
        "short-report",
        decode_report(&parsed, &[0x01, 0x02], &mut st, &mut ev).map(|_| ()),
        out,
        &mut n,
    );

    let boot = parse_report_descriptor(BOOT_KEYBOARD_DESC).unwrap();
    push(
        "key-rollover",
        decode_report(&boot, &[0, 0, 1, 1, 1, 1, 1, 1], &mut st, &mut ev).map(|_| ()),
        out,
        &mut n,
    );

    let mut ifaces = [HidInterface::default(); MAX_HID_INTERFACES];
    let _ = parse_config_hid_interfaces(COMPOSITE_CONFIG, &mut ifaces);
    push(
        "ep-outside-iface",
        crate::kernel::hid::class::endpoint_in_interface(SINGLE_KBD_CONFIG, 0, 0x83).and_then(|ok| {
            if ok {
                Ok(())
            } else {
                Err(HidError::EndpointNotInInterface)
            }
        }),
        out,
        &mut n,
    );

    let mut q = InputQueue::new();
    for _ in 0..crate::kernel::hid::queue::INPUT_QUEUE_CAP {
        let _ = q.push(InputEvent::MouseWheel { delta: 1 });
    }
    push("queue-overflow", q.push(InputEvent::MouseWheel { delta: 2 }), out, &mut n);

    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::hid::class::endpoint_in_interface;
    use crate::kernel::hid::types::{HID_PROTO_KEYBOARD, HID_PROTO_MOUSE};

    #[test]
    fn boot_keyboard_parses() {
        let p = parse_report_descriptor(BOOT_KEYBOARD_DESC).unwrap();
        assert_eq!(p.input_byte_len(0), 8);
        let mut st = DecodeState::new();
        let mut ev = [InputEvent::MouseWheel { delta: 0 }; 16];
        let n = decode_report(&p, &[0x02, 0, 0x04, 0, 0, 0, 0, 0], &mut st, &mut ev).unwrap();
        assert!(ev[..n].iter().any(|e| matches!(e, InputEvent::KeyDown { usage: 0x04, .. })));
        let n = decode_report(&p, &[0, 0, 0, 0, 0, 0, 0, 0], &mut st, &mut ev).unwrap();
        assert!(ev[..n].iter().any(|e| matches!(e, InputEvent::KeyUp { usage: 0x04, .. })));
    }

    #[test]
    fn report_id_keyboard() {
        let p = parse_report_descriptor(REPORT_ID_KEYBOARD_DESC).unwrap();
        assert!(p.has_report_ids);
        let mut st = DecodeState::new();
        let mut ev = [InputEvent::MouseWheel { delta: 0 }; 16];
        let n = decode_report(&p, &[0x01, 0x00, 0, 0x05, 0, 0, 0, 0, 0], &mut st, &mut ev).unwrap();
        assert!(ev[..n].iter().any(|e| matches!(e, InputEvent::KeyDown { usage: 0x05, .. })));
    }

    #[test]
    fn relative_mouse() {
        let p = parse_report_descriptor(RELATIVE_MOUSE_DESC).unwrap();
        let mut st = DecodeState::new();
        let mut ev = [InputEvent::MouseWheel { delta: 0 }; 16];
        let n = decode_report(&p, &[0x01, 0x05, 0xFB, 0x01], &mut st, &mut ev).unwrap();
        assert!(ev[..n].iter().any(|e| matches!(e, InputEvent::MouseButton { button: 1, pressed: true })));
        assert!(ev[..n].iter().any(|e| matches!(e, InputEvent::MouseMove { dx: 5, dy: -5 })));
        assert!(ev[..n].iter().any(|e| matches!(e, InputEvent::MouseWheel { delta: 1 })));
    }

    #[test]
    fn composite_two_hid_ifaces() {
        let mut ifaces = [HidInterface::default(); MAX_HID_INTERFACES];
        let n = parse_config_hid_interfaces(COMPOSITE_CONFIG, &mut ifaces).unwrap();
        assert_eq!(n, 2);
        assert_eq!(ifaces[0].iface, 0);
        assert_eq!(ifaces[0].protocol, HID_PROTO_KEYBOARD);
        assert_eq!(ifaces[0].ep_in, 0x81);
        assert_eq!(ifaces[1].iface, 1);
        assert_eq!(ifaces[1].protocol, HID_PROTO_MOUSE);
        assert_eq!(ifaces[1].ep_in, 0x82);
        assert!(endpoint_in_interface(COMPOSITE_CONFIG, 0, 0x81).unwrap());
        assert!(!endpoint_in_interface(COMPOSITE_CONFIG, 0, 0x82).unwrap());
        assert!(endpoint_in_interface(COMPOSITE_CONFIG, 1, 0x82).unwrap());
    }

    #[test]
    fn negatives_all_err() {
        let mut cases = [NegCase {
            name: "",
            result: Ok(()),
        }; 16];
        let n = run_negative_suite(&mut cases);
        assert!(n >= 8);
        for c in cases[..n].iter() {
            assert!(c.result.is_err(), "{}", c.name);
        }
    }
}
