//! HID Host v1 types. Usages are HID usage IDs, not characters.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEvent {
    KeyDown { usage: u16, modifiers: u8 },
    KeyUp { usage: u16, modifiers: u8 },
    MouseMove { dx: i16, dy: i16 },
    MouseButton { button: u8, pressed: bool },
    MouseWheel { delta: i8 },
    DeviceAttached { device_id: u16, interface: u8 },
    DeviceDetached { device_id: u16, interface: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HidError {
    Truncated,
    LengthOverflow,
    InvalidSizeCount,
    UnsupportedLongItem,
    UnknownReportId,
    ReportTooShort,
    EndpointNotInInterface,
    QueueOverflow,
    KeyRollover,
    ImpossibleSignedWidth,
    NoHidInterface,
    DescriptorTooLarge,
    NestedCollection,
}

impl HidError {
    pub fn as_str(self) -> &'static str {
        match self {
            HidError::Truncated => "truncated descriptor",
            HidError::LengthOverflow => "descriptor length overflow",
            HidError::InvalidSizeCount => "invalid report size/count",
            HidError::UnsupportedLongItem => "unsupported long item",
            HidError::UnknownReportId => "unknown report id",
            HidError::ReportTooShort => "report shorter than declared",
            HidError::EndpointNotInInterface => "endpoint not in interface",
            HidError::QueueOverflow => "input queue overflow",
            HidError::KeyRollover => "keyboard rollover/error report",
            HidError::ImpossibleSignedWidth => "impossible signed field width",
            HidError::NoHidInterface => "no hid interface",
            HidError::DescriptorTooLarge => "descriptor exceeds buffer",
            HidError::NestedCollection => "collection nest overflow",
        }
    }
}

pub const PAGE_GENERIC_DESKTOP: u16 = 0x01;
pub const PAGE_KEYBOARD: u16 = 0x07;
pub const PAGE_LED: u16 = 0x08;
pub const PAGE_BUTTON: u16 = 0x09;

pub const USAGE_MOUSE: u16 = 0x02;
pub const USAGE_KEYBOARD: u16 = 0x06;
pub const USAGE_X: u16 = 0x30;
pub const USAGE_Y: u16 = 0x31;
pub const USAGE_WHEEL: u16 = 0x38;

pub const KEY_ERROR_ROLLOVER: u16 = 0x01;
pub const KEY_POST_FAIL: u16 = 0x02;
pub const KEY_ERROR_UNDEFINED: u16 = 0x03;
pub const KEY_MOD_MIN: u16 = 0xE0;
pub const KEY_MOD_MAX: u16 = 0xE7;

pub const HID_CLASS: u8 = 0x03;
pub const HID_SUBCLASS_BOOT: u8 = 0x01;
pub const HID_PROTO_KEYBOARD: u8 = 0x01;
pub const HID_PROTO_MOUSE: u8 = 0x02;
pub const HID_PROTOCOL_BOOT: u8 = 0;
pub const HID_PROTOCOL_REPORT: u8 = 1;

pub const DESC_HID: u8 = 0x21;
pub const DESC_REPORT: u8 = 0x22;

pub const MAX_HID_INTERFACES: usize = 4;
pub const MAX_REPORT_FIELDS: usize = 32;
pub const MAX_REPORT_DESC: usize = 512;
pub const MAX_KEYS_DOWN: usize = 8;
