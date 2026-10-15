//! Character/layout translation. Not part of HID transport.

/// US QWERTY: HID keyboard usage → ASCII. `modifiers` bit 1 or 5 = Shift.
pub fn usage_to_ascii(usage: u16, modifiers: u8) -> Option<u8> {
    let shift = (modifiers & 0x22) != 0;
    let code = usage as u8;
    if usage > 0xFF {
        return None;
    }
    let c = match code {
        0x04..=0x1d => {
            let base = b'a' + (code - 0x04);
            if shift {
                base - 32
            } else {
                base
            }
        }
        0x1e..=0x26 => {
            if shift {
                match code {
                    0x1e => b'!',
                    0x1f => b'@',
                    0x20 => b'#',
                    0x21 => b'$',
                    0x22 => b'%',
                    0x23 => b'^',
                    0x24 => b'&',
                    0x25 => b'*',
                    0x26 => b'(',
                    _ => return None,
                }
            } else {
                b'1' + (code - 0x1e)
            }
        }
        0x27 => {
            if shift {
                b')'
            } else {
                b'0'
            }
        }
        0x28 => b'\n',
        0x2a => 0x08,
        0x2b => b'\t',
        0x2c => b' ',
        0x2d => {
            if shift {
                b'_'
            } else {
                b'-'
            }
        }
        0x2e => {
            if shift {
                b'+'
            } else {
                b'='
            }
        }
        0x2f => {
            if shift {
                b'{'
            } else {
                b'['
            }
        }
        0x30 => {
            if shift {
                b'}'
            } else {
                b']'
            }
        }
        0x31 => {
            if shift {
                b'|'
            } else {
                b'\\'
            }
        }
        0x33 => {
            if shift {
                b':'
            } else {
                b';'
            }
        }
        0x34 => {
            if shift {
                b'"'
            } else {
                b'\''
            }
        }
        0x35 => {
            if shift {
                b'~'
            } else {
                b'`'
            }
        }
        0x36 => {
            if shift {
                b'<'
            } else {
                b','
            }
        }
        0x37 => {
            if shift {
                b'>'
            } else {
                b'.'
            }
        }
        0x38 => {
            if shift {
                b'?'
            } else {
                b'/'
            }
        }
        _ => return None,
    };
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_shift() {
        assert_eq!(usage_to_ascii(0x04, 0), Some(b'a'));
        assert_eq!(usage_to_ascii(0x04, 0x02), Some(b'A'));
        assert_eq!(usage_to_ascii(0x28, 0), Some(b'\n'));
    }
}
