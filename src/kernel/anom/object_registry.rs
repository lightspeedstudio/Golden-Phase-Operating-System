//! Object protection registry — tracks protected objects and persistence.

use crate::kernel::anom::object_policy::{
    ObjectProtectionRecord, ObjectProtectionStatus, ProtectedObjectId, ProtectedObjectKind,
};
use crate::kernel::anom::object_record::ProtectedPhaseRecord;
use crate::kernel::hash::StateHash;
use crate::platform::{HashMap, String, Vec};

pub const OBJECT_STORE_DIR: &str = ".kairos/anom/objects";

#[derive(Debug, Default)]
pub struct ObjectProtectionRegistry {
    records: HashMap<String, ObjectProtectionRecord>,
    phase_records: HashMap<String, Vec<ProtectedPhaseRecord>>,
    latest_commitment: HashMap<String, StateHash>,
    craft_slots: HashMap<String, u32>,
    next_craft_slot: u32,
}

impl ObjectProtectionRegistry {
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
            phase_records: HashMap::new(),
            latest_commitment: HashMap::new(),
            craft_slots: HashMap::new(),
            next_craft_slot: 20,
        }
    }

    pub fn is_protected(&self, object_id: &ProtectedObjectId) -> bool {
        self.records
            .get(&object_id.canonical())
            .map(|r| {
                matches!(
                    r.status,
                    ObjectProtectionStatus::ProtectedLocked | ObjectProtectionStatus::ProtectedOpen
                )
            })
            .unwrap_or(false)
    }

    pub fn is_open(&self, object_id: &ProtectedObjectId) -> bool {
        self.records
            .get(&object_id.canonical())
            .map(|r| r.status == ObjectProtectionStatus::ProtectedOpen)
            .unwrap_or(false)
    }

    pub fn record(&self, object_id: &ProtectedObjectId) -> Option<&ObjectProtectionRecord> {
        self.records.get(&object_id.canonical())
    }

    pub fn record_mut(
        &mut self,
        object_id: &ProtectedObjectId,
    ) -> Option<&mut ObjectProtectionRecord> {
        self.records.get_mut(&object_id.canonical())
    }

    pub fn insert_record(&mut self, record: ObjectProtectionRecord) {
        let key = record.object_id.canonical();
        self.latest_commitment
            .insert(key.clone(), record.commitment_root);
        self.records.insert(key, record);
    }

    pub fn set_status(&mut self, object_id: &ProtectedObjectId, status: ObjectProtectionStatus) {
        if let Some(r) = self.records.get_mut(&object_id.canonical()) {
            r.status = status;
        }
    }

    pub fn public_commitment(&self, object_id: &ProtectedObjectId) -> Option<StateHash> {
        self.latest_commitment
            .get(&object_id.canonical())
            .copied()
            .or_else(|| {
                self.records
                    .get(&object_id.canonical())
                    .map(|r| r.commitment_root)
            })
    }

    pub fn chain_head(&self, object_id: &ProtectedObjectId) -> Option<StateHash> {
        self.records
            .get(&object_id.canonical())
            .map(|r| r.current_statechain_head)
    }

    pub fn protection_version(&self, object_id: &ProtectedObjectId) -> Option<u32> {
        self.records.get(&object_id.canonical()).map(|r| r.version)
    }

    pub fn append_phase_record(&mut self, record: ProtectedPhaseRecord) {
        let key = record.object_id.canonical();
        self.latest_commitment
            .insert(key.clone(), record.public_commitment);
        self.phase_records.entry(key).or_default().push(record);
    }

    pub fn latest_phase_record(
        &self,
        object_id: &ProtectedObjectId,
    ) -> Option<&ProtectedPhaseRecord> {
        self.phase_records
            .get(&object_id.canonical())
            .and_then(|v| v.last())
    }

    pub fn phase_records(&self, object_id: &ProtectedObjectId) -> &[ProtectedPhaseRecord] {
        self.phase_records
            .get(&object_id.canonical())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn list_protected(&self) -> Vec<&ObjectProtectionRecord> {
        self.records
            .values()
            .filter(|r| {
                matches!(
                    r.status,
                    ObjectProtectionStatus::ProtectedLocked | ObjectProtectionStatus::ProtectedOpen
                )
            })
            .collect()
    }

    pub fn protected_count(&self) -> usize {
        self.list_protected().len()
    }

    pub fn locked_count(&self) -> usize {
        self.records
            .values()
            .filter(|r| r.status == ObjectProtectionStatus::ProtectedLocked)
            .count()
    }

    pub fn open_count(&self) -> usize {
        self.records
            .values()
            .filter(|r| r.status == ObjectProtectionStatus::ProtectedOpen)
            .count()
    }

    pub fn craft_slot_lookup(&self, name: &str) -> Option<u32> {
        self.craft_slots.get(name).copied()
    }

    pub fn craft_slot(&mut self, name: &str) -> u32 {
        if let Some(&slot) = self.craft_slots.get(name) {
            return slot;
        }
        let slot = self.next_craft_slot;
        self.next_craft_slot += 1;
        self.craft_slots.insert(name.into(), slot);
        slot
    }

    pub fn mark_inactive(&mut self, object_id: &ProtectedObjectId) {
        if let Some(r) = self.records.get_mut(&object_id.canonical()) {
            r.status = ObjectProtectionStatus::Inactive;
        }
    }

    pub fn filename_for(object_id: &ProtectedObjectId) -> String {
        let safe = object_id.canonical().replace(':', "-").replace('/', "_");
        format!("{safe}.kairos")
    }

    #[cfg(feature = "std")]
    pub fn persist_record_host(record: &ObjectProtectionRecord) -> Result<(), std::io::Error> {
        let path = format!(
            "{}/{}",
            OBJECT_STORE_DIR,
            Self::filename_for(&record.object_id)
        );
        let data = serialize_record(record);
        let path_ref = std::path::Path::new(&path);
        let Some(parent) = path_ref.parent() else {
            return std::fs::write(path_ref, data);
        };

        let mut last_err = None;
        for _ in 0..3 {
            if let Err(err) = std::fs::create_dir_all(parent) {
                last_err = Some(err);
                continue;
            }
            match std::fs::write(path_ref, &data) {
                Ok(()) => return Ok(()),
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| std::io::Error::other("persist_record failed")))
    }

    pub fn persist_record(
        fs: &mut crate::fs::PhaseFs,
        record: &ObjectProtectionRecord,
    ) -> Result<(), crate::fs::FsError> {
        let _ = fs.mkdir("/anom");
        match fs.mkdir("/anom/objects") {
            Ok(()) | Err(crate::fs::FsError::AlreadyExists(_)) => {}
            Err(e) => return Err(e),
        }
        let path = format!("/anom/objects/{}", Self::filename_for(&record.object_id));
        fs.write_file(&path, &serialize_record(record))?;
        #[cfg(feature = "std")]
        {
            let _ = Self::persist_record_host(record);
        }
        Ok(())
    }

    #[cfg(feature = "std")]
    pub fn load_all(
        &mut self,
        boot_session_root: &StateHash,
        equation_root: &StateHash,
        statechain: &crate::kernel::statechain::StatechainLedger,
    ) {
        use crate::kernel::state_proof::{hash_payload, ObjectKind};
        let dir = std::path::Path::new(OBJECT_STORE_DIR);
        if !dir.exists() {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(data) = std::fs::read(entry.path()) {
                    if let Some(record) = deserialize_record(&data) {
                        if record.boot_session_root != *boot_session_root {
                            continue;
                        }
                        if record.equation_root != *equation_root {
                            continue;
                        }
                        if record.status == ObjectProtectionStatus::Inactive {
                            continue;
                        }
                        let head_ok = match record.object_id.kind {
                            ProtectedObjectKind::Process => {
                                let Ok(pid) = record.object_id.value.parse::<u32>() else {
                                    continue;
                                };
                                statechain
                                    .chain(ObjectKind::Process, pid)
                                    .and_then(|c| c.head())
                            }
                            ProtectedObjectKind::File => {
                                let digest = hash_payload(record.object_id.value.as_bytes());
                                let id = u16::from_le_bytes([digest[0], digest[1]]) as u32;
                                statechain
                                    .chain(ObjectKind::File, id)
                                    .and_then(|c| c.head())
                            }
                            ProtectedObjectKind::Craft => self
                                .craft_slots
                                .get(&record.object_id.value)
                                .and_then(|&slot| {
                                    statechain
                                        .chain(ObjectKind::Driver, slot)
                                        .and_then(|c| c.head())
                                }),
                            _ => None,
                        } == Some(record.current_statechain_head);
                        if !head_ok {
                            continue;
                        }
                        if record.object_id.kind == ProtectedObjectKind::Craft {
                            let _ = self.craft_slot(&record.object_id.value);
                        }
                        let key = record.object_id.canonical();
                        self.latest_commitment
                            .insert(key.clone(), record.commitment_root);
                        self.records.insert(key, record);
                    }
                }
            }
        }
    }

    pub fn load_from_fs(
        &mut self,
        fs: &mut crate::fs::PhaseFs,
        boot_session_root: &StateHash,
        equation_root: &StateHash,
        statechain: &crate::kernel::statechain::StatechainLedger,
    ) {
        use crate::kernel::state_proof::{hash_payload, ObjectKind};
        let Ok(names) = fs.list_dir("/anom/objects") else {
            return;
        };
        for name in names {
            let path = format!("/anom/objects/{name}");
            let Ok(data) = fs.read_file(&path) else {
                continue;
            };
            let Some(record) = deserialize_record(&data) else {
                continue;
            };
            if record.boot_session_root != *boot_session_root {
                continue;
            }
            if record.equation_root != *equation_root {
                continue;
            }
            if record.status == ObjectProtectionStatus::Inactive {
                continue;
            }
            let head_ok = match record.object_id.kind {
                ProtectedObjectKind::Process => {
                    let Ok(pid) = record.object_id.value.parse::<u32>() else {
                        continue;
                    };
                    statechain
                        .chain(ObjectKind::Process, pid)
                        .and_then(|c| c.head())
                }
                ProtectedObjectKind::File => {
                    let digest = hash_payload(record.object_id.value.as_bytes());
                    let id = u16::from_le_bytes([digest[0], digest[1]]) as u32;
                    statechain
                        .chain(ObjectKind::File, id)
                        .and_then(|c| c.head())
                }
                ProtectedObjectKind::Craft => self
                    .craft_slots
                    .get(&record.object_id.value)
                    .and_then(|&slot| {
                        statechain
                            .chain(ObjectKind::Driver, slot)
                            .and_then(|c| c.head())
                    }),
                _ => None,
            } == Some(record.current_statechain_head);
            if !head_ok {
                continue;
            }
            if record.object_id.kind == ProtectedObjectKind::Craft {
                let _ = self.craft_slot(&record.object_id.value);
            }
            let key = record.object_id.canonical();
            self.latest_commitment
                .insert(key.clone(), record.commitment_root);
            self.records.insert(key, record);
        }
    }
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn serialize_record(record: &ObjectProtectionRecord) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"KOBJ1");
    out.push(record.object_id.kind as u8);
    write_str(&mut out, &record.object_id.value);
    out.extend_from_slice(&record.version.to_le_bytes());
    out.extend_from_slice(&record.equation_root);
    out.extend_from_slice(&record.boot_session_root);
    write_bytes(&mut out, &record.object_salt);
    write_bytes(&mut out, &record.wrap_nonce);
    write_bytes(&mut out, &record.wrapped_object_key);
    write_bytes(&mut out, &record.wrap_tag);
    out.extend_from_slice(&record.created_sequence.to_le_bytes());
    let rekey = record.last_rekey_sequence.unwrap_or(u64::MAX);
    out.extend_from_slice(&rekey.to_le_bytes());
    out.extend_from_slice(&record.commitment_root);
    out.extend_from_slice(&record.current_statechain_head);
    out.push(record.status as u8);
    out
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn deserialize_record(data: &[u8]) -> Option<ObjectProtectionRecord> {
    use crate::kernel::anom::object_policy::{ObjectProtectionStatus, ProtectedObjectKind};
    if data.len() < 5 || &data[..5] != b"KOBJ1" {
        return None;
    }
    let mut i = 5;
    let kind = match data[i] {
        0 => ProtectedObjectKind::Process,
        1 => ProtectedObjectKind::Memory,
        2 => ProtectedObjectKind::File,
        3 => ProtectedObjectKind::Vm,
        4 => ProtectedObjectKind::Craft,
        5 => ProtectedObjectKind::StateClock,
        6 => ProtectedObjectKind::Network,
        _ => return None,
    };
    i += 1;
    let (value, ni) = read_str(data, i)?;
    i = ni;
    let version = u32::from_le_bytes(data[i..i + 4].try_into().ok()?);
    i += 4;
    let equation_root: StateHash = data[i..i + 32].try_into().ok()?;
    i += 32;
    let boot_session_root: StateHash = data[i..i + 32].try_into().ok()?;
    i += 32;
    let (object_salt, ni) = read_bytes(data, i)?;
    i = ni;
    let (wrap_nonce, ni) = read_bytes(data, i)?;
    i = ni;
    let (wrapped_object_key, ni) = read_bytes(data, i)?;
    i = ni;
    let (wrap_tag, ni) = read_bytes(data, i)?;
    i = ni;
    let created_sequence = u64::from_le_bytes(data[i..i + 8].try_into().ok()?);
    i += 8;
    let rekey_raw = u64::from_le_bytes(data[i..i + 8].try_into().ok()?);
    i += 8;
    let last_rekey_sequence = if rekey_raw == u64::MAX {
        None
    } else {
        Some(rekey_raw)
    };
    let commitment_root: StateHash = data[i..i + 32].try_into().ok()?;
    i += 32;
    let current_statechain_head: StateHash = data[i..i + 32].try_into().ok()?;
    i += 32;
    let status = match data.get(i)? {
        0 => ObjectProtectionStatus::Unprotected,
        1 => ObjectProtectionStatus::ProtectedLocked,
        2 => ObjectProtectionStatus::ProtectedOpen,
        3 => ObjectProtectionStatus::VerificationFailed,
        4 => ObjectProtectionStatus::HostedOnly,
        5 => ObjectProtectionStatus::Inactive,
        _ => ObjectProtectionStatus::ProtectedLocked,
    };
    Some(ObjectProtectionRecord {
        object_id: crate::kernel::anom::object_policy::ProtectedObjectId::new(kind, value),
        version,
        equation_root,
        boot_session_root,
        object_salt,
        wrap_nonce,
        wrapped_object_key,
        wrap_tag,
        created_sequence,
        last_rekey_sequence,
        commitment_root,
        current_statechain_head,
        status,
    })
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn write_str(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn read_str(data: &[u8], i: usize) -> Option<(String, usize)> {
    if data.len() < i + 4 {
        return None;
    }
    let len = u32::from_le_bytes(data[i..i + 4].try_into().ok()?) as usize;
    let start = i + 4;
    let end = start + len;
    let s = String::from_utf8(data.get(start..end)?.to_vec()).ok()?;
    Some((s, end))
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn write_bytes(out: &mut Vec<u8>, b: &[u8]) {
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(b);
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn read_bytes(data: &[u8], i: usize) -> Option<(Vec<u8>, usize)> {
    if data.len() < i + 4 {
        return None;
    }
    let len = u32::from_le_bytes(data[i..i + 4].try_into().ok()?) as usize;
    let start = i + 4;
    let end = start + len;
    Some((data.get(start..end)?.to_vec(), end))
}
