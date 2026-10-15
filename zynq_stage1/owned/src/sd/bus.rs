// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! SD0 byte/half/word bus. Device MMIO or host mock.

pub const SD0: u32 = 0xE010_0000;

pub const BLK_SIZE: u32 = 0x04;
pub const BLK_CNT: u32 = 0x06;
pub const ARG: u32 = 0x08;
pub const XFER: u32 = 0x0C;
pub const RESP0: u32 = 0x10;
pub const RESP1: u32 = 0x14;
pub const RESP2: u32 = 0x18;
pub const RESP3: u32 = 0x1C;
pub const BUF: u32 = 0x20;
pub const PRES: u32 = 0x24;
pub const HOST: u32 = 0x28;
pub const POWER: u32 = 0x29;
pub const CLK: u32 = 0x2C;
pub const TIMEOUT: u32 = 0x2E;
pub const SWRST: u32 = 0x2F;
pub const NISTR: u32 = 0x30;
pub const EISTR: u32 = 0x32;
pub const NISTER: u32 = 0x34;
pub const EISTER: u32 = 0x36;
pub const ADMA_ERR: u32 = 0x54;
pub const ADMA_SAR: u32 = 0x58;

pub trait SdBus {
    fn r8(&mut self, off: u32) -> u8;
    fn w8(&mut self, off: u32, v: u8);
    fn r16(&mut self, off: u32) -> u16;
    fn w16(&mut self, off: u32, v: u16);
    fn r32(&mut self, off: u32) -> u32;
    fn w32(&mut self, off: u32, v: u32);
    fn delay(&mut self, loops: u32);
    fn phys_write(&mut self, addr: u32, bytes: &[u8]);
    fn phys_read(&mut self, addr: u32, out: &mut [u8]);
}

#[cfg(not(feature = "std"))]
pub struct DeviceSd;

#[cfg(not(feature = "std"))]
impl SdBus for DeviceSd {
    fn r8(&mut self, off: u32) -> u8 {
        unsafe { core::ptr::read_volatile((SD0 + off) as *const u8) }
    }
    fn w8(&mut self, off: u32, v: u8) {
        unsafe { core::ptr::write_volatile((SD0 + off) as *mut u8, v) }
    }
    fn r16(&mut self, off: u32) -> u16 {
        unsafe { core::ptr::read_volatile((SD0 + off) as *const u16) }
    }
    fn w16(&mut self, off: u32, v: u16) {
        unsafe { core::ptr::write_volatile((SD0 + off) as *mut u16, v) }
    }
    fn r32(&mut self, off: u32) -> u32 {
        unsafe { core::ptr::read_volatile((SD0 + off) as *const u32) }
    }
    fn w32(&mut self, off: u32, v: u32) {
        unsafe { core::ptr::write_volatile((SD0 + off) as *mut u32, v) }
    }
    fn delay(&mut self, loops: u32) {
        for _ in 0..loops {
            core::hint::spin_loop();
        }
    }
    fn phys_write(&mut self, addr: u32, bytes: &[u8]) {
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), addr as *mut u8, bytes.len());
        }
    }
    fn phys_read(&mut self, addr: u32, out: &mut [u8]) {
        unsafe {
            core::ptr::copy_nonoverlapping(addr as *const u8, out.as_mut_ptr(), out.len());
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceW {
    pub off: u32,
    pub width: u8,
    pub val: u32,
}

#[cfg(feature = "std")]
pub struct HostSd {
    pub mem: [u8; 0x80],
    pub rca: u16,
    pub ocr_ready: bool,
    pub sectors: std::collections::BTreeMap<u32, [u8; 512]>,
    pub dest: std::collections::BTreeMap<u32, u8>,
    pub trace: Vec<TraceW>,
    pub cmds: Vec<(u8, u32, u16, u16)>,
    pub pio_buf_reads: u32,
    pub adma_used: bool,
}

#[cfg(feature = "std")]
impl HostSd {
    pub fn new() -> Self {
        let mut s = Self {
            mem: [0; 0x80],
            rca: 0x1234,
            ocr_ready: true,
            sectors: std::collections::BTreeMap::new(),
            dest: std::collections::BTreeMap::new(),
            trace: Vec::new(),
            cmds: Vec::new(),
            pio_buf_reads: 0,
            adma_used: false,
        };
        s.mem[PRES as usize] = 0;
        s
    }

    fn rec(&mut self, off: u32, width: u8, val: u32) {
        self.trace.push(TraceW { off, width, val });
    }

    fn put32(&mut self, off: u32, v: u32) {
        let o = off as usize;
        self.mem[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }

    fn get32(&self, off: u32) -> u32 {
        let o = off as usize;
        u32::from_le_bytes(self.mem[o..o + 4].try_into().unwrap())
    }

    fn handle_cmd(&mut self, xfer_cmd: u32) {
        let xfer = (xfer_cmd & 0xFFFF) as u16;
        let cmdw = (xfer_cmd >> 16) as u16;
        let index = (cmdw >> 8) as u8;
        let flags = cmdw & 0xFF;
        let arg = self.get32(ARG);
        self.cmds.push((index, arg, flags, xfer));
        match index {
            8 => self.put32(RESP0, 0x0000_01AA),
            41 => self.put32(RESP0, 0xC030_0000),
            3 => self.put32(RESP0, (self.rca as u32) << 16),
            9 => {
                self.put32(RESP0, 0);
                self.put32(RESP1, 0x003B_FF00);
                self.put32(RESP2, 0);
                self.put32(RESP3, 0x0040_0000);
            }
            18 => {
                self.adma_used = true;
                self.run_adma();
                self.mem[NISTR as usize] |= 0x02;
            }
            _ => {}
        }
        self.mem[NISTR as usize] |= 0x01;
        self.mem[SWRST as usize] = 0;
        self.mem[(CLK as usize) + 1] |= 0x00;
        let clk0 = self.mem[CLK as usize];
        self.mem[CLK as usize] = clk0 | 0x02;
    }

    fn run_adma(&mut self) {
        let mut sar = self.get32(ADMA_SAR);
        let blk = u16::from_le_bytes([self.mem[BLK_SIZE as usize], self.mem[BLK_SIZE as usize + 1]])
            as u32;
        let cnt = u16::from_le_bytes([self.mem[BLK_CNT as usize], self.mem[BLK_CNT as usize + 1]])
            as u32;
        let mut lba = self.get32(ARG);
        let mut remain = blk.saturating_mul(cnt);
        for _ in 0..8 {
            let attr = self.dest_u16(sar);
            let len = self.dest_u16(sar + 2) as u32;
            let addr = self.dest_u32(sar + 4);
            let n = if len == 0 { 65536 } else { len };
            let take = core::cmp::min(n, remain);
            let mut off = 0u32;
            while off < take {
                let sec = *self.sectors.get(&lba).unwrap_or(&[0u8; 512]);
                let chunk = core::cmp::min(512, (take - off) as usize);
                for i in 0..chunk {
                    self.dest.insert(addr + off + i as u32, sec[i]);
                }
                off += chunk as u32;
                lba = lba.wrapping_add(1);
            }
            remain = remain.saturating_sub(take);
            if (attr & 2) != 0 {
                break;
            }
            sar = sar.wrapping_add(8);
        }
    }

    fn dest_u16(&self, a: u32) -> u16 {
        u16::from_le_bytes([
            *self.dest.get(&a).unwrap_or(&0),
            *self.dest.get(&(a + 1)).unwrap_or(&0),
        ])
    }

    fn dest_u32(&self, a: u32) -> u32 {
        u32::from_le_bytes([
            *self.dest.get(&a).unwrap_or(&0),
            *self.dest.get(&(a + 1)).unwrap_or(&0),
            *self.dest.get(&(a + 2)).unwrap_or(&0),
            *self.dest.get(&(a + 3)).unwrap_or(&0),
        ])
    }
}

#[cfg(feature = "std")]
impl SdBus for HostSd {
    fn r8(&mut self, off: u32) -> u8 {
        if off == SWRST {
            self.mem[off as usize] = 0;
        }
        self.mem[off as usize]
    }
    fn w8(&mut self, off: u32, v: u8) {
        self.rec(off, 8, v as u32);
        self.mem[off as usize] = v;
        if off == SWRST {
            self.mem[off as usize] = 0;
        }
    }
    fn r16(&mut self, off: u32) -> u16 {
        if off == CLK {
            let c = u16::from_le_bytes([self.mem[off as usize], self.mem[off as usize + 1]]);
            return c | 0x0002;
        }
        u16::from_le_bytes([self.mem[off as usize], self.mem[off as usize + 1]])
    }
    fn w16(&mut self, off: u32, v: u16) {
        self.rec(off, 16, v as u32);
        if off == NISTR || off == EISTR {
            let cur = u16::from_le_bytes([self.mem[off as usize], self.mem[off as usize + 1]]);
            let nxt = cur & !v;
            let b = nxt.to_le_bytes();
            self.mem[off as usize] = b[0];
            self.mem[off as usize + 1] = b[1];
            return;
        }
        let b = v.to_le_bytes();
        self.mem[off as usize] = b[0];
        self.mem[off as usize + 1] = b[1];
    }
    fn r32(&mut self, off: u32) -> u32 {
        if off == BUF {
            self.pio_buf_reads += 1;
        }
        if off == PRES {
            return 0;
        }
        self.get32(off)
    }
    fn w32(&mut self, off: u32, v: u32) {
        self.rec(off, 32, v);
        self.put32(off, v);
        if off == XFER {
            self.handle_cmd(v);
        }
        if off == ADMA_SAR {
            self.adma_used = true;
        }
    }
    fn delay(&mut self, loops: u32) {
        self.rec(0xFFFF_FFFF, 0, loops);
    }
    fn phys_write(&mut self, addr: u32, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.dest.insert(addr + i as u32, *b);
        }
    }
    fn phys_read(&mut self, addr: u32, out: &mut [u8]) {
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = *self.dest.get(&(addr + i as u32)).unwrap_or(&0);
        }
    }
}
