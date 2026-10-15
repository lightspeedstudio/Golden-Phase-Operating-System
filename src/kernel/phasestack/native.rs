//! Native payloads: ARM ELF32 ET_EXEC loads into the isolated user
//! window when `mmu-app-isolation` is live on Cortex-A9. Host and
//! isolation-off builds still return `NativeIsolationUnavailable`.
//! Raw A32 non-ELF uses `arch::armv7::isolation::run_raw_a32`.
//! Not a Linux ABI, not dynamic linking, not a third-party libc.

use crate::kernel::crypto::phase_hash::ph_code;
use crate::kernel::hash::StateHash;
use crate::kernel::phasestack::types::PhaseStackError;
use crate::kernel::Kernel;
use crate::platform::Vec;

const EI_CLASS_32: u8 = 1;
const EI_DATA_LSB: u8 = 1;
const EI_VERSION: u8 = 1;
const ET_EXEC: u16 = 2;
const EM_ARM: u16 = 40;
const PT_NULL: u32 = 0;
const PT_LOAD: u32 = 1;
const PT_INTERP: u32 = 3;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;
const EF_ARM_ABI_FLOAT_HARD: u32 = 0x400;
const EHDR_SIZE: usize = 52;
const PHDR_SIZE: usize = 32;
const MAX_PHNUM: u16 = 8;
/// Same size as the isolation user code page (`mmu::PAGE_SIZE`).
const CODE_PAGE: u32 = 0x1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationStatus {
    /// Isolation machinery is not in this build (host, or feature off).
    Unavailable,
    /// Isolation is compiled in but not live (`enable()` has not run).
    Blocked,
    /// Raw A32 can enter the isolated user window.
    RawA32Ready,
    /// ELF execute is refused (host, or isolation not live).
    ElfRefused,
    /// ARM ELF32 can enter the isolated user window.
    ElfReady,
}

/// Raw-A32 isolation status. Not an ELF-execute claim.
pub fn mmu_wx_status() -> IsolationStatus {
    #[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
    {
        if crate::arch::armv7::isolation::enabled() {
            IsolationStatus::RawA32Ready
        } else {
            IsolationStatus::Blocked
        }
    }
    #[cfg(not(all(target_arch = "arm", feature = "mmu-app-isolation")))]
    IsolationStatus::Unavailable
}

/// ELF execute status. Separate from [`mmu_wx_status`].
pub fn elf_execute_status() -> IsolationStatus {
    #[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
    {
        if crate::arch::armv7::isolation::enabled() {
            IsolationStatus::ElfReady
        } else {
            IsolationStatus::ElfRefused
        }
    }
    #[cfg(not(all(target_arch = "arm", feature = "mmu-app-isolation")))]
    IsolationStatus::ElfRefused
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeImage {
    pub code_hash: StateHash,
    pub is_elf: bool,
    pub load_off: u32,
    pub load_len: u32,
    pub entry_off: u32,
}

pub fn is_elf(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0] == 0x7f && bytes[1] == b'E' && bytes[2] == b'L' && bytes[3] == b'F'
}

fn u16_at(bytes: &[u8], off: usize) -> Result<u16, PhaseStackError> {
    let s = bytes
        .get(off..off + 2)
        .ok_or(PhaseStackError::Overflow)?;
    Ok(u16::from_le_bytes(s.try_into().unwrap()))
}

fn u32_at(bytes: &[u8], off: usize) -> Result<u32, PhaseStackError> {
    let s = bytes
        .get(off..off + 4)
        .ok_or(PhaseStackError::Overflow)?;
    Ok(u32::from_le_bytes(s.try_into().unwrap()))
}

/// ARM ELF32 little-endian ET_EXEC, one RX `PT_LOAD`, no interpreter, no W^X.
pub fn parse_elf(bytes: &[u8]) -> Result<NativeImage, PhaseStackError> {
    if bytes.len() < EHDR_SIZE {
        return Err(PhaseStackError::AbiRejected);
    }
    if !is_elf(bytes) {
        return Err(PhaseStackError::AbiRejected);
    }
    if bytes[4] != EI_CLASS_32
        || bytes[5] != EI_DATA_LSB
        || bytes[6] != EI_VERSION
    {
        return Err(PhaseStackError::AbiRejected);
    }
    if u16_at(bytes, 16)? != ET_EXEC || u16_at(bytes, 18)? != EM_ARM {
        return Err(PhaseStackError::AbiRejected);
    }
    if u32_at(bytes, 20)? != 1 {
        return Err(PhaseStackError::AbiRejected);
    }
    let e_entry = u32_at(bytes, 24)?;
    if e_entry & 1 != 0 {
        return Err(PhaseStackError::AbiRejected);
    }
    let e_phoff = u32_at(bytes, 28)? as usize;
    let e_flags = u32_at(bytes, 36)?;
    if e_flags & EF_ARM_ABI_FLOAT_HARD != 0 {
        return Err(PhaseStackError::AbiRejected);
    }
    if u16_at(bytes, 40)? != EHDR_SIZE as u16 || u16_at(bytes, 42)? != PHDR_SIZE as u16 {
        return Err(PhaseStackError::AbiRejected);
    }
    let phnum = u16_at(bytes, 44)?;
    if phnum == 0 || phnum > MAX_PHNUM {
        return Err(PhaseStackError::AbiRejected);
    }
    let mut load: Option<(u32, u32, u32, u32)> = None;
    for i in 0..phnum {
        let off = e_phoff
            .checked_add(i as usize * PHDR_SIZE)
            .ok_or(PhaseStackError::Overflow)?;
        let p_type = u32_at(bytes, off)?;
        if p_type == PT_NULL {
            continue;
        }
        if p_type == PT_INTERP {
            return Err(PhaseStackError::AbiRejected);
        }
        if p_type != PT_LOAD {
            return Err(PhaseStackError::AbiRejected);
        }
        let p_offset = u32_at(bytes, off + 4)?;
        let p_vaddr = u32_at(bytes, off + 8)?;
        let p_filesz = u32_at(bytes, off + 16)?;
        let p_memsz = u32_at(bytes, off + 20)?;
        let p_flags = u32_at(bytes, off + 24)?;
        if p_flags & PF_W != 0 && p_flags & PF_X != 0 {
            return Err(PhaseStackError::AbiRejected);
        }
        if p_flags & PF_X == 0 {
            return Err(PhaseStackError::AbiRejected);
        }
        if p_flags & PF_R == 0 {
            return Err(PhaseStackError::AbiRejected);
        }
        if p_filesz == 0 || p_memsz < p_filesz || p_memsz > CODE_PAGE {
            return Err(PhaseStackError::AbiRejected);
        }
        let end = (p_offset as usize)
            .checked_add(p_filesz as usize)
            .ok_or(PhaseStackError::Overflow)?;
        if end > bytes.len() {
            return Err(PhaseStackError::Overflow);
        }
        if load.is_some() {
            return Err(PhaseStackError::AbiRejected);
        }
        load = Some((p_offset, p_filesz, p_vaddr, p_memsz));
    }
    let (load_off, load_len, p_vaddr, _memsz) = load.ok_or(PhaseStackError::AbiRejected)?;
    if e_entry < p_vaddr {
        return Err(PhaseStackError::AbiRejected);
    }
    let entry_off = e_entry - p_vaddr;
    if entry_off >= load_len {
        return Err(PhaseStackError::AbiRejected);
    }
    Ok(NativeImage {
        code_hash: ph_code(bytes),
        is_elf: true,
        load_off,
        load_len,
        entry_off,
    })
}

/// Wrap A32 bytes in a single RX `PT_LOAD` ARM ELF32 ET_EXEC (`e_entry` = 0).
pub fn encode_a32_exec_elf(payload: &[u8]) -> Result<Vec<u8>, PhaseStackError> {
    if payload.is_empty() || payload.len() > CODE_PAGE as usize {
        return Err(PhaseStackError::AbiRejected);
    }
    let phoff = EHDR_SIZE;
    let payload_off = EHDR_SIZE + PHDR_SIZE;
    let mut out = Vec::new();
    out.resize(payload_off + payload.len(), 0);
    out[0] = 0x7f;
    out[1] = b'E';
    out[2] = b'L';
    out[3] = b'F';
    out[4] = EI_CLASS_32;
    out[5] = EI_DATA_LSB;
    out[6] = EI_VERSION;
    out[16..18].copy_from_slice(&ET_EXEC.to_le_bytes());
    out[18..20].copy_from_slice(&EM_ARM.to_le_bytes());
    out[20..24].copy_from_slice(&1u32.to_le_bytes());
    out[24..28].copy_from_slice(&0u32.to_le_bytes());
    out[28..32].copy_from_slice(&(phoff as u32).to_le_bytes());
    out[36..40].copy_from_slice(&0x0500_0000u32.to_le_bytes());
    out[40..42].copy_from_slice(&(EHDR_SIZE as u16).to_le_bytes());
    out[42..44].copy_from_slice(&(PHDR_SIZE as u16).to_le_bytes());
    out[44..46].copy_from_slice(&1u16.to_le_bytes());
    let p = phoff;
    out[p..p + 4].copy_from_slice(&PT_LOAD.to_le_bytes());
    out[p + 4..p + 8].copy_from_slice(&(payload_off as u32).to_le_bytes());
    let filesz = payload.len() as u32;
    out[p + 16..p + 20].copy_from_slice(&filesz.to_le_bytes());
    out[p + 20..p + 24].copy_from_slice(&filesz.to_le_bytes());
    out[p + 24..p + 28].copy_from_slice(&(PF_R | PF_X).to_le_bytes());
    out[p + 28..p + 32].copy_from_slice(&4u32.to_le_bytes());
    out[payload_off..].copy_from_slice(payload);
    Ok(out)
}

/// Enter the isolated user window with the parsed RX segment.
/// Host / isolation-off: `NativeIsolationUnavailable`.
pub fn execute(
    image: &NativeImage,
    bytes: &[u8],
    kernel: &mut Kernel,
    pid: u32,
    package_id: StateHash,
) -> Result<bool, PhaseStackError> {
    let off = image.load_off as usize;
    let len = image.load_len as usize;
    let end = off.checked_add(len).ok_or(PhaseStackError::Overflow)?;
    if end > bytes.len() || (image.entry_off as usize) >= len {
        return Err(PhaseStackError::AbiRejected);
    }
    let load = &bytes[off..end];
    #[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
    {
        if !crate::arch::armv7::isolation::enabled() {
            return Err(PhaseStackError::NativeIsolationUnavailable);
        }
        match crate::arch::armv7::isolation::run_elf(
            kernel,
            load,
            image.entry_off,
            pid,
            package_id,
        ) {
            Ok(ok) => Ok(ok),
            Err(_) => Err(PhaseStackError::NativeIsolationUnavailable),
        }
    }
    #[cfg(not(all(target_arch = "arm", feature = "mmu-app-isolation")))]
    {
        let _ = (kernel, pid, package_id, load);
        Err(PhaseStackError::NativeIsolationUnavailable)
    }
}

pub fn execute_native_plugin(_image: &NativeImage) -> Result<(), PhaseStackError> {
    Err(PhaseStackError::NativePluginDenied)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn halt_a32() -> [u8; 12] {
        let mut b = [0u8; 12];
        b[0..4].copy_from_slice(&0xE3A0_0000u32.to_le_bytes());
        b[4..8].copy_from_slice(&0xE3A0_7001u32.to_le_bytes());
        b[8..12].copy_from_slice(&0xEF00_0000u32.to_le_bytes());
        b
    }

    fn tiny_elf() -> (NativeImage, Vec<u8>) {
        let bytes = encode_a32_exec_elf(&halt_a32()).unwrap();
        (parse_elf(&bytes).unwrap(), bytes)
    }

    #[test]
    fn elf_parse_and_hash() {
        let (img, bytes) = tiny_elf();
        assert!(img.is_elf);
        assert_eq!(img.entry_off, 0);
        assert_eq!(img.load_len, 12);
        let mut kernel = Kernel::new();
        assert_eq!(
            execute(&img, &bytes, &mut kernel, 0, [0u8; 32]),
            Err(PhaseStackError::NativeIsolationUnavailable)
        );
    }

    #[test]
    fn elf_execute_host_stays_unavailable() {
        assert_eq!(elf_execute_status(), IsolationStatus::ElfRefused);
        let (img, bytes) = tiny_elf();
        let mut kernel = Kernel::new();
        assert_eq!(
            execute(&img, &bytes, &mut kernel, 0, [0u8; 32]),
            Err(PhaseStackError::NativeIsolationUnavailable)
        );
    }

    #[test]
    fn elf64_is_not_a_loadable_image() {
        let mut elf = vec![0x7f, b'E', b'L', b'F', 2];
        elf.resize(EHDR_SIZE, 0);
        assert_eq!(parse_elf(&elf), Err(PhaseStackError::AbiRejected));
    }

    #[test]
    fn wx_load_is_rejected() {
        let mut bytes = encode_a32_exec_elf(&halt_a32()).unwrap();
        let flags = (PF_R | PF_W | PF_X).to_le_bytes();
        bytes[52 + 24..52 + 28].copy_from_slice(&flags);
        assert_eq!(parse_elf(&bytes), Err(PhaseStackError::AbiRejected));
    }

    #[test]
    fn interp_is_rejected() {
        let payload = halt_a32();
        let mut bytes = encode_a32_exec_elf(&payload).unwrap();
        bytes[44..46].copy_from_slice(&2u16.to_le_bytes());
        let mut extra = vec![0u8; PHDR_SIZE];
        extra[0..4].copy_from_slice(&PT_INTERP.to_le_bytes());
        let insert_at = EHDR_SIZE + PHDR_SIZE;
        bytes.splice(insert_at..insert_at, extra);
        let new_off = (EHDR_SIZE + 2 * PHDR_SIZE) as u32;
        bytes[52 + 4..52 + 8].copy_from_slice(&new_off.to_le_bytes());
        assert_eq!(parse_elf(&bytes), Err(PhaseStackError::AbiRejected));
    }

    #[test]
    fn thumb_entry_is_rejected() {
        let mut bytes = encode_a32_exec_elf(&halt_a32()).unwrap();
        bytes[24..28].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(parse_elf(&bytes), Err(PhaseStackError::AbiRejected));
    }

    #[test]
    fn raw_a32_status_is_not_a_blanket_blocked() {
        let raw = mmu_wx_status();
        let elf = elf_execute_status();
        assert_ne!(raw, elf);
        #[cfg(not(all(target_arch = "arm", feature = "mmu-app-isolation")))]
        {
            assert_eq!(raw, IsolationStatus::Unavailable);
            assert_eq!(elf, IsolationStatus::ElfRefused);
        }
        #[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
        assert!(
            raw == IsolationStatus::Blocked || raw == IsolationStatus::RawA32Ready,
            "ARM isolation build must report Blocked or RawA32Ready, got {raw:?}"
        );
    }
}
