//! HID host bind/poll/hotplug on top of the proven EHCI transport.
//! Does not replace `attach_keyboard` / `poll_key`.

use crate::kernel::hid::class::{
    endpoint_in_interface, parse_config_hid_interfaces, setup_get_hid_descriptor,
    setup_get_protocol, setup_get_report_descriptor, setup_set_idle, setup_set_protocol,
    HidInterface,
};
use crate::kernel::hid::decode::{decode_report, DecodeState};
use crate::kernel::hid::keymap::usage_to_ascii;
use crate::kernel::hid::queue::{clear_global, poll_global, push_global};
use crate::kernel::hid::report::{parse_report_descriptor, ParsedReportDesc};
use crate::kernel::hid::types::{
    HidError, InputEvent, MAX_HID_INTERFACES, MAX_REPORT_DESC, HID_SUBCLASS_BOOT,
};
use crate::platform::String;

use super::usb::{
    enumerate, get_config_blob, hid_control_in, hid_control_nodata, interrupt_in_toggle, port_ccs,
    release_keyboard_fallback, ticks, UsbDevice, TICKS_PER_MILLISECOND,
};

use sha2::{Digest, Sha256};

#[derive(Clone, Copy)]
struct Binding {
    iface: HidInterface,
    parsed: ParsedReportDesc,
    state: DecodeState,
    toggle: u32,
    last_ticks: u64,
    desc_sha: [u8; 32],
    report_len: u16,
}

struct HidHost {
    dev: UsbDevice,
    device_id: u16,
    n: u8,
    bind: [Binding; MAX_HID_INTERFACES],
    ccs: bool,
}

static mut HOST: Option<HidHost> = None;
static mut NEXT_ID: u16 = 1;
const CHAR_PEND: usize = 16;
static mut CHARS: [u8; CHAR_PEND] = [0; CHAR_PEND];
static mut CHAR_HEAD: usize = 0;
static mut CHAR_TAIL: usize = 0;

pub fn hid_host_active() -> bool {
    unsafe { (*core::ptr::addr_of!(HOST)).is_some() }
}

fn interval_ticks(eps: u32, binterval: u8) -> u64 {
    let iv = if binterval == 0 { 1 } else { binterval };
    if eps == 2 {
        let mf = 1u64 << (iv.min(16).saturating_sub(1) as u32);
        mf * (TICKS_PER_MILLISECOND / 8)
    } else {
        iv as u64 * TICKS_PER_MILLISECOND
    }
}

fn push_char(c: u8) {
    unsafe {
        let next = (CHAR_TAIL + 1) % CHAR_PEND;
        if next != CHAR_HEAD {
            CHARS[CHAR_TAIL] = c;
            CHAR_TAIL = next;
        }
    }
}

fn pop_char() -> Option<u8> {
    unsafe {
        if CHAR_HEAD == CHAR_TAIL {
            return None;
        }
        let c = CHARS[CHAR_HEAD];
        CHAR_HEAD = (CHAR_HEAD + 1) % CHAR_PEND;
        Some(c)
    }
}

fn emit(ev: InputEvent) {
    match ev {
        InputEvent::KeyDown { usage, modifiers } => {
            if let Some(c) = usage_to_ascii(usage, modifiers) {
                push_char(c);
            }
        }
        _ => {}
    }
    let _ = push_global(ev);
}

fn detach_locked(host: &mut HidHost) {
    for i in 0..host.n as usize {
        let _ = push_global(InputEvent::DeviceDetached {
            device_id: host.device_id,
            interface: host.bind[i].iface.iface,
        });
    }
}

/// Enumerate all HID interfaces, fetch report descriptors, bind endpoints.
pub fn attach_hid() -> Result<String, &'static str> {
    crate::kernel::hid::queue::clear_global();
    unsafe {
        CHAR_HEAD = 0;
        CHAR_TAIL = 0;
        *core::ptr::addr_of_mut!(HOST) = None;
    }
    release_keyboard_fallback();

    let (dev, _st) = enumerate()?;
    let mut config = [0u8; 512];
    let config_len = get_config_blob(dev.addr, dev.ep0_maxpkt, dev.eps, &mut config)?;
    let mut ifaces = [HidInterface::default(); MAX_HID_INTERFACES];
    let n = parse_config_hid_interfaces(&config[..config_len], &mut ifaces)
        .map_err(|_| "usb hid: no HID interface")?;
    if n == 0 {
        return Err("usb hid: no HID interface");
    }

    let device_id = unsafe {
        let id = NEXT_ID;
        NEXT_ID = NEXT_ID.wrapping_add(1);
        if NEXT_ID == 0 {
            NEXT_ID = 1;
        }
        id
    };

    let mut host = HidHost {
        dev,
        device_id,
        n: 0,
        bind: [Binding {
            iface: HidInterface::default(),
            parsed: ParsedReportDesc::empty(),
            state: DecodeState::new(),
            toggle: 0,
            last_ticks: 0,
            desc_sha: [0; 32],
            report_len: 0,
        }; MAX_HID_INTERFACES],
        ccs: true,
    };

    let mut report = alloc::format!(
        "usb hid: {:04x}:{:04x} addr={} cfg={} ifaces={}\n",
        dev.vid,
        dev.pid,
        dev.addr,
        dev.config_value,
        n
    );

    for k in 0..n {
        let iface = ifaces[k];
        if !endpoint_in_interface(&config[..config_len], iface.iface, iface.ep_in)
            .unwrap_or(false)
        {
            return Err("usb hid: endpoint not in interface");
        }

        let mut hid_d = [0u8; 16];
        let _ = hid_control_in(
            dev.addr,
            dev.ep0_maxpkt,
            dev.eps,
            &setup_get_hid_descriptor(iface.iface, 9),
            &mut hid_d[..9],
        );

        let want = if iface.report_desc_len == 0 {
            256u16
        } else {
            iface.report_desc_len
        };
        if want as usize > MAX_REPORT_DESC {
            return Err("usb hid: report descriptor too large");
        }
        let mut rdesc = [0u8; MAX_REPORT_DESC];
        let got = hid_control_in(
            dev.addr,
            dev.ep0_maxpkt,
            dev.eps,
            &setup_get_report_descriptor(iface.iface, want),
            &mut rdesc[..want as usize],
        )?;
        if got == 0 {
            return Err("usb hid: empty report descriptor");
        }
        let parsed = parse_report_descriptor(&rdesc[..got]).map_err(|e| e.as_str())?;
        let mut hasher = Sha256::new();
        hasher.update(&rdesc[..got]);
        let sha: [u8; 32] = hasher.finalize().into();

        let _ = hid_control_nodata(
            dev.addr,
            dev.ep0_maxpkt,
            dev.eps,
            &setup_set_idle(iface.iface, 0, 0),
        );
        let mut proto_b = [0u8; 1];
        let proto_got = hid_control_in(
            dev.addr,
            dev.ep0_maxpkt,
            dev.eps,
            &setup_get_protocol(iface.iface),
            &mut proto_b,
        );
        let boot = iface.subclass == HID_SUBCLASS_BOOT;
        let _ = hid_control_nodata(
            dev.addr,
            dev.ep0_maxpkt,
            dev.eps,
            &setup_set_protocol(iface.iface, boot),
        );

        let proto_s = match proto_got {
            Ok(1) => {
                if proto_b[0] == 0 {
                    "boot"
                } else {
                    "report"
                }
            }
            _ => "n/a",
        };

        report.push_str(&alloc::format!(
            "  if={} sub={:02x} proto={} ep=0x{:02x} maxpkt={} interval={} hid={:04x} report_len={} get_proto={} sha256=",
            iface.iface,
            iface.subclass,
            iface.protocol,
            iface.ep_in,
            iface.ep_in_maxpkt,
            iface.ep_in_interval,
            iface.hid_bcd,
            got,
            proto_s,
        ));
        for b in sha.iter() {
            report.push_str(&alloc::format!("{:02x}", b));
        }
        report.push('\n');

        host.bind[host.n as usize] = Binding {
            iface,
            parsed,
            state: DecodeState::new(),
            toggle: 0,
            last_ticks: 0,
            desc_sha: sha,
            report_len: got as u16,
        };
        host.n += 1;
        emit(InputEvent::DeviceAttached {
            device_id,
            interface: iface.iface,
        });
    }

    unsafe {
        *core::ptr::addr_of_mut!(HOST) = Some(host);
    }
    Ok(report)
}

fn poll_one(host: &mut HidHost, idx: usize) -> bool {
    let now = ticks();
    let eps = host.dev.eps;
    let iv = interval_ticks(eps, host.bind[idx].iface.ep_in_interval);
    if host.bind[idx].last_ticks != 0 && now.wrapping_sub(host.bind[idx].last_ticks) < iv {
        return false;
    }
    host.bind[idx].last_ticks = now;

    let maxpkt = host.bind[idx].iface.ep_in_maxpkt.max(1);
    let mut buf = [0u8; 64];
    let want = (maxpkt as usize).min(buf.len());
    match interrupt_in_toggle(
        host.dev.addr,
        host.bind[idx].iface.ep_in,
        maxpkt,
        eps,
        &mut buf[..want],
        2,
        &mut host.bind[idx].toggle,
    ) {
        Ok(0) => false,
        Ok(n) => {
            let mut evs = [InputEvent::MouseWheel { delta: 0 }; 16];
            match decode_report(
                &host.bind[idx].parsed,
                &buf[..n],
                &mut host.bind[idx].state,
                &mut evs,
            ) {
                Ok(c) => {
                    for e in evs[..c].iter() {
                        emit(*e);
                    }
                }
                Err(HidError::KeyRollover) => {}
                Err(_) => {}
            }
            false
        }
        Err(_) => !port_ccs(),
    }
}

/// Poll CCS and every bound interrupt-IN. Pushes InputEvents. Does not allocate.
pub fn poll_hid() {
    unsafe {
        let ccs = port_ccs();
        let drop_host = {
            let Some(host) = (*core::ptr::addr_of_mut!(HOST)).as_mut() else {
                return;
            };
            if !ccs {
                detach_locked(host);
                true
            } else {
                host.ccs = true;
                let n = host.n as usize;
                let mut gone = false;
                for i in 0..n {
                    if poll_one(host, i) {
                        gone = true;
                        break;
                    }
                }
                if gone {
                    detach_locked(host);
                }
                gone
            }
        };
        if drop_host {
            *core::ptr::addr_of_mut!(HOST) = None;
        }
    }
    super::usb::rearm_controller_irq();
}

/// Drain one shell character produced from KeyDown usages. Polls HID first.
pub fn poll_hid_char() -> Option<u8> {
    if let Some(c) = pop_char() {
        return Some(c);
    }
    if !hid_host_active() {
        return None;
    }
    poll_hid();
    pop_char()
}

pub fn detach_hid() -> String {
    unsafe {
        if let Some(mut host) = (*core::ptr::addr_of_mut!(HOST)).take() {
            detach_locked(&mut host);
            alloc::format!("usb hid: detached device_id={}", host.device_id)
        } else {
            String::from("usb hid: no device attached")
        }
    }
}

pub fn capture_events(ms: u32) -> String {
    if !hid_host_active() {
        return String::from("usb hid: not attached (run usb hid)");
    }
    let start = ticks();
    let budget = ms as u64 * TICKS_PER_MILLISECOND;
    let mut out = String::from("usb events:\n");
    let mut n = 0u32;
    while ticks().wrapping_sub(start) < budget && n < 48 {
        poll_hid();
        while let Some(ev) = poll_global() {
            out.push_str(&format_event(ev));
            out.push('\n');
            n += 1;
            if n >= 48 {
                break;
            }
        }
    }
    out.push_str(&alloc::format!("usb events: {n} from controller"));
    out
}

fn format_event(ev: InputEvent) -> String {
    match ev {
        InputEvent::KeyDown { usage, modifiers } => {
            alloc::format!("  KeyDown usage=0x{usage:04x} mod=0x{modifiers:02x}")
        }
        InputEvent::KeyUp { usage, modifiers } => {
            alloc::format!("  KeyUp usage=0x{usage:04x} mod=0x{modifiers:02x}")
        }
        InputEvent::MouseMove { dx, dy } => alloc::format!("  MouseMove dx={dx} dy={dy}"),
        InputEvent::MouseButton { button, pressed } => {
            alloc::format!("  MouseButton b={button} pressed={pressed}")
        }
        InputEvent::MouseWheel { delta } => alloc::format!("  MouseWheel delta={delta}"),
        InputEvent::DeviceAttached { device_id, interface } => {
            alloc::format!("  DeviceAttached id={device_id} if={interface}")
        }
        InputEvent::DeviceDetached { device_id, interface } => {
            alloc::format!("  DeviceDetached id={device_id} if={interface}")
        }
    }
}

pub fn hid_neg_report() -> String {
    use crate::kernel::hid::fixtures::{run_negative_suite, NegCase};
    let mut cases = [NegCase {
        name: "",
        result: Ok(()),
    }; 16];
    let n = run_negative_suite(&mut cases);
    let mut out = String::from("usb hid-neg: software fixtures (not hardware)\n");
    let mut bad = 0u32;
    for c in cases[..n].iter() {
        match c.result {
            Err(e) => out.push_str(&alloc::format!("  {} -> rejected ({})\n", c.name, e.as_str())),
            Ok(()) => {
                bad += 1;
                out.push_str(&alloc::format!("  {} -> accepted (unexpected)\n", c.name));
            }
        }
    }
    out.push_str(&alloc::format!(
        "usb hid-neg: {n} cases, {bad} unexpected accept(s)"
    ));
    out
}

pub fn status_line() -> String {
    unsafe {
        match (*core::ptr::addr_of!(HOST)).as_ref() {
            None => String::from("usb hid: inactive"),
            Some(h) => alloc::format!(
                "usb hid: {:04x}:{:04x} id={} ifaces={} ccs={}",
                h.dev.vid,
                h.dev.pid,
                h.device_id,
                h.n,
                h.ccs
            ),
        }
    }
}

pub fn last_report_sha(iface: u8) -> Option<[u8; 32]> {
    unsafe {
        (*core::ptr::addr_of!(HOST)).as_ref().and_then(|h| {
            (0..h.n as usize)
                .find(|&i| h.bind[i].iface.iface == iface)
                .map(|i| h.bind[i].desc_sha)
        })
    }
}

// silence unused until status printers use them
#[allow(dead_code)]
fn _binding_report_len(b: &Binding) -> u16 {
    b.report_len
}

pub fn clear_char_pend() {
    unsafe {
        CHAR_HEAD = 0;
        CHAR_TAIL = 0;
    }
    clear_global();
}
