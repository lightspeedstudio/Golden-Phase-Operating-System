//! G1P AXI-Lite polled board adapter (Bring-up V2 register map).
//!
//! Hardware window: `0x40000000` (`G1P_AXI_BASE`). Does not reimplement
//! Golden Phase algebra — callers compare against `gphase_core`.

use super::memory_map::G1P_AXI_BASE;
use crate::core::gphase_core::PhaseState;
use crate::kernel::phase_consensus::{
    ConsensusError, ConsensusMode, G1pBus, HwTrace, PhaseAccelerator, regs,
};

/// Identity / version / capabilities from REGISTER_MAP.md.
pub const EXPECTED_ID: u32 = regs::EXPECTED_ID;
pub const EXPECTED_VERSION: u32 = regs::EXPECTED_VERSION;
pub const EXPECTED_CAPABILITIES: u32 = regs::EXPECTED_CAPABILITIES;

/// GIC SPI ID from SDT `g1p_irq_id.h` (`interrupts = <0 29 4>` → ID 61).
pub const G1P_IRQ_ID: u32 = 61;
pub const G1P_IRQ_SPI: u32 = 29;

const REG_ID: u32 = regs::ID;
const REG_VERSION: u32 = regs::VERSION;
const REG_INPUT_A: u32 = regs::INPUT_A;
const REG_INPUT_B: u32 = regs::INPUT_B;
const REG_INPUT_C: u32 = regs::INPUT_C;
const REG_COMPOSE_AB: u32 = regs::COMPOSE_AB;
const REG_LEFT_RESULT: u32 = regs::LEFT_RESULT;
const REG_RIGHT_RESULT: u32 = regs::RIGHT_RESULT;
const REG_PARITY: u32 = regs::PARITY;
const REG_REFLECT_RESULT: u32 = regs::REFLECT_RESULT;
const REG_STATUS: u32 = regs::STATUS;
const REG_COMMAND: u32 = regs::COMMAND;
const REG_CAPABILITIES: u32 = regs::CAPABILITIES;
const REG_IRQ_ENABLE: u32 = 0x40;
const REG_IRQ_PENDING: u32 = 0x44;
const REG_IRQ_ACK: u32 = regs::IRQ_ACK;
const REG_ERROR_CLEAR: u32 = regs::ERROR_CLEAR;
const REG_TRACE_HASH: u32 = regs::TRACE_HASH;
const REG_TRACE_META: u32 = regs::TRACE_META;

const OPC_NOP: u32 = regs::OPC_NOP;
const OPC_EXEC: u32 = regs::OPC_EXEC;

pub const ST_DONE: u32 = regs::ST_DONE;
pub const ST_BUSY: u32 = regs::ST_BUSY;
pub const ST_INPUT_ERR: u32 = regs::ST_INPUT_ERR;
pub const ST_RESET_SEEN: u32 = regs::ST_RESET_SEEN;
pub const ST_OPCODE_ERR: u32 = regs::ST_OPCODE_ERR;

pub const IRQ_PEND_DONE: u32 = regs::IRQ_PEND_DONE;
pub const IRQ_PEND_ERR: u32 = regs::IRQ_PEND_ERR;

/// Default busy-wait budget (combinational capture is fast; bound hangs).
pub const DEFAULT_TIMEOUT_SPINS: u32 = crate::kernel::phase_consensus::DEFAULT_TIMEOUT_SPINS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum G1pError {
    Timeout,
    Busy,
    InputError,
    OpcodeError,
    NotDone,
    IdentityMismatch { id: u32, version: u32, caps: u32 },
}

impl G1pError {
    pub fn as_str(self) -> &'static str {
        match self {
            G1pError::Timeout => "timeout waiting for G1P idle/done",
            G1pError::Busy => "G1P still busy",
            G1pError::InputError => "G1P input_error latched",
            G1pError::OpcodeError => "G1P opcode_error latched",
            G1pError::NotDone => "G1P STATUS.done not set after EXEC",
            G1pError::IdentityMismatch { .. } => "G1P identity mismatch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecResult {
    pub compose_ab: u32,
    pub left: u32,
    pub right: u32,
    pub parity: u32,
    pub reflect: u32,
    pub status: u32,
    pub hash: u32,
    pub cycle: u16,
}

/// PS MMIO window at `G1P_AXI_BASE`.
pub struct MmioBus {
    base: u32,
}

impl G1pBus for MmioBus {
    fn read(&self, offset: u32) -> u32 {
        unsafe { core::ptr::read_volatile((self.base + offset) as *const u32) }
    }

    fn write(&self, offset: u32, value: u32) {
        unsafe { core::ptr::write_volatile((self.base + offset) as *mut u32, value) }
    }
}

/// Polled MMIO handle for one G1P AXI-Lite window.
pub struct G1p<B: G1pBus = MmioBus> {
    bus: B,
}

impl G1p<MmioBus> {
    pub const fn at(base: u32) -> Self {
        Self {
            bus: MmioBus { base },
        }
    }

    pub const fn default_window() -> Self {
        Self::at(G1P_AXI_BASE)
    }
}

impl<B: G1pBus> G1p<B> {
    #[inline]
    fn read(&self, offset: u32) -> u32 {
        self.bus.read(offset)
    }

    #[inline]
    fn write(&self, offset: u32, value: u32) {
        self.bus.write(offset, value);
    }

    pub fn id(&self) -> u32 {
        self.read(REG_ID)
    }

    pub fn version(&self) -> u32 {
        self.read(REG_VERSION)
    }

    pub fn capabilities(&self) -> u32 {
        self.read(REG_CAPABILITIES)
    }

    pub fn status(&self) -> u32 {
        self.read(REG_STATUS)
    }

    pub fn irq_pending(&self) -> u32 {
        self.read(REG_IRQ_PENDING)
    }

    pub fn irq_enable(&self) -> u32 {
        self.read(REG_IRQ_ENABLE)
    }

    pub fn set_irq_enable(&self, enable: bool) {
        self.write(REG_IRQ_ENABLE, if enable { 1 } else { 0 });
    }

    pub fn ack_irq(&self, bits: u32) {
        self.write(REG_IRQ_ACK, bits);
    }

    pub fn clear_errors(&self) {
        self.write(REG_ERROR_CLEAR, 0x3);
        self.write(REG_IRQ_ACK, IRQ_PEND_DONE | IRQ_PEND_ERR);
    }

    /// Probe ID/VERSION/CAPABILITIES against the locked V2 map.
    pub fn probe_identity(&self) -> Result<(), G1pError> {
        let id = self.id();
        let version = self.version();
        let caps = self.capabilities();
        if id == EXPECTED_ID && version == EXPECTED_VERSION && caps == EXPECTED_CAPABILITIES {
            Ok(())
        } else {
            Err(G1pError::IdentityMismatch { id, version, caps })
        }
    }

    /// Spin until `STATUS.busy` clears (or timeout).
    pub fn wait_not_busy(&self, timeout_spins: u32) -> Result<(), G1pError> {
        for _ in 0..timeout_spins {
            if (self.status() & ST_BUSY) == 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(G1pError::Timeout)
    }

    fn write_operands(&self, a: u32, b: u32, c: u32) -> Result<(), G1pError> {
        // Legal phase operands are [1:0] only; any high bit latches input_error.
        self.write(REG_INPUT_A, a);
        self.write(REG_INPUT_B, b);
        self.write(REG_INPUT_C, c);
        let st = self.status();
        if (st & ST_INPUT_ERR) != 0 {
            return Err(G1pError::InputError);
        }
        Ok(())
    }

    /// Write runtime inputs and issue EXEC; wait for completion; read results.
    pub fn exec(&self, a: u32, b: u32, c: u32, timeout_spins: u32) -> Result<ExecResult, G1pError> {
        self.clear_errors();
        self.wait_not_busy(timeout_spins)?;
        self.write_operands(a & 3, b & 3, c & 3)?;
        self.write(REG_COMMAND, OPC_EXEC);
        self.wait_not_busy(timeout_spins)?;
        let status = self.status();
        if (status & ST_OPCODE_ERR) != 0 {
            return Err(G1pError::OpcodeError);
        }
        if (status & ST_INPUT_ERR) != 0 {
            return Err(G1pError::InputError);
        }
        if (status & ST_DONE) == 0 {
            return Err(G1pError::NotDone);
        }
        let hash = self.read(REG_TRACE_HASH);
        let meta = self.read(REG_TRACE_META);
        Ok(ExecResult {
            compose_ab: self.read(REG_COMPOSE_AB) & 3,
            left: self.read(REG_LEFT_RESULT) & 3,
            right: self.read(REG_RIGHT_RESULT) & 3,
            parity: self.read(REG_PARITY) & 1,
            reflect: self.read(REG_REFLECT_RESULT) & 3,
            status,
            hash,
            cycle: (meta >> 16) as u16,
        })
    }

    /// Compose-only convenience (C = 0).
    pub fn compose_ab(&self, a: u32, b: u32) -> Result<u32, G1pError> {
        Ok(self.exec(a, b, 0, DEFAULT_TIMEOUT_SPINS)?.compose_ab)
    }

    /// Reflect ρ(A) via EXEC capture of REFLECT_RESULT.
    pub fn reflect(&self, a: u32) -> Result<u32, G1pError> {
        Ok(self.exec(a, 0, 0, DEFAULT_TIMEOUT_SPINS)?.reflect)
    }

    /// Exercise illegal high-bit input; expect `input_error`.
    pub fn exercise_invalid_input(&self) -> Result<(), G1pError> {
        self.clear_errors();
        self.write(REG_INPUT_A, 0x0000_0014);
        let st = self.status();
        if (st & ST_INPUT_ERR) == 0 {
            return Err(G1pError::NotDone);
        }
        self.clear_errors();
        Ok(())
    }

    /// Issue illegal opcode; expect `opcode_error`.
    pub fn exercise_invalid_opcode(&self) -> Result<(), G1pError> {
        self.clear_errors();
        self.write(REG_COMMAND, 0xAA);
        let st = self.status();
        if (st & ST_OPCODE_ERR) == 0 {
            return Err(G1pError::NotDone);
        }
        self.clear_errors();
        let _ = OPC_NOP;
        Ok(())
    }
}

impl<B: G1pBus + Send + Sync + 'static> PhaseAccelerator for G1p<B> {
    fn mode(&self) -> ConsensusMode {
        ConsensusMode::Accelerated
    }

    fn probe(&self) -> Result<(), ConsensusError> {
        self.probe_identity().map_err(|_| ConsensusError::ProbeFailed)
    }

    fn exec_triple(
        &self,
        a: PhaseState,
        b: PhaseState,
        c: PhaseState,
    ) -> Result<HwTrace, ConsensusError> {
        crate::kernel::phase_consensus::exec_triple_on_bus(
            &self.bus,
            a.as_u8(),
            b.as_u8(),
            c.as_u8(),
            DEFAULT_TIMEOUT_SPINS,
        )
    }
}

#[cfg(feature = "tensor")]
mod tensor_bank {
    use super::super::memory_map::G1P_TENSOR_OFFSET;
    use super::G1p;
    use crate::kernel::phase_consensus::G1pBus;

    /// `TNSR`. Path-1 ID at `G1P_AXI_BASE + 0x100`.
    pub const TENSOR_ID: u32 = 0x544E_5352;
    pub const TENSOR_VERSION: u32 = 0x0001_0000;

    const REG_T_ID: u32 = G1P_TENSOR_OFFSET;
    const REG_T_VER: u32 = G1P_TENSOR_OFFSET + 0x04;
    const REG_T_CUR: u32 = G1P_TENSOR_OFFSET + 0x08;
    const REG_T_OPERAND: u32 = G1P_TENSOR_OFFSET + 0x0C;
    const REG_T_RESULT: u32 = G1P_TENSOR_OFFSET + 0x10;
    const REG_T_STATUS: u32 = G1P_TENSOR_OFFSET + 0x14;
    const REG_T_CMD: u32 = G1P_TENSOR_OFFSET + 0x18;

    pub const TNSR_COMPOSE: u32 = 1;
    pub const TNSR_REFLECT: u32 = 2;
    pub const TNSR_ADVANCE_M: u32 = 3;
    pub const TNSR_REFRESH_V: u32 = 4;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TensorBankError {
        NotPresent { id: u32, version: u32 },
        Layer0Mismatch { id: u32 },
    }

    impl<B: G1pBus> G1p<B> {
        pub fn tensor_id(&self) -> u32 {
            self.read(REG_T_ID)
        }

        pub fn tensor_version(&self) -> u32 {
            self.read(REG_T_VER)
        }

        /// Probe Path-1. CPU0 only. No MMIO at `0x40001000`.
        pub fn probe_tensor_bank_result(&self) -> Result<(), TensorBankError> {
            let l0 = self.id();
            if l0 != super::EXPECTED_ID {
                return Err(TensorBankError::Layer0Mismatch { id: l0 });
            }
            let id = self.tensor_id();
            let version = self.tensor_version();
            if id == TENSOR_ID && version == TENSOR_VERSION {
                Ok(())
            } else {
                Err(TensorBankError::NotPresent { id, version })
            }
        }

        pub fn probe_tensor_bank(&self) -> bool {
            self.probe_tensor_bank_result().is_ok()
        }

        /// Advisory PL evaluate. Caller must still `tensor_state::commit` on CPU0.
        pub fn tensor_evaluate(&self, cur: u8, cmd: u32, operand: u8) -> Result<u8, super::G1pError> {
            self.write(REG_T_CUR, cur as u32);
            self.write(REG_T_OPERAND, operand as u32);
            self.write(REG_T_CMD, cmd);
            let st = self.read(REG_T_STATUS);
            if (st & 2) != 0 {
                return Err(super::G1pError::OpcodeError);
            }
            if (st & 1) == 0 {
                return Err(super::G1pError::NotDone);
            }
            Ok(self.read(REG_T_RESULT) as u8)
        }
    }
}

#[cfg(feature = "tensor")]
pub use tensor_bank::*;
