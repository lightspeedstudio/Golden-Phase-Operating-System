//! Keyboard and relative-mouse report decoders. Usages, not characters.

use super::report::{field_value, payload_and_id, ParsedReportDesc, ReportField};
use super::types::{
    HidError, InputEvent, KEY_ERROR_ROLLOVER, KEY_ERROR_UNDEFINED, KEY_MOD_MAX, KEY_MOD_MIN,
    KEY_POST_FAIL, MAX_KEYS_DOWN, PAGE_BUTTON, PAGE_GENERIC_DESKTOP, PAGE_KEYBOARD, USAGE_WHEEL,
    USAGE_X, USAGE_Y,
};

#[derive(Clone, Copy)]
pub struct DecodeState {
    pub keys: [u16; MAX_KEYS_DOWN],
    pub n_keys: u8,
    pub modifiers: u8,
    pub buttons: u8,
}

impl DecodeState {
    pub const fn new() -> Self {
        Self {
            keys: [0; MAX_KEYS_DOWN],
            n_keys: 0,
            modifiers: 0,
            buttons: 0,
        }
    }
}

fn is_rollover(usage: u16) -> bool {
    usage == KEY_ERROR_ROLLOVER || usage == KEY_POST_FAIL || usage == KEY_ERROR_UNDEFINED
}

fn push_ev(out: &mut [InputEvent], n: &mut usize, ev: InputEvent) {
    if *n < out.len() {
        out[*n] = ev;
        *n += 1;
    }
}

fn keys_contain(keys: &[u16], n: u8, usage: u16) -> bool {
    keys[..n as usize].iter().any(|&k| k == usage)
}

fn add_key(keys: &mut [u16; MAX_KEYS_DOWN], n: &mut u8, usage: u16) {
    if usage == 0 || keys_contain(keys, *n, usage) {
        return;
    }
    if (*n as usize) < MAX_KEYS_DOWN {
        keys[*n as usize] = usage;
        *n += 1;
    }
}

/// Decode one interrupt-IN report into InputEvents. Updates `state` for edges.
pub fn decode_report(
    parsed: &ParsedReportDesc,
    report: &[u8],
    state: &mut DecodeState,
    out: &mut [InputEvent],
) -> Result<usize, HidError> {
    let (rid, payload) = payload_and_id(parsed, report)?;
    let need = parsed.input_byte_len(rid);
    let payload_need = if parsed.has_report_ids {
        need.saturating_sub(1)
    } else {
        need
    };
    if payload.len() < payload_need {
        return Err(HidError::ReportTooShort);
    }

    let mut n_out = 0usize;
    let mut new_keys = [0u16; MAX_KEYS_DOWN];
    let mut n_new = 0u8;
    let mut new_mods = 0u8;
    let mut new_buttons = 0u8;
    let mut dx = 0i16;
    let mut dy = 0i16;
    let mut wheel = 0i8;
    let mut saw_mouse = false;
    let mut saw_keys = false;

    for i in 0..parsed.n_fields as usize {
        let f = parsed.fields[i];
        if f.report_id != rid {
            continue;
        }
        if f.is_const() {
            continue;
        }
        match f.usage_page {
            PAGE_KEYBOARD => {
                decode_keyboard_field(payload, &f, &mut new_keys, &mut n_new, &mut new_mods, &mut saw_keys)?;
            }
            PAGE_BUTTON => {
                decode_buttons(payload, &f, &mut new_buttons)?;
                saw_mouse = true;
            }
            PAGE_GENERIC_DESKTOP => {
                if decode_pointer(payload, &f, &mut dx, &mut dy, &mut wheel)? {
                    saw_mouse = true;
                }
            }
            _ => {}
        }
    }

    if saw_keys {
        for i in 0..n_new as usize {
            if is_rollover(new_keys[i]) {
                return Err(HidError::KeyRollover);
            }
        }
        let mods = new_mods;
        for bit in 0..8u8 {
            let was = (state.modifiers >> bit) & 1;
            let now = (mods >> bit) & 1;
            let usage = KEY_MOD_MIN + bit as u16;
            if was == 0 && now == 1 {
                push_ev(out, &mut n_out, InputEvent::KeyDown { usage, modifiers: mods });
            } else if was == 1 && now == 0 {
                push_ev(out, &mut n_out, InputEvent::KeyUp { usage, modifiers: mods });
            }
        }
        for i in 0..n_new as usize {
            let u = new_keys[i];
            if u == 0 {
                continue;
            }
            if !keys_contain(&state.keys, state.n_keys, u) {
                push_ev(
                    out,
                    &mut n_out,
                    InputEvent::KeyDown {
                        usage: u,
                        modifiers: mods,
                    },
                );
            }
        }
        for i in 0..state.n_keys as usize {
            let u = state.keys[i];
            if u != 0 && !keys_contain(&new_keys, n_new, u) {
                push_ev(
                    out,
                    &mut n_out,
                    InputEvent::KeyUp {
                        usage: u,
                        modifiers: mods,
                    },
                );
            }
        }
        state.keys = new_keys;
        state.n_keys = n_new;
        state.modifiers = mods;
    }

    if saw_mouse {
        if dx != 0 || dy != 0 {
            push_ev(out, &mut n_out, InputEvent::MouseMove { dx, dy });
        }
        if wheel != 0 {
            push_ev(out, &mut n_out, InputEvent::MouseWheel { delta: wheel });
        }
        for b in 1..=8u8 {
            let mask = 1u8 << (b - 1);
            let was = (state.buttons & mask) != 0;
            let now = (new_buttons & mask) != 0;
            if was != now {
                push_ev(
                    out,
                    &mut n_out,
                    InputEvent::MouseButton {
                        button: b,
                        pressed: now,
                    },
                );
            }
        }
        state.buttons = new_buttons;
    }

    Ok(n_out)
}

fn decode_keyboard_field(
    payload: &[u8],
    f: &ReportField,
    keys: &mut [u16; MAX_KEYS_DOWN],
    n_keys: &mut u8,
    mods: &mut u8,
    saw: &mut bool,
) -> Result<(), HidError> {
    *saw = true;
    if f.is_variable() && f.bit_size == 1 && f.usage_min >= KEY_MOD_MIN && f.usage_max <= KEY_MOD_MAX
    {
        for i in 0..f.count {
            let v = field_value(payload, f, i)?;
            if v != 0 {
                let usage = f.usage_min.saturating_add(i);
                if usage >= KEY_MOD_MIN && usage <= KEY_MOD_MAX {
                    *mods |= 1 << (usage - KEY_MOD_MIN);
                }
            }
        }
        return Ok(());
    }
    if !f.is_variable() {
        for i in 0..f.count {
            let v = field_value(payload, f, i)?;
            if v < 0 {
                return Err(HidError::ImpossibleSignedWidth);
            }
            add_key(keys, n_keys, v as u16);
        }
        return Ok(());
    }
    for i in 0..f.count {
        let v = field_value(payload, f, i)?;
        if v != 0 {
            add_key(keys, n_keys, f.usage_min.saturating_add(i));
        }
    }
    Ok(())
}

fn decode_buttons(payload: &[u8], f: &ReportField, buttons: &mut u8) -> Result<(), HidError> {
    for i in 0..f.count {
        let v = field_value(payload, f, i)?;
        if v != 0 {
            let btn = f.usage_min.saturating_add(i);
            if btn >= 1 && btn <= 8 {
                *buttons |= 1 << (btn - 1);
            }
        }
    }
    Ok(())
}

fn decode_pointer(
    payload: &[u8],
    f: &ReportField,
    dx: &mut i16,
    dy: &mut i16,
    wheel: &mut i8,
) -> Result<bool, HidError> {
    let mut used = false;
    if f.count == 0 {
        return Ok(false);
    }
    if f.usage_min == USAGE_X || f.usage_max == USAGE_X || (f.usage_min <= USAGE_X && f.usage_max >= USAGE_X)
    {
        // Packed X,Y,wheel as consecutive usages with count 3 is the boot mouse pattern.
        if f.count >= 1 && f.usage_min == USAGE_X {
            *dx = field_value(payload, f, 0)? as i16;
            used = true;
            if f.count >= 2 {
                *dy = field_value(payload, f, 1)? as i16;
            }
            if f.count >= 3 && (f.usage_max == USAGE_WHEEL || f.usage_min + 2 == USAGE_WHEEL) {
                *wheel = field_value(payload, f, 2)? as i8;
            }
            return Ok(used);
        }
    }
    if f.usage_min == USAGE_X && f.count == 1 {
        *dx = field_value(payload, f, 0)? as i16;
        return Ok(true);
    }
    if f.usage_min == USAGE_Y && f.count == 1 {
        *dy = field_value(payload, f, 0)? as i16;
        return Ok(true);
    }
    if f.usage_min == USAGE_WHEEL && f.count == 1 {
        *wheel = field_value(payload, f, 0)? as i8;
        return Ok(true);
    }
    Ok(used)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::hid::fixtures::{BOOT_KEYBOARD_DESC, RELATIVE_MOUSE_DESC};
    use crate::kernel::hid::report::parse_report_descriptor;

    #[test]
    fn modifier_edges() {
        let p = parse_report_descriptor(BOOT_KEYBOARD_DESC).unwrap();
        let mut st = DecodeState::new();
        let mut ev = [InputEvent::MouseWheel { delta: 0 }; 16];
        let n = decode_report(&p, &[0x02, 0, 0, 0, 0, 0, 0, 0], &mut st, &mut ev).unwrap();
        assert!(ev[..n].iter().any(|e| matches!(e, InputEvent::KeyDown { usage: 0xE1, .. })));
    }

    #[test]
    fn mouse_release() {
        let p = parse_report_descriptor(RELATIVE_MOUSE_DESC).unwrap();
        let mut st = DecodeState::new();
        let mut ev = [InputEvent::MouseWheel { delta: 0 }; 16];
        decode_report(&p, &[0x01, 0, 0, 0], &mut st, &mut ev).unwrap();
        let n = decode_report(&p, &[0x00, 0, 0, 0], &mut st, &mut ev).unwrap();
        assert!(ev[..n].iter().any(|e| matches!(e, InputEvent::MouseButton { button: 1, pressed: false })));
    }
}
