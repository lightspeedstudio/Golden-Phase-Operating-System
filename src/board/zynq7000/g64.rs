//! G64 research AXI-Lite sidecar (polled). Feature `g64-research` only.
//!
//! Does not reimplement Golden Phase algebra — callers compare to `gphase_core`.

use super::memory_map::G64_AXI_BASE;

pub const EXPECTED_ID: u32 = 0x4736_3453; // "G64S"
pub const EXPECTED_VERSION: u32 = 0x0001_0000;

const REG_ID: u32 = 0x00;
const REG_VERSION: u32 = 0x04;
const REG_CAPABILITIES: u32 = 0x08;
const REG_STATUS: u32 = 0x0C;
const REG_ERROR: u32 = 0x10;
const REG_COMMAND: u32 = 0x14;
const REG_INPUT_A: u32 = 0x1C;
const REG_INPUT_B: u32 = 0x20;
const REG_INPUT_C: u32 = 0x24;
const REG_RESULT_LEFT: u32 = 0x28;
const REG_RESULT_RIGHT: u32 = 0x2C;
const REG_CLASS: u32 = 0x30;
const REG_RESULT_COMPOSE: u32 = 0x34;
const REG_RESULT_REFLECT: u32 = 0x38;
const REG_CYCLE_COUNT: u32 = 0x3C;
const REG_OP_COUNT: u32 = 0x40;
const REG_ID_MASK_LO: u32 = 0x44;
const REG_ID_MASK_HI: u32 = 0x48;
const REG_REF_MASK_LO: u32 = 0x4C;
const REG_REF_MASK_HI: u32 = 0x50;
const REG_ERR_MASK_LO: u32 = 0x54;
const REG_ERR_MASK_HI: u32 = 0x58;
const REG_SLOT_IDX: u32 = 0x5C;
const REG_SLOT_DATA: u32 = 0x60;

pub const OPC_COMPOSE: u32 = 0x01;
pub const OPC_REFLECT: u32 = 0x02;
pub const OPC_TRIPLE: u32 = 0x03;
pub const OPC_ATLAS: u32 = 0x04;
pub const OPC_SOFT_RESET: u32 = 0x06;
pub const OPC_CLEAR_ERR: u32 = 0x07;

pub const ST_BUSY: u32 = 1 << 1;
pub const ST_DONE: u32 = 1 << 2;

pub const DEFAULT_TIMEOUT_SPINS: u32 = 2_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum G64Error {
    Timeout,
    Busy,
    IdentityMismatch { id: u32, version: u32 },
    Unsupported,
    HwError(u32),
}

impl G64Error {
    pub fn as_str(self) -> &'static str {
        match self {
            G64Error::Timeout => "timeout waiting for G64 idle/done",
            G64Error::Busy => "G64 still busy",
            G64Error::IdentityMismatch { .. } => "G64 identity mismatch",
            G64Error::Unsupported => "G64 unsupported opcode",
            G64Error::HwError(_) => "G64 ERROR register set",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasMasks {
    pub identity: u64,
    pub reflection: u64,
    pub error: u64,
}

pub struct G64 {
    base: u32,
}

impl G64 {
    pub const fn at(base: u32) -> Self {
        Self { base }
    }

    pub const fn default_window() -> Self {
        Self::at(G64_AXI_BASE)
    }

    #[inline]
    unsafe fn read(&self, offset: u32) -> u32 {
        core::ptr::read_volatile((self.base + offset) as *const u32)
    }

    #[inline]
    unsafe fn write(&self, offset: u32, value: u32) {
        core::ptr::write_volatile((self.base + offset) as *mut u32, value);
    }

    pub fn id(&self) -> u32 {
        unsafe { self.read(REG_ID) }
    }

    pub fn version(&self) -> u32 {
        unsafe { self.read(REG_VERSION) }
    }

    pub fn capabilities(&self) -> u32 {
        unsafe { self.read(REG_CAPABILITIES) }
    }

    pub fn status(&self) -> u32 {
        unsafe { self.read(REG_STATUS) }
    }

    pub fn error(&self) -> u32 {
        unsafe { self.read(REG_ERROR) }
    }

    pub fn probe_identity(&self) -> Result<(), G64Error> {
        let id = self.id();
        let ver = self.version();
        if id != EXPECTED_ID || ver != EXPECTED_VERSION {
            return Err(G64Error::IdentityMismatch { id, version: ver });
        }
        Ok(())
    }

    fn wait_done(&self, timeout_spins: u32) -> Result<(), G64Error> {
        for _ in 0..timeout_spins {
            let st = self.status();
            if (st & ST_BUSY) == 0 && (st & ST_DONE) != 0 {
                if (st & (1 << 3)) != 0 {
                    return Err(G64Error::Unsupported);
                }
                let err = self.error();
                if err != 0 {
                    return Err(G64Error::HwError(err));
                }
                return Ok(());
            }
        }
        Err(G64Error::Timeout)
    }

    pub fn soft_reset(&self) {
        unsafe { self.write(REG_COMMAND, OPC_SOFT_RESET) };
    }

    pub fn compose(&self, a: u8, b: u8, timeout_spins: u32) -> Result<u8, G64Error> {
        if (self.status() & ST_BUSY) != 0 {
            return Err(G64Error::Busy);
        }
        unsafe {
            self.write(REG_INPUT_A, u32::from(a & 3));
            self.write(REG_INPUT_B, u32::from(b & 3));
            self.write(REG_COMMAND, OPC_COMPOSE);
        }
        self.wait_done(timeout_spins)?;
        Ok((unsafe { self.read(REG_RESULT_COMPOSE) } & 3) as u8)
    }

    pub fn reflect(&self, a: u8, timeout_spins: u32) -> Result<u8, G64Error> {
        if (self.status() & ST_BUSY) != 0 {
            return Err(G64Error::Busy);
        }
        unsafe {
            self.write(REG_INPUT_A, u32::from(a & 3));
            self.write(REG_COMMAND, OPC_REFLECT);
        }
        self.wait_done(timeout_spins)?;
        Ok((unsafe { self.read(REG_RESULT_REFLECT) } & 3) as u8)
    }

    pub fn triple(
        &self,
        a: u8,
        b: u8,
        c: u8,
        timeout_spins: u32,
    ) -> Result<(u8, u8, u8), G64Error> {
        if (self.status() & ST_BUSY) != 0 {
            return Err(G64Error::Busy);
        }
        unsafe {
            self.write(REG_INPUT_A, u32::from(a & 3));
            self.write(REG_INPUT_B, u32::from(b & 3));
            self.write(REG_INPUT_C, u32::from(c & 3));
            self.write(REG_COMMAND, OPC_TRIPLE);
        }
        self.wait_done(timeout_spins)?;
        let left = (unsafe { self.read(REG_RESULT_LEFT) } & 3) as u8;
        let right = (unsafe { self.read(REG_RESULT_RIGHT) } & 3) as u8;
        let class = (unsafe { self.read(REG_CLASS) } & 7) as u8;
        Ok((left, right, class))
    }

    /// Pack slot as `{c,b,a}` in bits [5:0]. Index `i = a*16 + b*4 + c`.
    pub fn load_slot(&self, idx: u8, a: u8, b: u8, c: u8) {
        let pack = u32::from((a & 3) | ((b & 3) << 2) | ((c & 3) << 4));
        unsafe {
            self.write(REG_SLOT_IDX, u32::from(idx));
            self.write(REG_SLOT_DATA, pack);
        }
    }

    pub fn load_full_atlas_slots(&self) {
        let mut idx = 0u8;
        for a in 0u8..4 {
            for b in 0u8..4 {
                for c in 0u8..4 {
                    self.load_slot(idx, a, b, c);
                    idx = idx.wrapping_add(1);
                }
            }
        }
    }

    pub fn atlas(&self, timeout_spins: u32) -> Result<AtlasMasks, G64Error> {
        if (self.status() & ST_BUSY) != 0 {
            return Err(G64Error::Busy);
        }
        unsafe { self.write(REG_COMMAND, OPC_ATLAS) };
        self.wait_done(timeout_spins)?;
        let identity = unsafe {
            u64::from(self.read(REG_ID_MASK_LO)) | (u64::from(self.read(REG_ID_MASK_HI)) << 32)
        };
        let reflection = unsafe {
            u64::from(self.read(REG_REF_MASK_LO)) | (u64::from(self.read(REG_REF_MASK_HI)) << 32)
        };
        let error = unsafe {
            u64::from(self.read(REG_ERR_MASK_LO)) | (u64::from(self.read(REG_ERR_MASK_HI)) << 32)
        };
        let _ = (unsafe { self.read(REG_CYCLE_COUNT) }, unsafe {
            self.read(REG_OP_COUNT)
        });
        Ok(AtlasMasks {
            identity,
            reflection,
            error,
        })
    }
}
