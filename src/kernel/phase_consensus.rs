//! AXI lock-step consensus over G1P Bring-up V2 (`0x40000000`).
//!
//! Software authority remains [`crate::core::gphase_core`]. Hardware (or a
//! labeled mock of the real V2 map) must match before a compose result is
//! committed. Emulated mode is explicit and is never reported as hardware PASS.
//!
//! The 32-bit cycle signature is FNV-1a over captured operands/results/cycle.
//! It is not PhaseHash and not SHA-256.

use crate::core::gphase_core::{
    compose, left_group, reflect, right_group, topology_status, PhaseState, TopologyKind,
};
use core::fmt;
use core::sync::atomic::{AtomicU16, Ordering};

#[cfg(feature = "std")]
use std::boxed::Box;
#[cfg(feature = "std")]
use std::sync::Mutex;
#[cfg(all(feature = "baremetal", not(feature = "std")))]
use alloc::boxed::Box;
#[cfg(all(feature = "baremetal", not(feature = "std")))]
use spin::Mutex;

/// Bounded poll budget used by G1P `exec` and lock-step.
pub const DEFAULT_TIMEOUT_SPINS: u32 = 1_000_000;

pub mod regs {
    pub const ID: u32 = 0x00;
    pub const VERSION: u32 = 0x04;
    pub const INPUT_A: u32 = 0x08;
    pub const INPUT_B: u32 = 0x0C;
    pub const INPUT_C: u32 = 0x10;
    pub const COMPOSE_AB: u32 = 0x14;
    pub const LEFT_RESULT: u32 = 0x18;
    pub const RIGHT_RESULT: u32 = 0x1C;
    pub const PARITY: u32 = 0x20;
    pub const REFLECT_RESULT: u32 = 0x24;
    pub const STATUS: u32 = 0x30;
    pub const COMMAND: u32 = 0x34;
    pub const CAPABILITIES: u32 = 0x38;
    pub const IRQ_ACK: u32 = 0x48;
    pub const ERROR_CLEAR: u32 = 0x4C;
    pub const TRACE_HASH: u32 = 0x50;
    pub const TRACE_META: u32 = 0x54;

    pub const EXPECTED_ID: u32 = 0x4750_4C34;
    pub const EXPECTED_VERSION: u32 = 0x0002_0000;
    pub const EXPECTED_CAPABILITIES: u32 = 0x0000_00FF;

    pub const OPC_NOP: u32 = 0x00;
    pub const OPC_EXEC: u32 = 0x01;
    pub const OPC_SELFTEST: u32 = 0x02;

    pub const ST_DONE: u32 = 1 << 0;
    pub const ST_BUSY: u32 = 1 << 1;
    pub const ST_INPUT_ERR: u32 = 1 << 2;
    pub const ST_RESET_SEEN: u32 = 1 << 3;
    pub const ST_OPCODE_ERR: u32 = 1 << 4;

    pub const IRQ_PEND_DONE: u32 = 1 << 0;
    pub const IRQ_PEND_ERR: u32 = 1 << 1;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsensusMode {
    Accelerated,
    Emulated,
}

impl ConsensusMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ConsensusMode::Accelerated => "ACCELERATED",
            ConsensusMode::Emulated => "EMULATED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsensusError {
    Timeout,
    HardwareFault,
    Mismatch,
    ProbeFailed,
    AxiBusy,
}

impl ConsensusError {
    pub fn as_str(self) -> &'static str {
        match self {
            ConsensusError::Timeout => "axi lock-step timeout",
            ConsensusError::HardwareFault => "g1p hardware fault",
            ConsensusError::Mismatch => "software/hardware phase mismatch",
            ConsensusError::ProbeFailed => "g1p identity probe failed",
            ConsensusError::AxiBusy => "g1p still busy",
        }
    }
}

impl fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwTrace {
    pub compose: PhaseState,
    pub left: PhaseState,
    pub right: PhaseState,
    pub reflect: PhaseState,
    pub parity: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HwTrace {
    pub compose: u8,
    pub left: u8,
    pub right: u8,
    pub reflect: u8,
    pub parity: u8,
    pub status: u32,
    pub hash: u32,
    pub cycle: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsensusOk {
    pub mode: ConsensusMode,
    pub sw: SwTrace,
    pub hw: HwTrace,
}

/// FNV-1a 32-bit cycle signature. Must match `gpl_axi_bringup_v2.v`.
pub fn cycle_signature(
    a: u8,
    b: u8,
    c: u8,
    compose_ab: u8,
    left: u8,
    right: u8,
    reflect_ab: u8,
    parity: u8,
    cycle: u16,
) -> u32 {
    let bytes = [
        a & 3,
        b & 3,
        c & 3,
        compose_ab & 3,
        left & 3,
        right & 3,
        reflect_ab & 3,
        parity & 1,
        cycle as u8,
        (cycle >> 8) as u8,
    ];
    let mut h = 0x811c_9dc5u32;
    for byte in bytes {
        h ^= byte as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

pub fn software_trace(a: PhaseState, b: PhaseState, c: PhaseState) -> SwTrace {
    let topo = topology_status(a, b, c);
    SwTrace {
        compose: compose(a, b),
        left: left_group(a, b, c),
        right: right_group(a, b, c),
        reflect: reflect(a),
        parity: match topo.kind {
            TopologyKind::Same => 0,
            TopologyKind::Reflection => 1,
        },
    }
}

pub trait G1pBus: Send + Sync {
    fn read(&self, offset: u32) -> u32;
    fn write(&self, offset: u32, value: u32);
}

pub trait PhaseAccelerator: Send + Sync {
    fn mode(&self) -> ConsensusMode;
    fn probe(&self) -> Result<(), ConsensusError>;
    fn exec_triple(
        &self,
        a: PhaseState,
        b: PhaseState,
        c: PhaseState,
    ) -> Result<HwTrace, ConsensusError>;
}

pub fn exec_triple_on_bus<B: G1pBus + ?Sized>(
    bus: &B,
    a: u8,
    b: u8,
    c: u8,
    timeout_spins: u32,
) -> Result<HwTrace, ConsensusError> {
    bus.write(regs::ERROR_CLEAR, 0x3);
    bus.write(regs::IRQ_ACK, regs::IRQ_PEND_DONE | regs::IRQ_PEND_ERR);
    wait_not_busy(bus, timeout_spins)?;
    bus.write(regs::INPUT_A, (a & 3) as u32);
    bus.write(regs::INPUT_B, (b & 3) as u32);
    bus.write(regs::INPUT_C, (c & 3) as u32);
    let st = bus.read(regs::STATUS);
    if (st & regs::ST_INPUT_ERR) != 0 {
        return Err(ConsensusError::HardwareFault);
    }
    bus.write(regs::COMMAND, regs::OPC_EXEC);
    wait_not_busy(bus, timeout_spins)?;
    let status = bus.read(regs::STATUS);
    if (status & (regs::ST_OPCODE_ERR | regs::ST_INPUT_ERR)) != 0 {
        return Err(ConsensusError::HardwareFault);
    }
    if (status & regs::ST_DONE) == 0 {
        return Err(ConsensusError::HardwareFault);
    }
    let hash = bus.read(regs::TRACE_HASH);
    let meta = bus.read(regs::TRACE_META);
    Ok(HwTrace {
        compose: (bus.read(regs::COMPOSE_AB) & 3) as u8,
        left: (bus.read(regs::LEFT_RESULT) & 3) as u8,
        right: (bus.read(regs::RIGHT_RESULT) & 3) as u8,
        reflect: (bus.read(regs::REFLECT_RESULT) & 3) as u8,
        parity: (bus.read(regs::PARITY) & 1) as u8,
        status,
        hash,
        cycle: (meta >> 16) as u16,
    })
}

fn wait_not_busy<B: G1pBus + ?Sized>(bus: &B, timeout_spins: u32) -> Result<(), ConsensusError> {
    for _ in 0..timeout_spins {
        let st = bus.read(regs::STATUS);
        if (st & regs::ST_BUSY) == 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(ConsensusError::Timeout)
}

pub fn evaluate_lockstep<A: PhaseAccelerator + ?Sized>(
    accel: &A,
    a: PhaseState,
    b: PhaseState,
    c: PhaseState,
) -> Result<ConsensusOk, ConsensusError> {
    accel.probe()?;
    let sw = software_trace(a, b, c);
    let hw = accel.exec_triple(a, b, c)?;
    if sw.compose.as_u8() != hw.compose
        || sw.left.as_u8() != hw.left
        || sw.right.as_u8() != hw.right
        || sw.reflect.as_u8() != hw.reflect
        || sw.parity != hw.parity
    {
        return Err(ConsensusError::Mismatch);
    }
    let has_trace = hw.hash != 0 || hw.cycle != 0;
    if has_trace {
        let expect = cycle_signature(
            a.as_u8(),
            b.as_u8(),
            c.as_u8(),
            hw.compose,
            hw.left,
            hw.right,
            hw.reflect,
            hw.parity,
            hw.cycle,
        );
        if expect != hw.hash {
            return Err(ConsensusError::Mismatch);
        }
    }
    Ok(ConsensusOk {
        mode: accel.mode(),
        sw,
        hw,
    })
}

pub struct EmulatedAccelerator {
    cycle: AtomicU16,
}

impl EmulatedAccelerator {
    pub fn new() -> Self {
        Self {
            cycle: AtomicU16::new(0),
        }
    }
}

impl Default for EmulatedAccelerator {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseAccelerator for EmulatedAccelerator {
    fn mode(&self) -> ConsensusMode {
        ConsensusMode::Emulated
    }

    fn probe(&self) -> Result<(), ConsensusError> {
        Ok(())
    }

    fn exec_triple(
        &self,
        a: PhaseState,
        b: PhaseState,
        c: PhaseState,
    ) -> Result<HwTrace, ConsensusError> {
        let sw = software_trace(a, b, c);
        let n = self.cycle.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        let hash = cycle_signature(
            a.as_u8(),
            b.as_u8(),
            c.as_u8(),
            sw.compose.as_u8(),
            sw.left.as_u8(),
            sw.right.as_u8(),
            sw.reflect.as_u8(),
            sw.parity,
            n,
        );
        Ok(HwTrace {
            compose: sw.compose.as_u8(),
            left: sw.left.as_u8(),
            right: sw.right.as_u8(),
            reflect: sw.reflect.as_u8(),
            parity: sw.parity,
            status: regs::ST_DONE,
            hash,
            cycle: n,
        })
    }
}

struct MockState {
    input_a: u32,
    input_b: u32,
    input_c: u32,
    compose: u32,
    left: u32,
    right: u32,
    reflect: u32,
    parity: u32,
    status: u32,
    cycle: u16,
    hash: u32,
    meta: u32,
    pending: bool,
    polls_left: u32,
    busy_hold: u32,
    compose_xor: u8,
    emit_trace: bool,
    force_opcode_error: bool,
    identity_ok: bool,
}

impl MockState {
    fn new() -> Self {
        Self {
            input_a: 0,
            input_b: 0,
            input_c: 0,
            compose: 0,
            left: 0,
            right: 0,
            reflect: 0,
            parity: 0,
            status: regs::ST_RESET_SEEN,
            cycle: 0,
            hash: 0,
            meta: 0,
            pending: false,
            polls_left: 0,
            busy_hold: 0,
            compose_xor: 0,
            emit_trace: true,
            force_opcode_error: false,
            identity_ok: true,
        }
    }

    fn capture(&mut self) {
        let Some(pa) = PhaseState::from_u8(self.input_a as u8) else {
            self.status |= regs::ST_INPUT_ERR;
            self.pending = false;
            return;
        };
        let Some(pb) = PhaseState::from_u8(self.input_b as u8) else {
            self.status |= regs::ST_INPUT_ERR;
            self.pending = false;
            return;
        };
        let Some(pc) = PhaseState::from_u8(self.input_c as u8) else {
            self.status |= regs::ST_INPUT_ERR;
            self.pending = false;
            return;
        };
        let sw = software_trace(pa, pb, pc);
        self.compose = (sw.compose.as_u8() ^ self.compose_xor) as u32 & 3;
        self.left = sw.left.as_u8() as u32;
        self.right = sw.right.as_u8() as u32;
        self.reflect = sw.reflect.as_u8() as u32;
        self.parity = sw.parity as u32;
        self.cycle = self.cycle.wrapping_add(1);
        self.status = regs::ST_DONE | regs::ST_RESET_SEEN;
        if self.emit_trace {
            self.hash = cycle_signature(
                pa.as_u8(),
                pb.as_u8(),
                pc.as_u8(),
                self.compose as u8,
                self.left as u8,
                self.right as u8,
                self.reflect as u8,
                self.parity as u8,
                self.cycle,
            );
            self.meta = ((self.cycle as u32) << 16)
                | ((self.compose & 0xff) << 8)
                | (self.status & 0xff);
        } else {
            self.hash = 0;
            self.meta = 0;
        }
        self.pending = false;
        self.polls_left = 0;
    }
}

/// Host-test double of the frozen G1P V2 window plus TRACE `0x50`/`0x54`.
pub struct MockG1pBus {
    inner: Mutex<MockState>,
}

impl MockG1pBus {
    pub fn matching() -> Self {
        Self {
            inner: Mutex::new(MockState::new()),
        }
    }
}

fn mock_lock(bus: &MockG1pBus) -> impl core::ops::DerefMut<Target = MockState> + '_ {
    #[cfg(feature = "std")]
    {
        bus.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
    #[cfg(not(feature = "std"))]
    {
        bus.inner.lock()
    }
}

impl G1pBus for MockG1pBus {
    fn read(&self, offset: u32) -> u32 {
        let mut s = mock_lock(self);
        if offset == regs::STATUS && s.pending {
            if s.polls_left > 0 {
                s.polls_left -= 1;
            }
            if s.polls_left == 0 {
                s.capture();
            }
        }
        match offset {
            regs::ID => {
                if s.identity_ok {
                    regs::EXPECTED_ID
                } else {
                    0
                }
            }
            regs::VERSION => {
                if s.identity_ok {
                    regs::EXPECTED_VERSION
                } else {
                    0
                }
            }
            regs::CAPABILITIES => {
                if s.identity_ok {
                    regs::EXPECTED_CAPABILITIES
                } else {
                    0
                }
            }
            regs::INPUT_A => s.input_a,
            regs::INPUT_B => s.input_b,
            regs::INPUT_C => s.input_c,
            regs::COMPOSE_AB => s.compose,
            regs::LEFT_RESULT => s.left,
            regs::RIGHT_RESULT => s.right,
            regs::PARITY => s.parity,
            regs::REFLECT_RESULT => s.reflect,
            regs::STATUS => s.status,
            regs::TRACE_HASH => s.hash,
            regs::TRACE_META => s.meta,
            _ => 0,
        }
    }

    fn write(&self, offset: u32, value: u32) {
        let mut s = mock_lock(self);
        match offset {
            regs::INPUT_A => {
                if (value & !3) != 0 {
                    s.status |= regs::ST_INPUT_ERR;
                }
                s.input_a = value & 3;
            }
            regs::INPUT_B => {
                if (value & !3) != 0 {
                    s.status |= regs::ST_INPUT_ERR;
                }
                s.input_b = value & 3;
            }
            regs::INPUT_C => {
                if (value & !3) != 0 {
                    s.status |= regs::ST_INPUT_ERR;
                }
                s.input_c = value & 3;
            }
            regs::COMMAND => {
                let opc = value & 0xff;
                if opc == regs::OPC_NOP {
                } else if opc == regs::OPC_EXEC {
                    if s.force_opcode_error {
                        s.status |= regs::ST_OPCODE_ERR;
                        s.status &= !regs::ST_DONE;
                    } else if s.busy_hold > 0 {
                        s.status = regs::ST_BUSY | regs::ST_RESET_SEEN;
                        s.pending = true;
                        s.polls_left = s.busy_hold;
                    } else {
                        s.capture();
                    }
                } else if opc == regs::OPC_SELFTEST {
                    s.input_a = 0;
                    s.input_b = 1;
                    s.input_c = 0;
                    s.capture();
                } else {
                    s.status |= regs::ST_OPCODE_ERR;
                }
            }
            regs::ERROR_CLEAR => {
                if (value & 1) != 0 {
                    s.status &= !regs::ST_INPUT_ERR;
                }
                if (value & 2) != 0 {
                    s.status &= !regs::ST_OPCODE_ERR;
                }
            }
            regs::IRQ_ACK => {}
            _ => {}
        }
    }
}

pub struct MockG1p {
    bus: MockG1pBus,
    timeout_spins: u32,
}

impl MockG1p {
    pub fn matching() -> Self {
        Self {
            bus: MockG1pBus::matching(),
            timeout_spins: DEFAULT_TIMEOUT_SPINS,
        }
    }

    pub fn with_timeout(mut self, spins: u32) -> Self {
        self.timeout_spins = spins;
        self
    }

    pub fn hold_busy(self, polls: u32) -> Self {
        mock_lock(&self.bus).busy_hold = polls;
        self
    }

    pub fn xor_compose(self, xor: u8) -> Self {
        mock_lock(&self.bus).compose_xor = xor;
        self
    }

    pub fn force_error(self) -> Self {
        mock_lock(&self.bus).force_opcode_error = true;
        self
    }

    pub fn legacy_no_trace(self) -> Self {
        mock_lock(&self.bus).emit_trace = false;
        self
    }

    pub fn fail_probe(self) -> Self {
        mock_lock(&self.bus).identity_ok = false;
        self
    }
}

impl PhaseAccelerator for MockG1p {
    fn mode(&self) -> ConsensusMode {
        ConsensusMode::Accelerated
    }

    fn probe(&self) -> Result<(), ConsensusError> {
        let id = self.bus.read(regs::ID);
        let ver = self.bus.read(regs::VERSION);
        let caps = self.bus.read(regs::CAPABILITIES);
        if id == regs::EXPECTED_ID && ver == regs::EXPECTED_VERSION && caps == regs::EXPECTED_CAPABILITIES
        {
            Ok(())
        } else {
            Err(ConsensusError::ProbeFailed)
        }
    }

    fn exec_triple(
        &self,
        a: PhaseState,
        b: PhaseState,
        c: PhaseState,
    ) -> Result<HwTrace, ConsensusError> {
        exec_triple_on_bus(
            &self.bus,
            a.as_u8(),
            b.as_u8(),
            c.as_u8(),
            self.timeout_spins,
        )
    }
}

enum AccelKind {
    Emulated(EmulatedAccelerator),
    External(Box<dyn PhaseAccelerator>),
}

/// Kernel-owned consensus backend. Host default is Emulated.
pub struct PhaseConsensus {
    inner: AccelKind,
}

impl PhaseConsensus {
    pub fn emulated() -> Self {
        Self {
            inner: AccelKind::Emulated(EmulatedAccelerator::new()),
        }
    }

    pub fn from_accel(accel: Box<dyn PhaseAccelerator>) -> Self {
        Self {
            inner: AccelKind::External(accel),
        }
    }

    pub fn attach(&mut self, accel: Box<dyn PhaseAccelerator>) {
        self.inner = AccelKind::External(accel);
    }

    pub fn mode(&self) -> ConsensusMode {
        match &self.inner {
            AccelKind::Emulated(a) => a.mode(),
            AccelKind::External(a) => a.mode(),
        }
    }

    fn accel(&self) -> &dyn PhaseAccelerator {
        match &self.inner {
            AccelKind::Emulated(a) => a,
            AccelKind::External(a) => a.as_ref(),
        }
    }

    pub fn evaluate_triple(
        &self,
        a: PhaseState,
        b: PhaseState,
        c: PhaseState,
    ) -> Result<ConsensusOk, ConsensusError> {
        evaluate_lockstep(self.accel(), a, b, c)
    }

    pub fn evaluate_compose(
        &self,
        a: PhaseState,
        b: PhaseState,
    ) -> Result<ConsensusOk, ConsensusError> {
        self.evaluate_triple(a, b, PhaseState::Phi0)
    }
}

impl Default for PhaseConsensus {
    fn default() -> Self {
        Self::emulated()
    }
}
