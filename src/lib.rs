//! # Golden Phase Operating System (GPOS)
//!
//! *gφlden phase φperating system — a phase-native OS whose every layer is
//! governed by one equation.*
//!
//! GPOS is not a conventional OS with an algebra bolted on. Its scheduler,
//! memory model, filesystem, and integrity log are all expressed in a single
//! four-valued algebra derived from the **biquadratic kernel**
//!
//! ```text
//! y⁴ − φy² + α = 0
//! ```
//!
//! whose discrete solutions are the four **Kairos States** `Kairos0..Kairos3`
//! (spelled `Phi0..Phi3` in code). Process contention, page protection, and
//! packet integrity are all resolved by *composing* these states — there is one
//! source of mathematical truth, [`core::gphase_core`], and nothing reimplements
//! it.
//!
//! The crate id stays `kairos` for build/path stability; the product name is
//! Golden Phase Operating System.
//!
//! ## Architecture
//!
//! Each layer imports only from the ones below it, and the algebra sits at the
//! bottom:
//!
//! | Layer | Module | Role |
//! |-------|--------|------|
//! | Algebra | [`core`] | Equation-derived Kairos-State algebra: `compose`, `reflect`, topology |
//! | Protocol | [`protocol`] | Encoders, checksums, and the hybrid CRC+GPL integrity frame |
//! | VM / ISA | [`vm`], [`isa`] | The Golden Phase virtual machine and its instruction set |
//! | Kernel | [`kernel`] | Scheduler, phase-tagged memory, syscalls, statechain black box |
//! | Filesystem | [`fs`] | PhaseFS — phase-tagged blocks and inodes |
//! | Clock | [`stateclock`] | Kairos / StateClock (opportune-moment time) |
//! | Userland | [`userland`] | The `gpos>` shell and full command registry |
//! | Runtime | [`runtime`] | The hosted full-OS runner that ties it together |
//! | Boot | [`boot`], [`arch`], [`board`] | Bare-metal entry: RISC-V QEMU and Cortex-A9 (PZ7020) |
//!
//! ## Build targets
//!
//! The same kernel runs in three places:
//!
//! - **Host** (`std`, the default): `cargo run --bin kairos_host` — the OS as a
//!   hosted process, best for development.
//! - **RISC-V QEMU** (`baremetal`): [`boot`] — freestanding, boots to a serial
//!   `gpos>` over UART on the `virt` machine.
//! - **PZ7020-SL-C / Cortex-A9** (`zynq-a9`): `board::zynq7000` — freestanding
//!   ARM image with its own DDR/SD bring-up, booting the same runtime from real
//!   silicon.
//!
//! ## Where to start reading
//!
//! - The algebra everything rests on: [`core::gphase_core`].
//! - What the OS *does*: [`kernel`] (scheduler + phase-tagged memory) and
//!   [`userland`] (the shell you talk to).
//! - How it boots on hardware: `board::zynq7000::kernel_main` (built with the
//!   `zynq-a9` feature for the ARM target).

#![cfg_attr(all(feature = "baremetal", not(feature = "std"), not(test)), no_std)]

#[cfg(all(feature = "baremetal", not(feature = "std"), not(test)))]
#[macro_use]
extern crate alloc;

pub mod brand;
pub mod core;
pub mod fs;
#[cfg(feature = "ssh")]
pub mod hosted;
pub mod kernel;
pub mod platform;
pub mod protocol;
pub mod stateclock;
pub mod userland;
pub mod vm;

pub mod arch;
pub mod board;
pub mod boot;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod isa;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod runtime;
#[cfg(feature = "std")]
pub mod zynq_oracle;
#[cfg(all(feature = "std", feature = "phasestack"))]
pub mod sdk;

pub use core::gphase_core::PhaseState;
pub use fs::{FsError, PhaseFs};
pub use kernel::Kernel;

#[cfg(any(feature = "std", feature = "baremetal"))]
pub use kernel::statechain::StatechainLedger;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub use kernel::statechain_checkpoint::{CheckpointError, SignedCheckpoint};
#[cfg(any(feature = "std", feature = "baremetal"))]
pub use kernel::statechain_watchdog::{StatechainAuditReport, StatechainWatchdog};
pub use stateclock::{StateClock, StateClockSnapshot};
pub use userland::Hosted;
pub use vm::{Cpu, Instruction, Register, Trap, VmError};

#[cfg(feature = "std")]
pub use isa::{assemble, assemble_instructions, decode, encode, AsmError, DisasmError};

#[cfg(any(feature = "std", feature = "baremetal"))]
pub use runtime::Runtime;
