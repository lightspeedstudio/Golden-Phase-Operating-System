//! PhaseFS — Phi-tagged hosted filesystem facade.

use crate::core::gphase_core::{compose, PhaseState};
use crate::fs::block::{BlockError, BlockStore};
use crate::fs::directory::{normalize_path, path_components, split_parent_name, DirectoryEntry};
use crate::fs::inode::{Inode, InodeId, InodeKind};
use crate::platform::{HashMap, String, ToString, Vec};
use crate::protocol::encoder::{self, EncodeError};
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    Path(crate::fs::directory::PathError),
    NotFound(String),
    AlreadyExists(String),
    NotDirectory(String),
    NotFile(String),
    NotEmpty(String),
    Locked(String),
    Block(BlockError),
    Encode(EncodeError),
    InvalidOperation(String),
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsError::Path(e) => write!(f, "{e}"),
            FsError::NotFound(p) => write!(f, "not found: {p}"),
            FsError::AlreadyExists(p) => write!(f, "already exists: {p}"),
            FsError::NotDirectory(p) => write!(f, "not a directory: {p}"),
            FsError::NotFile(p) => write!(f, "not a file: {p}"),
            FsError::NotEmpty(p) => write!(f, "directory not empty: {p}"),
            FsError::Locked(p) => write!(f, "locked: {p}"),
            FsError::Block(e) => write!(f, "{e}"),
            FsError::Encode(e) => write!(f, "{e}"),
            FsError::InvalidOperation(msg) => write!(f, "{msg}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FsError {}

impl From<crate::fs::directory::PathError> for FsError {
    fn from(value: crate::fs::directory::PathError) -> Self {
        FsError::Path(value)
    }
}

impl From<BlockError> for FsError {
    fn from(value: BlockError) -> Self {
        FsError::Block(value)
    }
}

impl From<EncodeError> for FsError {
    fn from(value: EncodeError) -> Self {
        FsError::Encode(value)
    }
}

#[derive(Debug, Clone)]
struct JournalRecord {
    path: String,
    data: Vec<u8>,
}

pub struct PhaseFs {
    blocks: BlockStore,
    inodes: HashMap<InodeId, Inode>,
    entries: HashMap<InodeId, Vec<DirectoryEntry>>,
    root: InodeId,
    next_inode: InodeId,
    journal_enabled: bool,
    pending_journal: Option<JournalRecord>,
}

impl Default for PhaseFs {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseFs {
    pub fn new() -> Self {
        let mut fs = Self {
            blocks: BlockStore::new(),
            inodes: HashMap::new(),
            entries: HashMap::new(),
            root: 0,
            next_inode: 1,
            journal_enabled: false,
            pending_journal: None,
        };
        let root_block = fs.blocks.alloc(Some("/".into()));
        fs.blocks
            .tag(root_block, PhaseState::Phi2)
            .expect("root block tag");
        fs.blocks
            .write_data(root_block, Vec::new())
            .expect("root block data");

        let root = Inode::new_directory(0, String::new(), None);
        let mut root_inode = root;
        root_inode.blocks.push(root_block);
        fs.inodes.insert(0, root_inode);
        fs.entries.insert(0, Vec::new());
        fs
    }

    pub fn set_journal_enabled(&mut self, enabled: bool) {
        self.journal_enabled = enabled;
    }

    pub fn recover_journal(&mut self) -> Result<(), FsError> {
        if let Some(record) = self.pending_journal.take() {
            self.write_file_inner(&record.path, &record.data, true)?;
        }
        Ok(())
    }

    #[cfg(any(test, debug_assertions))]
    pub fn set_pending_journal_for_test(&mut self, path: &str, data: &[u8]) {
        self.pending_journal = Some(JournalRecord {
            path: path.to_string(),
            data: data.to_vec(),
        });
    }

    pub fn simulate_journal_interrupt(&mut self) {
        // pending_journal remains for recover_journal()
    }

    pub fn mkdir(&mut self, path: &str) -> Result<(), FsError> {
        let normalized = normalize_path(path)?;
        if self.resolve_inode(&normalized).is_ok() {
            return Err(FsError::AlreadyExists(normalized));
        }
        let (parent_path, name) = split_parent_name(&normalized)?;
        let parent = self.resolve_inode(&parent_path)?;
        self.ensure_directory(parent, &parent_path)?;

        let inode_id = self.next_inode;
        self.next_inode += 1;
        let meta_block = self.blocks.alloc(Some(normalized.clone()));
        self.blocks.tag(meta_block, PhaseState::Phi2)?;
        self.blocks.write_data(meta_block, Vec::new())?;

        let mut inode = Inode::new_directory(inode_id, name, Some(parent));
        inode.blocks.push(meta_block);
        self.inodes.insert(inode_id, inode);
        self.entries.insert(inode_id, Vec::new());
        self.entries
            .get_mut(&parent)
            .expect("parent entries")
            .push(DirectoryEntry {
                name: self.inodes.get(&inode_id).unwrap().name.clone(),
                inode: inode_id,
            });
        Ok(())
    }

    pub fn create_file(&mut self, path: &str) -> Result<(), FsError> {
        let normalized = normalize_path(path)?;
        if self.resolve_inode(&normalized).is_ok() {
            return Err(FsError::AlreadyExists(normalized));
        }
        let (parent_path, name) = split_parent_name(&normalized)?;
        let parent = self.resolve_inode(&parent_path)?;
        self.ensure_directory(parent, &parent_path)?;

        let inode_id = self.next_inode;
        self.next_inode += 1;
        let data_block = self.blocks.alloc(Some(normalized.clone()));
        self.blocks.tag(data_block, PhaseState::Phi1)?;

        let mut inode = Inode::new_file(inode_id, name, Some(parent));
        inode.blocks.push(data_block);
        self.inodes.insert(inode_id, inode);
        self.entries
            .get_mut(&parent)
            .expect("parent entries")
            .push(DirectoryEntry {
                name: self.inodes.get(&inode_id).unwrap().name.clone(),
                inode: inode_id,
            });
        Ok(())
    }

    pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), FsError> {
        if self.journal_enabled {
            self.pending_journal = Some(JournalRecord {
                path: normalize_path(path)?,
                data: data.to_vec(),
            });
            let result = self.write_file_inner(path, data, true);
            if result.is_ok() {
                self.pending_journal = None;
            }
            result
        } else {
            self.write_file_inner(path, data, true)
        }
    }

    fn write_file_inner(
        &mut self,
        path: &str,
        data: &[u8],
        create_if_missing: bool,
    ) -> Result<(), FsError> {
        encoder::round_trip(data)?;
        let normalized = normalize_path(path)?;
        let inode_id = match self.resolve_inode(&normalized) {
            Ok(id) => id,
            Err(FsError::NotFound(_)) if create_if_missing => {
                self.create_file(&normalized)?;
                self.resolve_inode(&normalized)?
            }
            other => other?,
        };
        self.ensure_file(inode_id, &normalized)?;
        if self.inodes.get(&inode_id).unwrap().tag == PhaseState::Phi3 {
            return Err(FsError::Locked(normalized));
        }
        let block_id = *self
            .inodes
            .get(&inode_id)
            .unwrap()
            .blocks
            .first()
            .ok_or_else(|| FsError::InvalidOperation("file has no data block".into()))?;
        self.blocks.write_data(block_id, data.to_vec())?;
        self.blocks.tag(block_id, PhaseState::Phi1)?;
        Ok(())
    }

    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>, FsError> {
        let normalized = normalize_path(path)?;
        let inode_id = self.resolve_inode(&normalized)?;
        self.ensure_file(inode_id, &normalized)?;
        self.read_file_data(inode_id)
    }

    /// Read file content directly by inode id, skipping path resolution
    /// entirely. Callers (e.g. `kernel::syscall`'s `PhaseCache`-backed
    /// `FS_READ_FILE` handler) must already have `inode_id` from a trusted
    /// source; an id that no longer refers to a live file returns `FsError`
    /// exactly like `read_file` would for a stale path.
    pub fn read_file_by_inode(&mut self, inode_id: InodeId) -> Result<Vec<u8>, FsError> {
        let name = self
            .inodes
            .get(&inode_id)
            .map(|inode| inode.name.clone())
            .unwrap_or_default();
        self.ensure_file(inode_id, &name)?;
        self.read_file_data(inode_id)
    }

    fn read_file_data(&self, inode_id: InodeId) -> Result<Vec<u8>, FsError> {
        let inode = self.inodes.get(&inode_id).unwrap();
        let block_id = *inode
            .blocks
            .first()
            .ok_or_else(|| FsError::InvalidOperation("file has no data block".into()))?;
        let data = self.blocks.get(block_id)?.data.clone();
        encoder::round_trip(&data)?;
        Ok(data)
    }

    pub fn delete_file(&mut self, path: &str) -> Result<(), FsError> {
        let normalized = normalize_path(path)?;
        let inode_id = self.resolve_inode(&normalized)?;
        self.ensure_file(inode_id, &normalized)?;
        if self.inodes.get(&inode_id).unwrap().tag == PhaseState::Phi3 {
            return Err(FsError::Locked(normalized));
        }
        let parent = self
            .inodes
            .get(&inode_id)
            .unwrap()
            .parent
            .ok_or_else(|| FsError::InvalidOperation("orphan inode".into()))?;
        self.remove_entry(parent, &self.inodes.get(&inode_id).unwrap().name.clone());
        for block_id in self.inodes.get(&inode_id).unwrap().blocks.clone() {
            self.blocks.free(block_id)?;
        }
        self.inodes.get_mut(&inode_id).unwrap().mark_deleted();
        self.inodes.remove(&inode_id);
        Ok(())
    }

    pub fn rm(&mut self, path: &str) -> Result<(), FsError> {
        let normalized = normalize_path(path)?;
        if normalized == "/" {
            return Err(FsError::InvalidOperation("cannot remove root".into()));
        }
        let inode_id = self.resolve_inode(&normalized)?;
        match self.inodes.get(&inode_id).unwrap().kind {
            InodeKind::File => self.delete_file(&normalized),
            InodeKind::Directory => self.delete_directory(&normalized),
        }
    }

    fn delete_directory(&mut self, path: &str) -> Result<(), FsError> {
        let normalized = normalize_path(path)?;
        let inode_id = self.resolve_inode(&normalized)?;
        self.ensure_directory(inode_id, &normalized)?;
        if self.inodes.get(&inode_id).unwrap().tag == PhaseState::Phi3 {
            return Err(FsError::Locked(normalized));
        }
        if !self
            .entries
            .get(&inode_id)
            .map(|e| e.is_empty())
            .unwrap_or(true)
        {
            return Err(FsError::NotEmpty(normalized));
        }
        let parent = self
            .inodes
            .get(&inode_id)
            .unwrap()
            .parent
            .ok_or_else(|| FsError::InvalidOperation("orphan inode".into()))?;
        let name = self.inodes.get(&inode_id).unwrap().name.clone();
        self.remove_entry(parent, &name);
        for block_id in self.inodes.get(&inode_id).unwrap().blocks.clone() {
            self.blocks.free(block_id)?;
        }
        self.entries.remove(&inode_id);
        self.inodes.remove(&inode_id);
        Ok(())
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<String>, FsError> {
        let normalized = normalize_path(path)?;
        let inode_id = self.resolve_inode(&normalized)?;
        self.ensure_directory_ro(inode_id, &normalized)?;
        let names: Vec<String> = self
            .entries
            .get(&inode_id)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|e| e.name.clone())
            .collect();
        Ok(names)
    }

    pub fn lock_path(&mut self, path: &str) -> Result<(), FsError> {
        let normalized = normalize_path(path)?;
        let inode_id = self.resolve_inode(&normalized)?;
        let block_ids = self.inodes.get(&inode_id).unwrap().blocks.clone();
        for block_id in block_ids {
            self.blocks.lock(block_id)?;
        }
        self.inodes.get_mut(&inode_id).unwrap().tag = PhaseState::Phi3;
        Ok(())
    }

    pub fn unlock_path(&mut self, path: &str) -> Result<(), FsError> {
        let normalized = normalize_path(path)?;
        let inode_id = self.resolve_inode(&normalized)?;
        let kind = self.inodes.get(&inode_id).unwrap().kind;
        let block_ids = self.inodes.get(&inode_id).unwrap().blocks.clone();
        for block_id in block_ids {
            match kind {
                InodeKind::File => self.blocks.unlock(block_id)?,
                InodeKind::Directory => self.blocks.unlock_as_directory(block_id)?,
            }
        }
        self.inodes.get_mut(&inode_id).unwrap().tag = match kind {
            InodeKind::File => PhaseState::Phi1,
            InodeKind::Directory => PhaseState::Phi2,
        };
        Ok(())
    }

    pub fn stat(&self, path: &str) -> Result<PhaseState, FsError> {
        let normalized = normalize_path(path)?;
        let inode_id = self.resolve_inode(&normalized)?;
        Ok(self.inodes.get(&inode_id).unwrap().tag)
    }

    pub fn contention_check(&self, path_a: &str, path_b: &str) -> Result<PhaseState, FsError> {
        let a = self.stat(path_a)?;
        let b = self.stat(path_b)?;
        Ok(compose(a, b))
    }

    pub fn block_tag(&self, path: &str) -> Result<PhaseState, FsError> {
        let normalized = normalize_path(path)?;
        let inode_id = self.resolve_inode(&normalized)?;
        let block_id = *self
            .inodes
            .get(&inode_id)
            .unwrap()
            .blocks
            .first()
            .ok_or_else(|| FsError::InvalidOperation("no block".into()))?;
        Ok(self.blocks.get(block_id)?.tag)
    }

    /// Public inode-id lookup by path, for callers (e.g. syscalls) that need
    /// the id for `PhaseIndex` bookkeeping alongside a `PhaseFs` mutation.
    pub fn inode_id_for_path(&self, path: &str) -> Result<InodeId, FsError> {
        self.resolve_inode(path)
    }

    /// Read-only accessor for one inode by id.
    pub fn inode(&self, id: InodeId) -> Option<&Inode> {
        self.inodes.get(&id)
    }

    /// Iterate all live inodes (`PhaseIndex` rebuilds / diagnostics).
    pub fn iter_inodes(&self) -> impl Iterator<Item = &Inode> {
        self.inodes.values()
    }

    /// Stamp an inode's `chronos`/`kairos_sector` from the live `StateClock`
    /// at create/mutate time. Returns the stamped inode id.
    pub fn stamp_phase_time(
        &mut self,
        path: &str,
        chronos: u64,
        kairos_sector: u8,
    ) -> Result<InodeId, FsError> {
        let normalized = normalize_path(path)?;
        let inode_id = self.resolve_inode(&normalized)?;
        if let Some(inode) = self.inodes.get_mut(&inode_id) {
            inode.chronos = chronos;
            inode.kairos_sector = kairos_sector;
        }
        Ok(inode_id)
    }

    /// Phase-state histograms for operator `phase` / `dashboard` views.
    pub fn phase_histograms(&self) -> ([usize; 4], [usize; 4]) {
        let mut inode_counts = [0usize; 4];
        for inode in self.inodes.values() {
            inode_counts[inode.tag.as_u8() as usize] += 1;
        }
        (inode_counts, self.blocks.phase_histogram())
    }

    pub fn locked_object_count(&self) -> usize {
        let (inode_h, block_h) = self.phase_histograms();
        inode_h[PhaseState::Phi3.as_u8() as usize] + block_h[PhaseState::Phi3.as_u8() as usize]
    }

    fn resolve_inode(&self, path: &str) -> Result<InodeId, FsError> {
        let normalized = normalize_path(path)?;
        if normalized == "/" {
            return Ok(self.root);
        }
        let components = path_components(&normalized)?;
        let mut current = self.root;
        for component in components {
            let entry = self
                .entries
                .get(&current)
                .and_then(|entries| entries.iter().find(|e| e.name == component))
                .ok_or_else(|| FsError::NotFound(normalized.clone()))?;
            current = entry.inode;
        }
        if self.inodes.contains_key(&current) {
            Ok(current)
        } else {
            Err(FsError::NotFound(normalized))
        }
    }

    fn ensure_directory(&self, inode_id: InodeId, path: &str) -> Result<(), FsError> {
        self.ensure_directory_ro(inode_id, path)
    }

    fn ensure_directory_ro(&self, inode_id: InodeId, path: &str) -> Result<(), FsError> {
        match self.inodes.get(&inode_id).map(|i| i.kind) {
            Some(InodeKind::Directory) => Ok(()),
            Some(InodeKind::File) => Err(FsError::NotDirectory(path.to_string())),
            None => Err(FsError::NotFound(path.to_string())),
        }
    }

    fn ensure_file(&self, inode_id: InodeId, path: &str) -> Result<(), FsError> {
        match self.inodes.get(&inode_id).map(|i| i.kind) {
            Some(InodeKind::File) => Ok(()),
            Some(InodeKind::Directory) => Err(FsError::NotFile(path.to_string())),
            None => Err(FsError::NotFound(path.to_string())),
        }
    }

    fn remove_entry(&mut self, parent: InodeId, name: &str) {
        if let Some(entries) = self.entries.get_mut(&parent) {
            entries.retain(|e| e.name != name);
        }
    }

    /// Rename a file or directory. Source must exist; destination must not.
    /// Does not write the WAL. `persist_rename` journals this when
    /// `phasefs-journal` is enabled.
    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), FsError> {
        let from_n = normalize_path(from)?;
        let to_n = normalize_path(to)?;
        if from_n == "/" || to_n == "/" {
            return Err(FsError::Path(crate::fs::directory::PathError::IsRoot));
        }
        if from_n == to_n {
            return Ok(());
        }
        if to_n.starts_with(&format!("{from_n}/")) {
            return Err(FsError::InvalidOperation(
                "cannot rename into own subtree".into(),
            ));
        }
        let id = self.resolve_inode(&from_n)?;
        if self.inodes.get(&id).unwrap().tag == PhaseState::Phi3 {
            return Err(FsError::Locked(from_n));
        }
        if self.resolve_inode(&to_n).is_ok() {
            return Err(FsError::AlreadyExists(to_n));
        }
        let (to_parent_path, to_name) = split_parent_name(&to_n)?;
        let to_parent = self.resolve_inode(&to_parent_path)?;
        self.ensure_directory(to_parent, &to_parent_path)?;
        let from_parent = self
            .inodes
            .get(&id)
            .unwrap()
            .parent
            .ok_or_else(|| FsError::InvalidOperation("orphan inode".into()))?;
        let from_name = self.inodes.get(&id).unwrap().name.clone();
        self.remove_entry(from_parent, &from_name);
        self.entries
            .get_mut(&to_parent)
            .expect("parent entries")
            .push(DirectoryEntry {
                name: to_name.clone(),
                inode: id,
            });
        let inode = self.inodes.get_mut(&id).unwrap();
        inode.name = to_name;
        inode.parent = Some(to_parent);
        self.rewrite_owner_paths(id, &to_n)?;
        Ok(())
    }

    fn rewrite_owner_paths(&mut self, id: InodeId, new_path: &str) -> Result<(), FsError> {
        let blocks = self
            .inodes
            .get(&id)
            .map(|inode| inode.blocks.clone())
            .unwrap_or_default();
        for bid in blocks {
            self.blocks.set_owner_path(bid, Some(new_path.into()))?;
        }
        let children: Vec<(InodeId, String)> = self
            .entries
            .get(&id)
            .map(|es| es.iter().map(|e| (e.inode, e.name.clone())).collect())
            .unwrap_or_default();
        for (cid, name) in children {
            let child_path = format!("{new_path}/{name}");
            self.rewrite_owner_paths(cid, &child_path)?;
        }
        Ok(())
    }

    /// Owner path stored on the inode's primary block, if any.
    pub fn owner_path_for(&self, path: &str) -> Result<Option<String>, FsError> {
        let id = self.resolve_inode(path)?;
        let bid = *self
            .inodes
            .get(&id)
            .and_then(|inode| inode.blocks.first())
            .ok_or_else(|| FsError::InvalidOperation("no block".into()))?;
        Ok(self.blocks.get(bid)?.owner_path.clone())
    }
}

#[cfg(feature = "phasefs-journal")]
impl PhaseFs {
    pub fn root_id(&self) -> InodeId {
        self.root
    }

    pub fn next_inode_id(&self) -> InodeId {
        self.next_inode
    }

    pub fn next_block_id(&self) -> crate::fs::block::BlockId {
        self.blocks.next_id()
    }

    pub fn dir_entries(&self, id: InodeId) -> Option<&[DirectoryEntry]> {
        self.entries.get(&id).map(|e| e.as_slice())
    }

    pub fn primary_block_data(&self, id: InodeId) -> Option<Vec<u8>> {
        let inode = self.inodes.get(&id)?;
        let bid = *inode.blocks.first()?;
        self.blocks.get(bid).ok().map(|b| b.data.clone())
    }

    pub fn primary_block_owner(&self, id: InodeId) -> Option<String> {
        let inode = self.inodes.get(&id)?;
        let bid = *inode.blocks.first()?;
        self.blocks.get(bid).ok().and_then(|b| b.owner_path.clone())
    }

    pub fn block_phase(&self, id: crate::fs::block::BlockId) -> Option<PhaseState> {
        self.blocks.get(id).ok().map(|b| b.tag)
    }

    pub fn from_parts(
        blocks: crate::platform::HashMap<crate::fs::block::BlockId, crate::fs::block::PhaseBlock>,
        inodes: crate::platform::HashMap<InodeId, Inode>,
        entries: crate::platform::HashMap<InodeId, Vec<DirectoryEntry>>,
        root: InodeId,
        next_inode: InodeId,
        next_block: crate::fs::block::BlockId,
    ) -> Self {
        Self {
            blocks: crate::fs::block::BlockStore::from_parts(blocks, next_block),
            inodes,
            entries,
            root,
            next_inode,
            journal_enabled: false,
            pending_journal: None,
        }
    }

    pub fn apply_redo(&mut self, op: &crate::fs::journal::RedoOp) -> Result<(), FsError> {
        use crate::fs::journal::RedoOp;
        match op {
            RedoOp::Create { path } => self.apply_create(path),
            RedoOp::Write { path, data } => self.write_file(path, data),
            RedoOp::Mkdir { path } => self.apply_mkdir(path),
            RedoOp::Rename { from, to } => self.apply_rename(from, to),
            RedoOp::Remove { path } => self.apply_remove(path),
            RedoOp::Lock { path } => self.apply_lock(path),
            RedoOp::Unlock { path } => self.apply_unlock(path),
            RedoOp::Stamp {
                path,
                chronos,
                kairos_sector,
            } => {
                let _ = self.stamp_phase_time(path, *chronos, *kairos_sector)?;
                Ok(())
            }
        }
    }

    fn apply_create(&mut self, path: &str) -> Result<(), FsError> {
        match self.create_file(path) {
            Ok(()) => Ok(()),
            Err(FsError::AlreadyExists(_)) => {
                let id = self.resolve_inode(path)?;
                self.ensure_file(id, path)
            }
            Err(e) => Err(e),
        }
    }

    fn apply_mkdir(&mut self, path: &str) -> Result<(), FsError> {
        match self.mkdir(path) {
            Ok(()) => Ok(()),
            Err(FsError::AlreadyExists(_)) => {
                let id = self.resolve_inode(path)?;
                self.ensure_directory(id, path)
            }
            Err(e) => Err(e),
        }
    }

    fn apply_remove(&mut self, path: &str) -> Result<(), FsError> {
        match self.rm(path) {
            Ok(()) => Ok(()),
            Err(FsError::NotFound(_)) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn apply_lock(&mut self, path: &str) -> Result<(), FsError> {
        let normalized = normalize_path(path)?;
        let id = match self.resolve_inode(&normalized) {
            Ok(id) => id,
            Err(FsError::NotFound(_)) => return Ok(()),
            Err(e) => return Err(e),
        };
        if self.inodes.get(&id).unwrap().tag == PhaseState::Phi3 {
            return Ok(());
        }
        self.lock_path(&normalized)
    }

    fn apply_unlock(&mut self, path: &str) -> Result<(), FsError> {
        let normalized = normalize_path(path)?;
        let id = match self.resolve_inode(&normalized) {
            Ok(id) => id,
            Err(FsError::NotFound(_)) => return Ok(()),
            Err(e) => return Err(e),
        };
        if self.inodes.get(&id).unwrap().tag != PhaseState::Phi3 {
            return Ok(());
        }
        self.unlock_path(&normalized)
    }

    fn apply_rename(&mut self, from: &str, to: &str) -> Result<(), FsError> {
        let from_n = normalize_path(from)?;
        let to_n = normalize_path(to)?;
        let src = self.resolve_inode(&from_n);
        let dst = self.resolve_inode(&to_n);
        match (src, dst) {
            (Err(FsError::NotFound(_)), Ok(_)) => Ok(()),
            (Ok(a), Ok(b)) if a == b => Ok(()),
            (Ok(_), Err(FsError::NotFound(_))) => self.rename(&from_n, &to_n),
            (Ok(_), Ok(_)) => Err(FsError::AlreadyExists(to_n)),
            (Err(e), _) => Err(e),
            (_, Err(e)) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::directory::PathError;

    #[test]
    fn rename_file_succeeds_and_preserves_data_and_phase() {
        let mut fs = PhaseFs::new();
        fs.write_file("/old.txt", b"payload").unwrap();
        let id = fs.inode_id_for_path("/old.txt").unwrap();
        let tag = fs.stat("/old.txt").unwrap();
        assert_eq!(
            fs.owner_path_for("/old.txt").unwrap().as_deref(),
            Some("/old.txt")
        );

        fs.rename("/old.txt", "/new.txt").unwrap();

        assert!(matches!(
            fs.read_file("/old.txt"),
            Err(FsError::NotFound(_))
        ));
        assert_eq!(fs.read_file("/new.txt").unwrap(), b"payload");
        assert_eq!(fs.inode_id_for_path("/new.txt").unwrap(), id);
        assert_eq!(fs.stat("/new.txt").unwrap(), tag);
        assert_eq!(fs.inode(id).unwrap().name, "new.txt");
        assert_eq!(
            fs.owner_path_for("/new.txt").unwrap().as_deref(),
            Some("/new.txt")
        );
    }

    #[test]
    fn rename_directory_succeeds_and_rewrites_child_owner_paths() {
        let mut fs = PhaseFs::new();
        fs.mkdir("/docs").unwrap();
        fs.write_file("/docs/a.txt", b"child").unwrap();
        let dir_id = fs.inode_id_for_path("/docs").unwrap();
        let file_id = fs.inode_id_for_path("/docs/a.txt").unwrap();

        fs.rename("/docs", "/papers").unwrap();

        assert!(matches!(fs.list_dir("/docs"), Err(FsError::NotFound(_))));
        assert_eq!(fs.list_dir("/papers").unwrap(), vec!["a.txt".to_string()]);
        assert_eq!(fs.read_file("/papers/a.txt").unwrap(), b"child");
        assert_eq!(fs.inode_id_for_path("/papers").unwrap(), dir_id);
        assert_eq!(fs.inode_id_for_path("/papers/a.txt").unwrap(), file_id);
        assert_eq!(
            fs.owner_path_for("/papers").unwrap().as_deref(),
            Some("/papers")
        );
        assert_eq!(
            fs.owner_path_for("/papers/a.txt").unwrap().as_deref(),
            Some("/papers/a.txt")
        );
    }

    #[test]
    fn rename_refuses_missing_source() {
        let mut fs = PhaseFs::new();
        assert!(matches!(
            fs.rename("/missing.txt", "/x.txt"),
            Err(FsError::NotFound(_))
        ));
    }

    #[test]
    fn rename_refuses_destination_collision() {
        let mut fs = PhaseFs::new();
        fs.write_file("/a.txt", b"a").unwrap();
        fs.write_file("/b.txt", b"b").unwrap();
        assert!(matches!(
            fs.rename("/a.txt", "/b.txt"),
            Err(FsError::AlreadyExists(_))
        ));
        assert_eq!(fs.read_file("/a.txt").unwrap(), b"a");
        assert_eq!(fs.read_file("/b.txt").unwrap(), b"b");
    }

    #[test]
    fn rename_refuses_invalid_path() {
        let mut fs = PhaseFs::new();
        fs.write_file("/ok.txt", b"x").unwrap();
        assert!(matches!(
            fs.rename("relative.txt", "/x.txt"),
            Err(FsError::Path(PathError::NotAbsolute(_)))
        ));
        assert!(matches!(
            fs.rename("/ok.txt", "/../x.txt"),
            Err(FsError::Path(PathError::ParentReference))
        ));
        assert!(matches!(
            fs.rename("/", "/elsewhere"),
            Err(FsError::Path(PathError::IsRoot))
        ));
    }

    #[test]
    fn rename_refuses_into_own_subtree() {
        let mut fs = PhaseFs::new();
        fs.mkdir("/a").unwrap();
        fs.mkdir("/a/b").unwrap();
        assert!(matches!(
            fs.rename("/a", "/a/b/c"),
            Err(FsError::InvalidOperation(_))
        ));
        assert!(fs.list_dir("/a").is_ok());
    }
}
