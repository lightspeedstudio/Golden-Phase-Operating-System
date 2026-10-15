//! USB 2.0 hub class parser and bounded topology (host-testable).
//!
//! Does not talk to EHCI. The PZ7020 host driver uses these checks before
//! allocating addresses or issuing port reset/power.

pub const CLASS_HUB: u8 = 0x09;
pub const DESC_HUB: u8 = 0x29;
pub const MAX_HUB_PORTS: u8 = 8;
pub const MAX_HUB_DEPTH: u8 = 2;
pub const MAX_CHILDREN: u8 = 4;

pub const FEAT_PORT_RESET: u16 = 4;
pub const FEAT_PORT_POWER: u16 = 8;
pub const FEAT_C_PORT_CONNECTION: u16 = 16;
pub const FEAT_C_PORT_RESET: u16 = 20;

pub const PORT_CONNECTION: u16 = 1 << 0;
pub const PORT_ENABLE: u16 = 1 << 1;
pub const PORT_OVERCURRENT: u16 = 1 << 3;
pub const PORT_RESET: u16 = 1 << 4;
pub const PORT_POWER: u16 = 1 << 8;
pub const PORT_LOW_SPEED: u16 = 1 << 9;
pub const PORT_HIGH_SPEED: u16 = 1 << 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubError {
    Truncated,
    BadType,
    NoPorts,
    TooManyPorts,
    Depth,
    Overcurrent,
    Malformed,
}

impl HubError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Truncated => "hub: truncated descriptor",
            Self::BadType => "hub: bad descriptor type",
            Self::NoPorts => "hub: bNbrPorts is zero",
            Self::TooManyPorts => "hub: bNbrPorts exceeds bound",
            Self::Depth => "hub: nested hub refused (depth bound)",
            Self::Overcurrent => "hub: overcurrent",
            Self::Malformed => "hub: malformed descriptor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HubDescriptor {
    pub ports: u8,
    pub characteristics: u16,
    pub pwr_on_2_pwr_good: u8,
    pub hub_current: u8,
}

pub fn parse_hub_descriptor(blob: &[u8]) -> Result<HubDescriptor, HubError> {
    if blob.len() < 7 {
        return Err(HubError::Truncated);
    }
    let blen = blob[0] as usize;
    if blen < 7 || blen > blob.len() {
        return Err(HubError::Malformed);
    }
    if blob[1] != DESC_HUB {
        return Err(HubError::BadType);
    }
    let ports = blob[2];
    if ports == 0 {
        return Err(HubError::NoPorts);
    }
    if ports > MAX_HUB_PORTS {
        return Err(HubError::TooManyPorts);
    }
    Ok(HubDescriptor {
        ports,
        characteristics: u16::from_le_bytes([blob[3], blob[4]]),
        pwr_on_2_pwr_good: blob[5],
        hub_current: blob[6],
    })
}

pub fn port_status_speed(status: u16) -> u32 {
    if (status & PORT_HIGH_SPEED) != 0 {
        2
    } else if (status & PORT_LOW_SPEED) != 0 {
        1
    } else {
        0
    }
}

pub fn port_connected(status: u16) -> bool {
    (status & PORT_CONNECTION) != 0
}

pub fn port_overcurrent(status: u16) -> bool {
    (status & PORT_OVERCURRENT) != 0
}

pub fn refuse_nested(depth: u8) -> Result<(), HubError> {
    if depth > MAX_HUB_DEPTH {
        Err(HubError::Depth)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_4port() -> [u8; 9] {
        [
            9, DESC_HUB, 4, 0x00, 0x00, 50, 100, 0x00, 0xFF,
        ]
    }

    #[test]
    fn parses_standard_hub_descriptor() {
        let d = parse_hub_descriptor(&spec_4port()).unwrap();
        assert_eq!(d.ports, 4);
        assert_eq!(d.pwr_on_2_pwr_good, 50);
    }

    #[test]
    fn refuses_malformed() {
        assert_eq!(parse_hub_descriptor(&[2, DESC_HUB]), Err(HubError::Truncated));
        assert_eq!(
            parse_hub_descriptor(&[9, 0x01, 4, 0, 0, 1, 1, 0, 0]),
            Err(HubError::BadType)
        );
        let mut z = spec_4port();
        z[2] = 0;
        assert_eq!(parse_hub_descriptor(&z), Err(HubError::NoPorts));
        z[2] = 9;
        assert_eq!(parse_hub_descriptor(&z), Err(HubError::TooManyPorts));
        z[0] = 3;
        z[2] = 4;
        assert_eq!(parse_hub_descriptor(&z), Err(HubError::Malformed));
    }

    #[test]
    fn nested_hub_refused() {
        assert!(refuse_nested(1).is_ok());
        assert!(refuse_nested(2).is_ok());
        assert_eq!(refuse_nested(3), Err(HubError::Depth));
    }

    #[test]
    fn overcurrent_detected() {
        assert!(port_overcurrent(PORT_OVERCURRENT | PORT_CONNECTION));
        assert!(!port_overcurrent(PORT_CONNECTION));
    }
}
