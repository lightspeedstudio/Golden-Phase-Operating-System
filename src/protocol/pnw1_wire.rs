//! PN-WIRE-1 raw EtherType `0x88B5` frame contract (not PhaseNet codec).
//!
//! Shared by PZ7020 GEM (`gem::wire_*`) and the QEMU virtio peer so REQ/REP
//! bytes match exactly. This is L2 proof framing only.

pub const ETHERTYPE: u16 = 0x88B5;
pub const FRAME_LEN: usize = 64;
pub const BOARD_MAC: [u8; 6] = [0x02, 0x00, 0x00, 0x47, 0x01, 0x50];
pub const MAGIC: [u8; 4] = *b"PNW1";
pub const KIND_REQ: u8 = 0x01;
pub const KIND_REP: u8 = 0x02;
pub const COOKIE: [u8; 8] = [0xC0, 0xFF, 0xEE, 0x11, 0x22, 0x33, 0x44, 0x55];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pnw1Req {
    pub seq: u8,
}

/// Build a PNW1 REQ (broadcast) or REP (to board MAC) of fixed [`FRAME_LEN`].
pub fn build_frame(buf: &mut [u8], kind: u8, seq: u8, src_mac: &[u8; 6]) -> usize {
    assert!(buf.len() >= FRAME_LEN);
    for b in buf.iter_mut().take(FRAME_LEN) {
        *b = 0;
    }
    if kind == KIND_REQ {
        for b in buf.iter_mut().take(6) {
            *b = 0xFF;
        }
    } else {
        buf[0..6].copy_from_slice(&BOARD_MAC);
    }
    buf[6..12].copy_from_slice(src_mac);
    buf[12] = (ETHERTYPE >> 8) as u8;
    buf[13] = (ETHERTYPE & 0xFF) as u8;
    buf[14..18].copy_from_slice(&MAGIC);
    buf[18] = kind;
    buf[19] = seq;
    buf[20..28].copy_from_slice(&COOKIE);
    for (i, b) in buf[28..FRAME_LEN].iter_mut().enumerate() {
        *b = (0xA0 + i) as u8;
    }
    FRAME_LEN
}

pub fn expected_payload(kind: u8, seq: u8) -> [u8; FRAME_LEN - 14] {
    let mut p = [0u8; FRAME_LEN - 14];
    p[0..4].copy_from_slice(&MAGIC);
    p[4] = kind;
    p[5] = seq;
    p[6..14].copy_from_slice(&COOKIE);
    for (i, b) in p[14..].iter_mut().enumerate() {
        *b = (0xA0 + i) as u8;
    }
    p
}

/// Parse a PNW1 REQ from the board. Returns `None` if not a matching REQ.
pub fn parse_board_req(frame: &[u8]) -> Option<Pnw1Req> {
    if frame.len() < FRAME_LEN {
        return None;
    }
    let et = ((frame[12] as u16) << 8) | frame[13] as u16;
    if et != ETHERTYPE {
        return None;
    }
    if frame[6..12] != BOARD_MAC {
        return None;
    }
    let expect = expected_payload(KIND_REQ, frame[19]);
    if frame[14..FRAME_LEN] != expect {
        return None;
    }
    Some(Pnw1Req { seq: frame[19] })
}

/// Build controlled REP for a parsed REQ.
pub fn build_rep(buf: &mut [u8], seq: u8, peer_mac: &[u8; 6]) -> usize {
    build_frame(buf, KIND_REP, seq, peer_mac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_roundtrip_parse() {
        let mut buf = [0u8; FRAME_LEN];
        let n = build_frame(&mut buf, KIND_REQ, 7, &BOARD_MAC);
        assert_eq!(n, FRAME_LEN);
        let req = parse_board_req(&buf).expect("parse");
        assert_eq!(req.seq, 7);
        let mut rep = [0u8; FRAME_LEN];
        let rn = build_rep(&mut rep, req.seq, &[0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
        assert_eq!(rn, FRAME_LEN);
        assert_eq!(&rep[14..FRAME_LEN], &expected_payload(KIND_REP, 7));
        assert_eq!(&rep[0..6], &BOARD_MAC);
    }
}
