//! Object and capability table (`feature = "captable"`), subsystem 2 of the
//! eight kernel-subsystems build program — see
//! `fpga/zynq/evidence/KERNEL_MATH_CONTRACT_2026-08-12.md` and
//! `fpga/zynq/evidence/CAPTABLE_STATUS_<date>.md`.
//!
//! One generational object identity + authority system, reused by later
//! subsystems instead of each inventing its own handle metadata (Part III of
//! the build contract). Two parallel generation-stamped slot tables:
//!
//! - **Objects** — one slot per real kernel resource this table names:
//!   `File` (`InodeId`), `Queue`, `Network`, `Process` (`ProcessId`),
//!   `Memory` (`PageId`), `Device` (IRQ identity).
//! - **Capabilities** — one slot per outstanding *reference* to an object,
//!   carrying `subject` (owning process), `rights` (bitset), optional
//!   `expiry` (Chronos-tick deadline), and an optional `parent` for
//!   delegation-chain tracking.
//!
//! ## What this reuses / does not reuse (per the math contract, §0.4/§6)
//! - [`crate::kernel::event_log::ProcessId`] as the capability subject/owner
//!   — no new process-identity type.
//! - [`crate::kernel::crypto::phase_hash::ph_content`] (PH-C) to bind each
//!   *object's* identity for cold-path audit only, computed once at object
//!   creation — never inside `authorize`'s hot path, and never treated as an
//!   authorization token (PH-C is unkeyed).
//! - No `PhaseState`/G64/four-state-reflection-quotient integration: per the
//!   math contract's own row for `captable`, none is needed (this subsystem
//!   only ever consumes an opaque, cold-path PH-C binding).
//! - No dependency on `StatechainLedger`'s live accepted root/checkpoint in
//!   the hot `authorize` path (`G_root` is not gated here — see Non-goals).
//!
//! ## Trust gate (math contract §6, specialized to this subsystem)
//! `Allow(c, op, caller, t)` is exactly:
//! 1. `V(c)`      — capability slot occupied, `c.generation` matches.
//! 2. `V(c.object)` — object slot occupied, its generation matches the
//!    generation captured in `c.object` at grant/delegate time.
//! 3. `c.subject == caller`.
//! 4. `rights(op) & !c.rights == 0` (requested rights are a subset).
//! 5. `t < c.expiry` (or `c.expiry` is `None`, meaning unbounded).
//! 6. `delegationChainValid(c)` — walking `c.parent` up to
//!    [`MAX_DELEGATION_DEPTH`] hops, every ancestor must itself still satisfy
//!    `V(ancestor)`.
//!
//! Destroying a capability (`close`) or an object (`destroy_object_for_inode`)
//! increments that slot's generation *before* the slot is ever returned to
//! its free list (math contract §6's `g_i <- g_i + 1`), which is exactly what
//! makes both "stale handle after close" and "parent revocation invalidates
//! children" hold: a revoked ancestor's slot generation no longer matches
//! what any descendant captured, so `V(ancestor)` (step 6 above) fails for
//! every descendant in one O(1) mutation, without a separate revocation-epoch
//! field. `c.object`'s generation check (step 2) gives the same effect for
//! "destroying the object invalidates every outstanding capability to it",
//! independent of delegation.
//!
//! ## Non-goals (explicit — see also `CAPTABLE_STATUS_<date>.md`)
//! - `ObjectKind` is `File`, `Queue`, `Network`, `Process`, `Memory`, and
//!   `Device`. `Device` names an IRQ identity, not IOMMU / bus-master
//!   filtering (H01).
//! - `FS_READ_FILE`/`FS_WRITE_FILE`/`FS_MKDIR`/`FS_RM` require a `File`
//!   capability (`RIGHT_READ` / `RIGHT_WRITE` / `RIGHT_DELETE`). `FS_STAT`/
//!   `FS_LOCK`/`FS_UNLOCK`/`FS_LS`/`FS_RENAME` remain path-based.
//! - No `G_quarantine` conjunct: no fault-injection/quarantine concept is
//!   defined for capability objects in this subsystem.
//! - No persistence/recovery: this table is RAM-only, like `PhaseFs` itself;
//!   `X_before/X_fault/X_restored` recovery mathematics do not apply to a
//!   subsystem with no checkpoint/restore operation of its own (revisited
//!   when `txlog` integrates checkpoint restore across subsystems).
//! - Does not touch, wrap, or duplicate the pre-existing, unrelated
//!   `crate::kernel::anom::object_registry`/`object_protection` (ANOM's
//!   encrypted whole-object protection/persistence system) — different
//!   purpose, different keying (string canonical IDs, not generational
//!   handles), left completely alone.

use core::fmt;

use crate::fs::inode::InodeId;
use crate::kernel::event_log::{PageId, ProcessId};
use crate::kernel::hash::StateHash;
use crate::platform::HashMap;

/// Bounded delegation-chain walk depth (complexity target: `O(D)`).
pub const MAX_DELEGATION_DEPTH: u32 = 8;

pub type Rights = u32;

pub const RIGHT_READ: Rights = 1 << 0;
pub const RIGHT_WRITE: Rights = 1 << 1;
pub const RIGHT_DELETE: Rights = 1 << 2;
/// Required to call [`CapabilityTable::delegate`] using a capability as the
/// parent.
pub const RIGHT_GRANT: Rights = 1 << 3;

/// `h = (i, g)` — the shared generational-handle shape (Part III of the build
/// contract). Used both for object handles and capability handles in this
/// module; later subsystems may reuse this exact type for their own tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenerationalHandle {
    pub slot: u32,
    pub generation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    File,
    /// Added for `ipc` (subsystem 5). Bound 1:1 to a packed
    /// `kernel::ipc::IpcHandle` (`ObjectSlot::queue_ref`), the same
    /// "capability names an external resource id" shape `File` already
    /// uses for `InodeId`.
    Queue,
    /// Added for `netsess` (subsystem 8). Bound 1:1 to a packed
    /// `kernel::netsess::SessionHandle` (`ObjectSlot::network_ref`), the
    /// same "capability names an external resource id" shape `Queue`
    /// already uses for `IpcHandle`.
    Network,
    /// Bound 1:1 to [`ProcessId`] (`ObjectSlot::resource_id`).
    Process,
    /// Bound 1:1 to [`PageId`] (`ObjectSlot::resource_id`).
    Memory,
    /// Bound 1:1 to an IRQ identity (`ObjectSlot::resource_id`). Not IOMMU.
    Device,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapError {
    /// Slot index out of range, or not currently occupied.
    InvalidHandle,
    /// Slot occupied, but by a different (newer) generation — the classic
    /// stale-handle-after-reuse case.
    StaleGeneration,
    /// The object this capability names no longer exists (destroyed, and its
    /// slot's generation has since advanced).
    ObjectGone,
    /// `c.subject != caller`.
    WrongSubject,
    /// Requested rights are not a subset of the capability's granted rights.
    RightsEscalation,
    /// `t >= c.expiry`.
    Expired,
    /// An ancestor in the delegation chain is no longer valid (closed,
    /// stale, or its object is gone).
    DelegationChainBroken,
    /// Delegation chain exceeds `MAX_DELEGATION_DEPTH`.
    MaxDepthExceeded,
    /// `delegate`'s requested rights are not a subset of the parent's rights.
    DelegationRightsEscalation,
    /// `delegate`'s requested expiry is later than the parent's (or the
    /// parent is bounded and the child asked for unbounded).
    DelegationExpiryEscalation,
    /// `resolve_queue`/`destroy_queue_via_cap` called on a capability whose
    /// object is not `ObjectKind::Queue` (e.g. a `File` capability).
    WrongObjectKind,
}

impl fmt::Display for CapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            CapError::InvalidHandle => "invalid handle",
            CapError::StaleGeneration => "stale generation",
            CapError::ObjectGone => "object no longer exists",
            CapError::WrongSubject => "wrong subject",
            CapError::RightsEscalation => "rights escalation",
            CapError::Expired => "capability expired",
            CapError::DelegationChainBroken => "delegation chain broken",
            CapError::MaxDepthExceeded => "delegation depth exceeded",
            CapError::DelegationRightsEscalation => "delegation rights escalation",
            CapError::DelegationExpiryEscalation => "delegation expiry escalation",
            CapError::WrongObjectKind => "capability names the wrong object kind",
        };
        f.write_str(msg)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CapTableStats {
    pub objects_created: u64,
    pub objects_destroyed: u64,
    pub opens: u64,
    pub closes: u64,
    pub delegations: u64,
    pub authorize_ok: u64,
    pub authorize_denied_stale: u64,
    pub authorize_denied_rights: u64,
    pub authorize_denied_subject: u64,
    pub authorize_denied_expiry: u64,
    pub authorize_denied_chain: u64,
    pub generation_retired: u64,
}

struct ObjectSlot {
    occupied: bool,
    generation: u32,
    kind: ObjectKind,
    /// Meaningful only when `kind == ObjectKind::File`.
    inode_id: InodeId,
    /// Meaningful only when `kind == ObjectKind::Queue`: a packed
    /// `kernel::ipc::IpcHandle` (`(slot as u64) << 32 | generation as u64`).
    queue_ref: u64,
    /// Meaningful only when `kind == ObjectKind::Network`: a packed
    /// `kernel::netsess::SessionHandle` (`(slot as u64) << 32 | generation
    /// as u64`).
    network_ref: u64,
    /// Meaningful when `kind` is `Process` (`ProcessId`), `Memory`
    /// (`PageId`), or `Device` (IRQ identity).
    resource_id: u64,
    /// PH-C over the object's identity, bound once at creation. Cold-path
    /// audit metadata only — never consulted by `authorize`.
    #[allow(dead_code)]
    content_root: StateHash,
}

struct CapabilitySlot {
    occupied: bool,
    generation: u32,
    object: GenerationalHandle,
    subject: ProcessId,
    rights: Rights,
    /// Chronos-tick deadline; `None` means unbounded.
    expiry: Option<u64>,
    parent: Option<GenerationalHandle>,
}

/// Bumps `*generation`. Returns `true` if the counter would have overflowed
/// (`u32::MAX -> 0`): on overflow the counter is left at `u32::MAX` and the
/// caller must permanently retire the slot (never return it to a free list),
/// per the math contract's "never wrap silently into a valid old generation"
/// rule.
fn bump_generation(generation: &mut u32) -> bool {
    match generation.checked_add(1) {
        Some(next) => {
            *generation = next;
            false
        }
        None => true,
    }
}

/// Generational object/capability table. See module docs for the full trust
/// gate and design rationale.
pub struct CapabilityTable {
    objects: crate::platform::Vec<ObjectSlot>,
    objects_free: crate::platform::Vec<u32>,
    object_by_inode: HashMap<InodeId, u32>,
    object_by_resource: HashMap<(ObjectKind, u64), u32>,
    caps: crate::platform::Vec<CapabilitySlot>,
    caps_free: crate::platform::Vec<u32>,
    stats: CapTableStats,
}

impl Default for CapabilityTable {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityTable {
    pub fn new() -> Self {
        Self {
            objects: crate::platform::Vec::new(),
            objects_free: crate::platform::Vec::new(),
            object_by_inode: HashMap::new(),
            object_by_resource: HashMap::new(),
            caps: crate::platform::Vec::new(),
            caps_free: crate::platform::Vec::new(),
            stats: CapTableStats::default(),
        }
    }

    pub fn stats(&self) -> CapTableStats {
        self.stats
    }

    pub fn object_count(&self) -> usize {
        self.objects.iter().filter(|o| o.occupied).count()
    }

    pub fn capability_count(&self) -> usize {
        self.caps.iter().filter(|c| c.occupied).count()
    }

    fn find_or_create_file_object(&mut self, inode_id: InodeId) -> GenerationalHandle {
        if let Some(&slot) = self.object_by_inode.get(&inode_id) {
            if let Some(obj) = self.objects.get(slot as usize) {
                if obj.occupied {
                    return GenerationalHandle {
                        slot,
                        generation: obj.generation,
                    };
                }
            }
        }
        let content_root = crate::kernel::crypto::phase_hash::ph_content(&inode_id.to_le_bytes());
        let handle = if let Some(slot) = self.objects_free.pop() {
            let obj = &mut self.objects[slot as usize];
            obj.occupied = true;
            obj.kind = ObjectKind::File;
            obj.inode_id = inode_id;
            obj.queue_ref = 0;
            obj.network_ref = 0;
            obj.resource_id = 0;
            obj.content_root = content_root;
            GenerationalHandle {
                slot,
                generation: obj.generation,
            }
        } else {
            let slot = self.objects.len() as u32;
            self.objects.push(ObjectSlot {
                occupied: true,
                generation: 0,
                kind: ObjectKind::File,
                inode_id,
                queue_ref: 0,
                network_ref: 0,
                resource_id: 0,
                content_root,
            });
            GenerationalHandle {
                slot,
                generation: 0,
            }
        };
        self.object_by_inode.insert(inode_id, handle.slot);
        self.stats.objects_created += 1;
        handle
    }

    /// Mint a fresh `Queue` object bound to `queue_ref` (a packed
    /// `kernel::ipc::IpcHandle`) plus a root capability on it for `subject`,
    /// in one call. Unlike `open_file`, there is no dedup-by-existing-ref
    /// lookup: `ipc::IPC_CREATE_QUEUE` always creates a brand-new `ipc`
    /// queue immediately before calling this, so there is never a
    /// pre-existing object to find.
    pub fn open_queue(
        &mut self,
        queue_ref: u64,
        subject: ProcessId,
        rights: Rights,
        expiry: Option<u64>,
    ) -> GenerationalHandle {
        let content_root = crate::kernel::crypto::phase_hash::ph_content(&queue_ref.to_le_bytes());
        let object = if let Some(slot) = self.objects_free.pop() {
            let obj = &mut self.objects[slot as usize];
            obj.occupied = true;
            obj.kind = ObjectKind::Queue;
            obj.inode_id = 0;
            obj.queue_ref = queue_ref;
            obj.network_ref = 0;
            obj.resource_id = 0;
            obj.content_root = content_root;
            GenerationalHandle {
                slot,
                generation: obj.generation,
            }
        } else {
            let slot = self.objects.len() as u32;
            self.objects.push(ObjectSlot {
                occupied: true,
                generation: 0,
                kind: ObjectKind::Queue,
                inode_id: 0,
                queue_ref,
                network_ref: 0,
                resource_id: 0,
                content_root,
            });
            GenerationalHandle {
                slot,
                generation: 0,
            }
        };
        self.stats.objects_created += 1;
        let handle = self.alloc_cap_slot(CapabilitySlot {
            occupied: true,
            generation: 0,
            object,
            subject,
            rights,
            expiry,
            parent: None,
        });
        self.stats.opens += 1;
        handle
    }

    /// Mint an additional root capability on an *existing* `Queue` object
    /// (e.g. so a second process can send/recv on a queue the first process
    /// created), owned by `subject`, gated by the granter already holding
    /// `RIGHT_GRANT` on some capability naming that same object — reuses
    /// `delegate`'s exact authorization step rather than duplicating it.
    pub fn share_queue(
        &mut self,
        existing: GenerationalHandle,
        granter: ProcessId,
        subject: ProcessId,
        rights: Rights,
        expiry: Option<u64>,
        now_chronos: u64,
    ) -> Result<GenerationalHandle, CapError> {
        self.delegate(existing, granter, subject, rights, expiry, now_chronos)
    }

    /// Authorize `requested` on `cap`, then resolve and return the packed
    /// `queue_ref` its object is bound to. Fails `WrongObjectKind` if `cap`
    /// names a `File` object instead.
    pub fn resolve_queue(
        &mut self,
        cap: GenerationalHandle,
        caller: ProcessId,
        requested: Rights,
        now_chronos: u64,
    ) -> Result<u64, CapError> {
        self.authorize(cap, caller, requested, now_chronos)?;
        let object = self.get_cap(cap)?.object;
        let obj = self.get_object(object)?;
        match obj.kind {
            ObjectKind::Queue => Ok(obj.queue_ref),
            _ => Err(CapError::WrongObjectKind),
        }
    }

    /// Mint a fresh `Network` object bound to `network_ref` (a packed
    /// `kernel::netsess::SessionHandle`) plus a root capability on it for
    /// `subject`, in one call. Mirrors `open_queue` exactly (see its docs):
    /// `netsess::Netsess::open_session` always creates a brand-new session
    /// immediately before this is called, so there is never a pre-existing
    /// object to find.
    pub fn open_network(
        &mut self,
        network_ref: u64,
        subject: ProcessId,
        rights: Rights,
        expiry: Option<u64>,
    ) -> GenerationalHandle {
        let content_root =
            crate::kernel::crypto::phase_hash::ph_content(&network_ref.to_le_bytes());
        let object = if let Some(slot) = self.objects_free.pop() {
            let obj = &mut self.objects[slot as usize];
            obj.occupied = true;
            obj.kind = ObjectKind::Network;
            obj.inode_id = 0;
            obj.queue_ref = 0;
            obj.network_ref = network_ref;
            obj.resource_id = 0;
            obj.content_root = content_root;
            GenerationalHandle {
                slot,
                generation: obj.generation,
            }
        } else {
            let slot = self.objects.len() as u32;
            self.objects.push(ObjectSlot {
                occupied: true,
                generation: 0,
                kind: ObjectKind::Network,
                inode_id: 0,
                queue_ref: 0,
                network_ref,
                resource_id: 0,
                content_root,
            });
            GenerationalHandle {
                slot,
                generation: 0,
            }
        };
        self.stats.objects_created += 1;
        let handle = self.alloc_cap_slot(CapabilitySlot {
            occupied: true,
            generation: 0,
            object,
            subject,
            rights,
            expiry,
            parent: None,
        });
        self.stats.opens += 1;
        handle
    }

    /// Authorize `requested` on `cap`, then resolve and return the packed
    /// `network_ref` its object is bound to. Fails `WrongObjectKind` if
    /// `cap` names a `File` or `Queue` object instead. Mirrors
    /// `resolve_queue` exactly.
    pub fn resolve_network(
        &mut self,
        cap: GenerationalHandle,
        caller: ProcessId,
        requested: Rights,
        now_chronos: u64,
    ) -> Result<u64, CapError> {
        self.authorize(cap, caller, requested, now_chronos)?;
        let object = self.get_cap(cap)?.object;
        let obj = self.get_object(object)?;
        match obj.kind {
            ObjectKind::Network => Ok(obj.network_ref),
            _ => Err(CapError::WrongObjectKind),
        }
    }

    /// Authorize `RIGHT_DELETE` on `cap`, then destroy its `Network` object
    /// (invalidating every outstanding capability on it) and close `cap`
    /// itself. Returns the object's `network_ref` so the caller can also
    /// close the real `netsess` session it names. Mirrors
    /// `destroy_queue_via_cap` exactly.
    pub fn destroy_network_via_cap(
        &mut self,
        cap: GenerationalHandle,
        caller: ProcessId,
        now_chronos: u64,
    ) -> Result<u64, CapError> {
        self.authorize(cap, caller, RIGHT_DELETE, now_chronos)?;
        let object = self.get_cap(cap)?.object;
        let network_ref = match self.get_object(object)?.kind {
            ObjectKind::Network => self.get_object(object)?.network_ref,
            _ => return Err(CapError::WrongObjectKind),
        };
        let obj = self
            .objects
            .get_mut(object.slot as usize)
            .expect("object validated by get_object above");
        obj.occupied = false;
        let retired = bump_generation(&mut obj.generation);
        if retired {
            self.stats.generation_retired += 1;
        } else {
            self.objects_free.push(object.slot);
        }
        self.stats.objects_destroyed += 1;
        self.close(cap)?;
        Ok(network_ref)
    }

    /// Authorize `RIGHT_DELETE` on `cap`, then destroy its `Queue` object
    /// (invalidating every outstanding capability on it) and close `cap`
    /// itself. Returns the object's `queue_ref` so the caller can also
    /// destroy the real `ipc` queue it names.
    pub fn destroy_queue_via_cap(
        &mut self,
        cap: GenerationalHandle,
        caller: ProcessId,
        now_chronos: u64,
    ) -> Result<u64, CapError> {
        self.authorize(cap, caller, RIGHT_DELETE, now_chronos)?;
        let object = self.get_cap(cap)?.object;
        let queue_ref = match self.get_object(object)?.kind {
            ObjectKind::Queue => self.get_object(object)?.queue_ref,
            _ => return Err(CapError::WrongObjectKind),
        };
        let obj = self
            .objects
            .get_mut(object.slot as usize)
            .expect("object validated by get_object above");
        obj.occupied = false;
        let retired = bump_generation(&mut obj.generation);
        if retired {
            self.stats.generation_retired += 1;
        } else {
            self.objects_free.push(object.slot);
        }
        self.stats.objects_destroyed += 1;
        self.close(cap)?;
        Ok(queue_ref)
    }

    /// Mint a root capability naming `inode_id`'s `File` object (created on
    /// first use, reused on subsequent opens of the same inode), owned by
    /// `subject`, with `rights` and optional `expiry` (Chronos ticks).
    pub fn open_file(
        &mut self,
        inode_id: InodeId,
        subject: ProcessId,
        rights: Rights,
        expiry: Option<u64>,
    ) -> GenerationalHandle {
        let object = self.find_or_create_file_object(inode_id);
        let handle = self.alloc_cap_slot(CapabilitySlot {
            occupied: true,
            generation: 0, // overwritten by alloc_cap_slot if reusing a slot
            object,
            subject,
            rights,
            expiry,
            parent: None,
        });
        self.stats.opens += 1;
        handle
    }

    /// Authorize `requested` on `cap`, then return the `InodeId` its `File`
    /// object is bound to.
    pub fn resolve_file(
        &mut self,
        cap: GenerationalHandle,
        caller: ProcessId,
        requested: Rights,
        now_chronos: u64,
    ) -> Result<InodeId, CapError> {
        self.authorize(cap, caller, requested, now_chronos)?;
        let object = self.get_cap(cap)?.object;
        let obj = self.get_object(object)?;
        match obj.kind {
            ObjectKind::File => Ok(obj.inode_id),
            _ => Err(CapError::WrongObjectKind),
        }
    }

    fn find_or_create_resource(
        &mut self,
        kind: ObjectKind,
        resource_id: u64,
    ) -> GenerationalHandle {
        if let Some(&slot) = self.object_by_resource.get(&(kind, resource_id)) {
            if let Some(obj) = self.objects.get(slot as usize) {
                if obj.occupied {
                    return GenerationalHandle {
                        slot,
                        generation: obj.generation,
                    };
                }
            }
        }
        let content_root = crate::kernel::crypto::phase_hash::ph_content(&resource_id.to_le_bytes());
        let handle = if let Some(slot) = self.objects_free.pop() {
            let obj = &mut self.objects[slot as usize];
            obj.occupied = true;
            obj.kind = kind;
            obj.inode_id = 0;
            obj.queue_ref = 0;
            obj.network_ref = 0;
            obj.resource_id = resource_id;
            obj.content_root = content_root;
            GenerationalHandle {
                slot,
                generation: obj.generation,
            }
        } else {
            let slot = self.objects.len() as u32;
            self.objects.push(ObjectSlot {
                occupied: true,
                generation: 0,
                kind,
                inode_id: 0,
                queue_ref: 0,
                network_ref: 0,
                resource_id,
                content_root,
            });
            GenerationalHandle {
                slot,
                generation: 0,
            }
        };
        self.object_by_resource
            .insert((kind, resource_id), handle.slot);
        self.stats.objects_created += 1;
        handle
    }

    fn mint_resource_cap(
        &mut self,
        kind: ObjectKind,
        resource_id: u64,
        subject: ProcessId,
        rights: Rights,
        expiry: Option<u64>,
    ) -> GenerationalHandle {
        let object = self.find_or_create_resource(kind, resource_id);
        let handle = self.alloc_cap_slot(CapabilitySlot {
            occupied: true,
            generation: 0,
            object,
            subject,
            rights,
            expiry,
            parent: None,
        });
        self.stats.opens += 1;
        handle
    }

    fn resolve_resource(
        &mut self,
        cap: GenerationalHandle,
        caller: ProcessId,
        requested: Rights,
        now_chronos: u64,
        kind: ObjectKind,
    ) -> Result<u64, CapError> {
        self.authorize(cap, caller, requested, now_chronos)?;
        let object = self.get_cap(cap)?.object;
        let obj = self.get_object(object)?;
        if obj.kind != kind {
            return Err(CapError::WrongObjectKind);
        }
        Ok(obj.resource_id)
    }

    pub fn open_process(
        &mut self,
        pid: ProcessId,
        subject: ProcessId,
        rights: Rights,
        expiry: Option<u64>,
    ) -> GenerationalHandle {
        self.mint_resource_cap(ObjectKind::Process, pid as u64, subject, rights, expiry)
    }

    pub fn resolve_process(
        &mut self,
        cap: GenerationalHandle,
        caller: ProcessId,
        requested: Rights,
        now_chronos: u64,
    ) -> Result<ProcessId, CapError> {
        self.resolve_resource(cap, caller, requested, now_chronos, ObjectKind::Process)
            .map(|id| id as ProcessId)
    }

    pub fn open_memory(
        &mut self,
        page: PageId,
        subject: ProcessId,
        rights: Rights,
        expiry: Option<u64>,
    ) -> GenerationalHandle {
        self.mint_resource_cap(ObjectKind::Memory, page as u64, subject, rights, expiry)
    }

    pub fn resolve_memory(
        &mut self,
        cap: GenerationalHandle,
        caller: ProcessId,
        requested: Rights,
        now_chronos: u64,
    ) -> Result<PageId, CapError> {
        self.resolve_resource(cap, caller, requested, now_chronos, ObjectKind::Memory)
            .map(|id| id as PageId)
    }

    pub fn open_device(
        &mut self,
        irq: u32,
        subject: ProcessId,
        rights: Rights,
        expiry: Option<u64>,
    ) -> GenerationalHandle {
        self.mint_resource_cap(ObjectKind::Device, irq as u64, subject, rights, expiry)
    }

    pub fn resolve_device(
        &mut self,
        cap: GenerationalHandle,
        caller: ProcessId,
        requested: Rights,
        now_chronos: u64,
    ) -> Result<u32, CapError> {
        self.resolve_resource(cap, caller, requested, now_chronos, ObjectKind::Device)
            .map(|id| id as u32)
    }

    fn alloc_cap_slot(&mut self, mut slot_data: CapabilitySlot) -> GenerationalHandle {
        if let Some(slot) = self.caps_free.pop() {
            let existing_generation = self.caps[slot as usize].generation;
            slot_data.generation = existing_generation;
            self.caps[slot as usize] = slot_data;
            GenerationalHandle {
                slot,
                generation: existing_generation,
            }
        } else {
            let slot = self.caps.len() as u32;
            let generation = slot_data.generation;
            self.caps.push(slot_data);
            GenerationalHandle { slot, generation }
        }
    }

    /// Destroy (revoke) exactly the capability slot named by `cap`. Does not
    /// touch the underlying object — other open capabilities on the same
    /// object remain valid.
    pub fn close(&mut self, cap: GenerationalHandle) -> Result<(), CapError> {
        let slot = self
            .caps
            .get_mut(cap.slot as usize)
            .ok_or(CapError::InvalidHandle)?;
        if !slot.occupied {
            return Err(CapError::InvalidHandle);
        }
        if slot.generation != cap.generation {
            return Err(CapError::StaleGeneration);
        }
        slot.occupied = false;
        let retired = bump_generation(&mut slot.generation);
        if retired {
            self.stats.generation_retired += 1;
        } else {
            self.caps_free.push(cap.slot);
        }
        self.stats.closes += 1;
        Ok(())
    }

    /// Revoke every occupied capability whose subject is `pid`.
    pub fn close_all_for_subject(&mut self, pid: ProcessId) -> u32 {
        let handles: crate::platform::Vec<GenerationalHandle> = self
            .caps
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.occupied && slot.subject == pid)
            .map(|(i, slot)| GenerationalHandle {
                slot: i as u32,
                generation: slot.generation,
            })
            .collect();
        let mut n = 0u32;
        for h in handles {
            if self.close(h).is_ok() {
                n += 1;
            }
        }
        n
    }

    /// Destroy the `File` object bound to `inode_id`, if one exists. Bumps
    /// its generation, which invalidates every outstanding capability
    /// referencing it (their `V(c.object)` check fails from then on).
    /// Returns `true` iff an object was found and destroyed.
    pub fn destroy_object_for_inode(&mut self, inode_id: InodeId) -> bool {
        let Some(slot) = self.object_by_inode.remove(&inode_id) else {
            return false;
        };
        let Some(obj) = self.objects.get_mut(slot as usize) else {
            return false;
        };
        if !obj.occupied {
            return false;
        }
        obj.occupied = false;
        let retired = bump_generation(&mut obj.generation);
        if retired {
            self.stats.generation_retired += 1;
        } else {
            self.objects_free.push(slot);
        }
        self.stats.objects_destroyed += 1;
        true
    }

    fn get_object(&self, handle: GenerationalHandle) -> Result<&ObjectSlot, CapError> {
        let obj = self
            .objects
            .get(handle.slot as usize)
            .ok_or(CapError::ObjectGone)?;
        if !obj.occupied {
            return Err(CapError::ObjectGone);
        }
        if obj.generation != handle.generation {
            return Err(CapError::ObjectGone);
        }
        Ok(obj)
    }

    fn get_cap(&self, handle: GenerationalHandle) -> Result<&CapabilitySlot, CapError> {
        let slot = self
            .caps
            .get(handle.slot as usize)
            .ok_or(CapError::InvalidHandle)?;
        if !slot.occupied {
            return Err(CapError::InvalidHandle);
        }
        if slot.generation != handle.generation {
            return Err(CapError::StaleGeneration);
        }
        Ok(slot)
    }

    fn delegation_chain_valid(
        &self,
        parent: Option<GenerationalHandle>,
        depth: u32,
    ) -> Result<(), CapError> {
        let Some(parent_handle) = parent else {
            return Ok(());
        };
        if depth >= MAX_DELEGATION_DEPTH {
            return Err(CapError::MaxDepthExceeded);
        }
        let parent_slot = self
            .get_cap(parent_handle)
            .map_err(|_| CapError::DelegationChainBroken)?;
        self.get_object(parent_slot.object)
            .map_err(|_| CapError::DelegationChainBroken)?;
        self.delegation_chain_valid(parent_slot.parent, depth + 1)
    }

    fn authorize_inner(
        &self,
        cap: GenerationalHandle,
        caller: ProcessId,
        requested: Rights,
        now_chronos: u64,
    ) -> Result<(), CapError> {
        let slot = self.get_cap(cap)?;
        if slot.subject != caller {
            return Err(CapError::WrongSubject);
        }
        if requested & !slot.rights != 0 {
            return Err(CapError::RightsEscalation);
        }
        if let Some(expiry) = slot.expiry {
            if now_chronos >= expiry {
                return Err(CapError::Expired);
            }
        }
        self.get_object(slot.object)
            .map_err(|_| CapError::ObjectGone)?;
        self.delegation_chain_valid(slot.parent, 0)?;
        Ok(())
    }

    /// The hard trust gate `Allow(c, op, caller, t)` (see module docs).
    /// `requested` is the rights bitset the caller's intended operation
    /// needs; `now_chronos` is the live `StateClock` Chronos tick.
    pub fn authorize(
        &mut self,
        cap: GenerationalHandle,
        caller: ProcessId,
        requested: Rights,
        now_chronos: u64,
    ) -> Result<(), CapError> {
        let result = self.authorize_inner(cap, caller, requested, now_chronos);
        match result {
            Ok(()) => self.stats.authorize_ok += 1,
            Err(CapError::InvalidHandle) | Err(CapError::StaleGeneration) => {
                self.stats.authorize_denied_stale += 1
            }
            Err(CapError::ObjectGone) => self.stats.authorize_denied_stale += 1,
            Err(CapError::RightsEscalation) => self.stats.authorize_denied_rights += 1,
            Err(CapError::WrongSubject) => self.stats.authorize_denied_subject += 1,
            Err(CapError::Expired) => self.stats.authorize_denied_expiry += 1,
            Err(CapError::DelegationChainBroken) | Err(CapError::MaxDepthExceeded) => {
                self.stats.authorize_denied_chain += 1
            }
            Err(CapError::DelegationRightsEscalation)
            | Err(CapError::DelegationExpiryEscalation)
            | Err(CapError::WrongObjectKind) => {}
        }
        result
    }

    /// Mint a child capability on the same object as `parent`, owned by
    /// `subject`. `granter` must be `parent`'s current subject and must hold
    /// `RIGHT_GRANT`. Enforces delegation monotonicity: `requested_rights`
    /// must be a subset of `parent`'s rights, and `expiry` must be no later
    /// than `parent`'s (an unbounded child under a bounded parent is
    /// rejected as an escalation).
    pub fn delegate(
        &mut self,
        parent: GenerationalHandle,
        granter: ProcessId,
        subject: ProcessId,
        requested_rights: Rights,
        expiry: Option<u64>,
        now_chronos: u64,
    ) -> Result<GenerationalHandle, CapError> {
        self.authorize(parent, granter, RIGHT_GRANT, now_chronos)?;
        let parent_slot = self.get_cap(parent)?;
        if requested_rights & !parent_slot.rights != 0 {
            return Err(CapError::DelegationRightsEscalation);
        }
        let child_expiry = match (parent_slot.expiry, expiry) {
            (None, child) => child,
            (Some(p), Some(c)) if c <= p => Some(c),
            (Some(_), _) => return Err(CapError::DelegationExpiryEscalation),
        };
        let object = parent_slot.object;
        let handle = self.alloc_cap_slot(CapabilitySlot {
            occupied: true,
            generation: 0,
            object,
            subject,
            rights: requested_rights,
            expiry: child_expiry,
            parent: Some(parent),
        });
        self.stats.delegations += 1;
        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ph1() -> ProcessId {
        1
    }
    fn ph2() -> ProcessId {
        2
    }

    #[test]
    fn open_close_round_trip_and_stale_handle_rejected() {
        let mut t = CapabilityTable::new();
        let cap = t.open_file(10, ph1(), RIGHT_READ | RIGHT_WRITE, None);
        assert!(t.authorize(cap, ph1(), RIGHT_READ, 0).is_ok());
        t.close(cap).expect("close");
        // The slot is now free (unoccupied, not yet reused), so a closed
        // handle is correctly reported as invalid rather than stale.
        assert_eq!(
            t.authorize(cap, ph1(), RIGHT_READ, 0),
            Err(CapError::InvalidHandle)
        );
        // Double close of the same handle must also fail, not silently
        // succeed.
        assert_eq!(t.close(cap), Err(CapError::InvalidHandle));
    }

    #[test]
    fn slot_reuse_does_not_revalidate_old_handle() {
        let mut t = CapabilityTable::new();
        let cap_a = t.open_file(10, ph1(), RIGHT_READ, None);
        t.close(cap_a).expect("close a");
        // New open on a different inode should reuse the freed slot index.
        let cap_b = t.open_file(20, ph2(), RIGHT_WRITE, None);
        assert_eq!(
            cap_a.slot, cap_b.slot,
            "expected the freed slot to be reused"
        );
        assert_ne!(cap_a.generation, cap_b.generation);
        // The old (stale) handle for the same slot index must not validate
        // against the new occupant's rights/subject.
        assert_eq!(
            t.authorize(cap_a, ph1(), RIGHT_READ, 0),
            Err(CapError::StaleGeneration)
        );
        assert!(t.authorize(cap_b, ph2(), RIGHT_WRITE, 0).is_ok());
    }

    #[test]
    fn rights_escalation_is_rejected() {
        let mut t = CapabilityTable::new();
        let cap = t.open_file(10, ph1(), RIGHT_READ, None);
        assert_eq!(
            t.authorize(cap, ph1(), RIGHT_WRITE, 0),
            Err(CapError::RightsEscalation)
        );
        assert!(t.authorize(cap, ph1(), RIGHT_READ, 0).is_ok());
    }

    #[test]
    fn wrong_subject_is_rejected() {
        let mut t = CapabilityTable::new();
        let cap = t.open_file(10, ph1(), RIGHT_READ, None);
        assert_eq!(
            t.authorize(cap, ph2(), RIGHT_READ, 0),
            Err(CapError::WrongSubject)
        );
    }

    #[test]
    fn expiry_is_enforced() {
        let mut t = CapabilityTable::new();
        let cap = t.open_file(10, ph1(), RIGHT_READ, Some(10));
        assert!(t.authorize(cap, ph1(), RIGHT_READ, 5).is_ok());
        assert_eq!(
            t.authorize(cap, ph1(), RIGHT_READ, 10),
            Err(CapError::Expired)
        );
        assert_eq!(
            t.authorize(cap, ph1(), RIGHT_READ, 11),
            Err(CapError::Expired)
        );
    }

    #[test]
    fn delegation_monotonicity_is_enforced() {
        let mut t = CapabilityTable::new();
        let root = t.open_file(10, ph1(), RIGHT_READ | RIGHT_GRANT, Some(100));
        // Requesting a right the parent doesn't have.
        assert_eq!(
            t.delegate(root, ph1(), ph2(), RIGHT_WRITE, None, 0),
            Err(CapError::DelegationRightsEscalation)
        );
        // Requesting unbounded expiry under a bounded parent.
        assert_eq!(
            t.delegate(root, ph1(), ph2(), RIGHT_READ, None, 0),
            Err(CapError::DelegationExpiryEscalation)
        );
        // Requesting a later expiry than the parent's.
        assert_eq!(
            t.delegate(root, ph1(), ph2(), RIGHT_READ, Some(200), 0),
            Err(CapError::DelegationExpiryEscalation)
        );
        // A valid, narrower delegation succeeds.
        let child = t
            .delegate(root, ph1(), ph2(), RIGHT_READ, Some(50), 0)
            .expect("valid delegation");
        assert!(t.authorize(child, ph2(), RIGHT_READ, 10).is_ok());
        // Delegating without RIGHT_GRANT on the parent must fail.
        let no_grant = t.open_file(11, ph1(), RIGHT_READ, None);
        assert!(matches!(
            t.delegate(no_grant, ph1(), ph2(), RIGHT_READ, None, 0),
            Err(_)
        ));
    }

    #[test]
    fn parent_revocation_invalidates_children() {
        let mut t = CapabilityTable::new();
        let root = t.open_file(10, ph1(), RIGHT_READ | RIGHT_GRANT, None);
        let child = t
            .delegate(root, ph1(), ph2(), RIGHT_READ, None, 0)
            .expect("delegate");
        assert!(t.authorize(child, ph2(), RIGHT_READ, 0).is_ok());
        t.close(root).expect("revoke root");
        assert_eq!(
            t.authorize(child, ph2(), RIGHT_READ, 0),
            Err(CapError::DelegationChainBroken)
        );
    }

    #[test]
    fn object_destruction_invalidates_all_open_capabilities() {
        let mut t = CapabilityTable::new();
        let cap_a = t.open_file(10, ph1(), RIGHT_READ, None);
        let cap_b = t.open_file(10, ph2(), RIGHT_WRITE, None);
        assert_ne!(cap_a.slot, cap_b.slot, "expected distinct capability slots");
        assert!(t.authorize(cap_a, ph1(), RIGHT_READ, 0).is_ok());
        assert!(t.authorize(cap_b, ph2(), RIGHT_WRITE, 0).is_ok());
        assert!(t.destroy_object_for_inode(10));
        assert_eq!(
            t.authorize(cap_a, ph1(), RIGHT_READ, 0),
            Err(CapError::ObjectGone)
        );
        assert_eq!(
            t.authorize(cap_b, ph2(), RIGHT_WRITE, 0),
            Err(CapError::ObjectGone)
        );
        // Destroying an inode with no live object is a real, observable no-op.
        assert!(!t.destroy_object_for_inode(999));
    }

    #[test]
    fn deep_delegation_chain_within_bound_succeeds_and_beyond_bound_fails() {
        let mut t = CapabilityTable::new();
        let mut current = t.open_file(10, ph1(), RIGHT_READ | RIGHT_GRANT, None);
        let mut granter = ph1();
        // Build a chain exactly MAX_DELEGATION_DEPTH long; every link keeps
        // RIGHT_GRANT so the same handle can also attempt one more hop below.
        for depth in 0..MAX_DELEGATION_DEPTH {
            let subject = 100 + depth;
            current = t
                .delegate(current, granter, subject, RIGHT_READ | RIGHT_GRANT, None, 0)
                .unwrap_or_else(|e| panic!("delegation at depth {depth} failed: {e:?}"));
            granter = subject;
        }
        // Exactly at the bound: still authorizes (walks MAX_DELEGATION_DEPTH
        // ancestors up to the root object's capability).
        assert!(t.authorize(current, granter, RIGHT_READ, 0).is_ok());
        // `delegate` itself only re-validates the immediate parent (whose
        // own chain is still within bound), so minting one hop further
        // succeeds...
        let too_deep = t
            .delegate(current, granter, 999, RIGHT_READ, None, 0)
            .expect("delegate succeeds: parent's own chain is still in bound");
        // ...but authorizing that resulting handle must fail once its full
        // chain is walked and found to exceed MAX_DELEGATION_DEPTH.
        assert_eq!(
            t.authorize(too_deep, 999, RIGHT_READ, 0),
            Err(CapError::MaxDepthExceeded)
        );
    }

    #[test]
    fn generation_overflow_never_wraps_retires_slot_instead() {
        let mut generation = u32::MAX;
        let retired = bump_generation(&mut generation);
        assert!(retired, "must report overflow instead of wrapping");
        assert_eq!(
            generation,
            u32::MAX,
            "must not silently wrap into a small, potentially-valid-looking generation"
        );
    }

    #[test]
    fn open_queue_mints_object_and_capability_and_resolve_queue_round_trips() {
        let mut t = CapabilityTable::new();
        let cap = t.open_queue(0xAABB_CCDD, ph1(), RIGHT_READ | RIGHT_WRITE, None);
        assert_eq!(t.resolve_queue(cap, ph1(), RIGHT_WRITE, 0), Ok(0xAABB_CCDD));
        // A `File` capability must be rejected by `resolve_queue`.
        let file_cap = t.open_file(10, ph1(), RIGHT_READ, None);
        assert_eq!(
            t.resolve_queue(file_cap, ph1(), RIGHT_READ, 0),
            Err(CapError::WrongObjectKind)
        );
    }

    #[test]
    fn share_queue_grants_a_second_process_its_own_capability() {
        let mut t = CapabilityTable::new();
        let owner_cap = t.open_queue(42, ph1(), RIGHT_READ | RIGHT_WRITE | RIGHT_GRANT, None);
        let shared_cap = t
            .share_queue(owner_cap, ph1(), ph2(), RIGHT_READ, None, 0)
            .expect("share");
        assert_ne!(owner_cap.slot, shared_cap.slot);
        assert_eq!(t.resolve_queue(shared_cap, ph2(), RIGHT_READ, 0), Ok(42));
        // The shared capability was only granted RIGHT_READ; RIGHT_WRITE
        // must be rejected as a real escalation, not silently allowed.
        assert_eq!(
            t.resolve_queue(shared_cap, ph2(), RIGHT_WRITE, 0),
            Err(CapError::RightsEscalation)
        );
    }

    #[test]
    fn destroy_queue_via_cap_invalidates_object_and_closes_the_capability() {
        let mut t = CapabilityTable::new();
        let owner_cap = t.open_queue(7, ph1(), RIGHT_READ | RIGHT_DELETE | RIGHT_GRANT, None);
        let other_cap = t
            .share_queue(owner_cap, ph1(), ph2(), RIGHT_READ, None, 0)
            .expect("share");
        assert_eq!(t.destroy_queue_via_cap(owner_cap, ph1(), 0), Ok(7));
        // The destroying capability itself is now closed (invalid, not
        // stale — its slot has not been reused yet).
        assert_eq!(
            t.authorize(owner_cap, ph1(), RIGHT_READ, 0),
            Err(CapError::InvalidHandle)
        );
        // A second, still-open capability on the now-destroyed object must
        // also be rejected — object destruction invalidates every
        // outstanding capability on it, exactly like `destroy_object_for_inode`.
        assert_eq!(
            t.authorize(other_cap, ph2(), RIGHT_READ, 0),
            Err(CapError::ObjectGone)
        );
    }

    #[test]
    fn destroy_queue_via_cap_rejects_wrong_kind_and_missing_rights() {
        let mut t = CapabilityTable::new();
        let file_cap = t.open_file(10, ph1(), RIGHT_READ | RIGHT_DELETE, None);
        assert_eq!(
            t.destroy_queue_via_cap(file_cap, ph1(), 0),
            Err(CapError::WrongObjectKind)
        );
        let queue_cap = t.open_queue(9, ph1(), RIGHT_READ, None);
        assert_eq!(
            t.destroy_queue_via_cap(queue_cap, ph1(), 0),
            Err(CapError::RightsEscalation),
            "no RIGHT_DELETE was granted"
        );
    }

    /// Required proof (for the `netsess` consumer, subsystem 8): exact
    /// mirror of `open_queue_mints_object_and_capability_and_resolve_queue_
    /// round_trips`, one `ObjectKind` over.
    #[test]
    fn open_network_mints_object_and_capability_and_resolve_network_round_trips() {
        let mut t = CapabilityTable::new();
        let cap = t.open_network(0x1122_3344_5566_7788, ph1(), RIGHT_READ | RIGHT_WRITE, None);
        assert_eq!(
            t.resolve_network(cap, ph1(), RIGHT_WRITE, 0),
            Ok(0x1122_3344_5566_7788)
        );
        // A `Queue` capability must be rejected by `resolve_network`, and a
        // `File` capability likewise.
        let queue_cap = t.open_queue(1, ph1(), RIGHT_READ, None);
        assert_eq!(
            t.resolve_network(queue_cap, ph1(), RIGHT_READ, 0),
            Err(CapError::WrongObjectKind)
        );
        let file_cap = t.open_file(10, ph1(), RIGHT_READ, None);
        assert_eq!(
            t.resolve_network(file_cap, ph1(), RIGHT_READ, 0),
            Err(CapError::WrongObjectKind)
        );
    }

    /// Required proof (for the `netsess` consumer, subsystem 8): exact
    /// mirror of `destroy_queue_via_cap_invalidates_object_and_closes_the_
    /// capability`, one `ObjectKind` over.
    #[test]
    fn destroy_network_via_cap_invalidates_object_and_closes_the_capability() {
        let mut t = CapabilityTable::new();
        let owner_cap = t.open_network(7, ph1(), RIGHT_READ | RIGHT_DELETE, None);
        assert_eq!(t.destroy_network_via_cap(owner_cap, ph1(), 0), Ok(7));
        // The destroying capability itself is now closed (invalid, not
        // stale — its slot has not been reused yet).
        assert_eq!(
            t.authorize(owner_cap, ph1(), RIGHT_READ, 0),
            Err(CapError::InvalidHandle)
        );
    }

    /// Required proof (for the `netsess` consumer, subsystem 8): exact
    /// mirror of `destroy_queue_via_cap_rejects_wrong_kind_and_missing_
    /// rights`, one `ObjectKind` over.
    #[test]
    fn destroy_network_via_cap_rejects_wrong_kind_and_missing_rights() {
        let mut t = CapabilityTable::new();
        let file_cap = t.open_file(10, ph1(), RIGHT_READ | RIGHT_DELETE, None);
        assert_eq!(
            t.destroy_network_via_cap(file_cap, ph1(), 0),
            Err(CapError::WrongObjectKind)
        );
        let queue_cap = t.open_queue(9, ph1(), RIGHT_READ | RIGHT_DELETE, None);
        assert_eq!(
            t.destroy_network_via_cap(queue_cap, ph1(), 0),
            Err(CapError::WrongObjectKind)
        );
        let network_cap = t.open_network(11, ph1(), RIGHT_READ, None);
        assert_eq!(
            t.destroy_network_via_cap(network_cap, ph1(), 0),
            Err(CapError::RightsEscalation),
            "no RIGHT_DELETE was granted"
        );
    }

    #[test]
    fn resolve_file_requires_matching_kind_and_rights() {
        let mut t = CapabilityTable::new();
        let cap = t.open_file(10, ph1(), RIGHT_READ, None);
        assert_eq!(t.resolve_file(cap, ph1(), RIGHT_READ, 0), Ok(10));
        assert_eq!(
            t.resolve_file(cap, ph1(), RIGHT_WRITE, 0),
            Err(CapError::RightsEscalation)
        );
        let queue = t.open_queue(1, ph1(), RIGHT_READ, None);
        assert_eq!(
            t.resolve_file(queue, ph1(), RIGHT_READ, 0),
            Err(CapError::WrongObjectKind)
        );
    }

    #[test]
    fn process_memory_device_kinds_authorize_and_reject_cross_kind() {
        let mut t = CapabilityTable::new();
        let proc_cap = t.open_process(7, ph1(), RIGHT_READ | RIGHT_WRITE, None);
        let mem_cap = t.open_memory(3, ph1(), RIGHT_READ, None);
        let dev_cap = t.open_device(54, ph1(), RIGHT_READ, None);
        assert_eq!(t.resolve_process(proc_cap, ph1(), RIGHT_WRITE, 0), Ok(7));
        assert_eq!(t.resolve_memory(mem_cap, ph1(), RIGHT_READ, 0), Ok(3));
        assert_eq!(t.resolve_device(dev_cap, ph1(), RIGHT_READ, 0), Ok(54));
        assert_eq!(
            t.resolve_process(mem_cap, ph1(), RIGHT_READ, 0),
            Err(CapError::WrongObjectKind)
        );
        assert_eq!(
            t.resolve_memory(dev_cap, ph1(), RIGHT_READ, 0),
            Err(CapError::WrongObjectKind)
        );
        assert_eq!(
            t.resolve_device(proc_cap, ph1(), RIGHT_READ, 0),
            Err(CapError::WrongObjectKind)
        );
        assert_eq!(
            t.resolve_file(proc_cap, ph1(), RIGHT_READ, 0),
            Err(CapError::WrongObjectKind)
        );
    }
}
