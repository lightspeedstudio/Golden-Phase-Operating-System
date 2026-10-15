//! Bare-metal USB HID host v1 — parser, decoders, bounded InputEvent queue.
//!
//! Transport lives in `board::zynq7000::usb`. This module does not talk to
//! the ChipIdea controller. Host fixtures prove parser logic only.

pub mod class;
pub mod decode;
pub mod fixtures;
pub mod keymap;
pub mod queue;
pub mod report;
pub mod types;

pub use class::{
    endpoint_in_interface, parse_config_hid_interfaces, setup_get_hid_descriptor,
    setup_get_protocol, setup_get_report_descriptor, setup_set_idle, setup_set_protocol,
    HidInterface,
};
pub use decode::{decode_report, DecodeState};
pub use fixtures::{run_negative_suite, NegCase};
pub use keymap::usage_to_ascii;
pub use queue::{
    clear_global, poll_global, push_global, InputQueue, INPUT_QUEUE_CAP,
};
pub use report::{
    extract_signed, extract_unsigned, parse_report_descriptor, ParsedReportDesc, ReportField,
};
pub use types::{HidError, InputEvent};
