//! DomainRoot computation.

use crate::core::gphase_core::PhaseState;
use crate::kernel::authority::StateHash;
use crate::kernel::state_domain::encoding::{
    encode_state_index, word_length_for_state_count, PhaseWord,
};
use crate::platform::{String, Vec};

#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::hash::tagged_hash;

pub const DOMAIN_ROOT_TAG: &str = "KAIROS_STATE_DOMAIN_V1";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DomainId {
    Chemistry,
    Craft,
    Network,
    Security,
    /// USB host device-lifecycle domain (see [`usb_labels`]).
    Usb,
    #[cfg(feature = "std")]
    Custom(String),
}

impl DomainId {
    pub fn name(&self) -> &str {
        match self {
            DomainId::Chemistry => "chemistry",
            DomainId::Craft => "craft",
            DomainId::Network => "network",
            DomainId::Security => "security",
            DomainId::Usb => "usb",
            #[cfg(feature = "std")]
            DomainId::Custom(s) => s.as_str(),
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        match token.to_ascii_lowercase().as_str() {
            "chemistry" | "chem" => Some(DomainId::Chemistry),
            "craft" => Some(DomainId::Craft),
            "network" | "net" => Some(DomainId::Network),
            "security" | "sec" => Some(DomainId::Security),
            "usb" => Some(DomainId::Usb),
            #[cfg(feature = "std")]
            other if !other.is_empty() => Some(DomainId::Custom(other.into())),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainStateId(pub u32);

#[derive(Debug, Clone)]
pub struct DomainState {
    pub id: DomainStateId,
    pub label: String,
    pub description: String,
    pub phase_word: PhaseWord,
    pub root_hint: Option<PhaseState>,
}

#[derive(Debug, Clone)]
pub struct StateDomain {
    pub domain_id: DomainId,
    pub name: String,
    pub state_count: usize,
    pub word_length: usize,
    pub capacity: usize,
    pub states: Vec<DomainState>,
    pub domain_root: StateHash,
}

pub fn root_hint_from_word(word: &[PhaseState]) -> Option<PhaseState> {
    word.last().copied()
}

pub fn build_domain_mapping(
    domain_id: DomainId,
    labels: &[(String, String)],
    equation_root: &StateHash,
) -> StateDomain {
    let state_count = labels.len();
    let word_length = word_length_for_state_count(state_count);
    let capacity = 4usize.saturating_pow(word_length as u32);
    let name: String = domain_id.name().into();
    let states: Vec<DomainState> = labels
        .iter()
        .enumerate()
        .map(|(i, (label, desc))| {
            let phase_word = encode_state_index(i, word_length);
            DomainState {
                id: DomainStateId(i as u32),
                label: label.clone(),
                description: desc.clone(),
                phase_word: phase_word.clone(),
                root_hint: root_hint_from_word(&phase_word),
            }
        })
        .collect();
    let domain_root = compute_domain_root(equation_root, &domain_id, &states, word_length);
    StateDomain {
        domain_id,
        name,
        state_count,
        word_length,
        capacity,
        states,
        domain_root,
    }
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn compute_domain_root(
    equation_root: &StateHash,
    domain_id: &DomainId,
    states: &[DomainState],
    word_length: usize,
) -> StateHash {
    let mut buf = Vec::new();
    buf.extend_from_slice(equation_root);
    buf.extend_from_slice(domain_id.name().as_bytes());
    buf.extend_from_slice(&(states.len() as u32).to_le_bytes());
    buf.extend_from_slice(&(word_length as u32).to_le_bytes());
    for state in states {
        buf.extend_from_slice(state.label.as_bytes());
        for phase in &state.phase_word {
            buf.push(phase.as_u8());
        }
    }
    tagged_hash(DOMAIN_ROOT_TAG, &[&buf])
}

#[cfg(not(any(feature = "std", feature = "baremetal")))]
pub fn compute_domain_root(
    _equation_root: &StateHash,
    _domain_id: &DomainId,
    _states: &[DomainState],
    _word_length: usize,
) -> StateHash {
    [0u8; 32]
}

pub fn chemistry_labels() -> Vec<(String, String)> {
    vec![
        ("neutral".into(), "stable neutral".into()),
        ("positive_ion".into(), "charged positive".into()),
        ("negative_ion".into(), "charged negative".into()),
        ("radical".into(), "unpaired/reactive".into()),
        ("bonded".into(), "bound".into()),
        ("unbonded".into(), "free".into()),
        ("excited".into(), "high-energy".into()),
        ("ground".into(), "low-energy".into()),
        ("unstable".into(), "warning".into()),
        ("decayed".into(), "terminal".into()),
    ]
}

pub fn craft_labels() -> Vec<(String, String)> {
    vec![
        ("idle".into(), "craft idle".into()),
        ("armed".into(), "craft armed".into()),
        ("scan".into(), "craft scanning".into()),
        ("lock".into(), "craft locked".into()),
        ("heat".into(), "craft heating".into()),
        ("cool".into(), "craft cooling".into()),
        ("bond".into(), "craft bonding".into()),
        ("cut".into(), "craft cutting".into()),
        ("seal".into(), "craft sealing".into()),
        ("vent".into(), "craft venting".into()),
        ("fault".into(), "craft fault".into()),
        ("recover".into(), "craft recovering".into()),
        ("calibrate".into(), "craft calibrating".into()),
        ("stow".into(), "craft stowed".into()),
        ("deploy".into(), "craft deployed".into()),
        ("halt".into(), "craft halted".into()),
    ]
}

pub fn security_labels() -> Vec<(String, String)> {
    vec![
        ("open".into(), "security open".into()),
        ("watch".into(), "security watching".into()),
        ("alert".into(), "security alert".into()),
        ("contain".into(), "security contained".into()),
        ("isolate".into(), "security isolated".into()),
        ("verify".into(), "security verifying".into()),
        ("seal".into(), "security sealed".into()),
        ("breach".into(), "security breach".into()),
    ]
}

pub fn network_labels() -> Vec<(String, String)> {
    (0..64)
        .map(|i| (format!("net_{i:02}"), format!("network node {i}")))
        .collect()
}

/// USB host device-lifecycle states. Each becomes a phase word over Kairos0-3
/// via [`build_domain_mapping`]; the lifecycle ripens Disconnected -> Polling as
/// the device is powered, detected, reset, addressed, configured and served.
pub fn usb_labels() -> Vec<(String, String)> {
    vec![
        ("disconnected".into(), "no device on the port".into()),
        ("powered".into(), "VBUS on, waiting for a device".into()),
        ("connected".into(), "device detected on the port".into()),
        ("reset".into(), "bus reset completed".into()),
        ("addressed".into(), "device address assigned".into()),
        ("configured".into(), "enumerated and configured".into()),
        ("hid_ready".into(), "HID boot protocol armed".into()),
        ("polling".into(), "reading input reports".into()),
        ("fault".into(), "device error / unsupported".into()),
        ("removed".into(), "device unplugged".into()),
    ]
}

pub fn build_usb_domain(equation_root: &StateHash) -> StateDomain {
    build_domain_mapping(DomainId::Usb, &usb_labels(), equation_root)
}

pub fn build_chemistry_domain(equation_root: &StateHash) -> StateDomain {
    build_domain_mapping(DomainId::Chemistry, &chemistry_labels(), equation_root)
}

pub fn build_craft_domain(equation_root: &StateHash) -> StateDomain {
    build_domain_mapping(DomainId::Craft, &craft_labels(), equation_root)
}

pub fn build_network_domain(equation_root: &StateHash) -> StateDomain {
    build_domain_mapping(DomainId::Network, &network_labels(), equation_root)
}

pub fn build_security_domain(equation_root: &StateHash) -> StateDomain {
    build_domain_mapping(DomainId::Security, &security_labels(), equation_root)
}

impl StateDomain {
    pub fn state_by_label(&self, label: &str) -> Option<&DomainState> {
        self.states
            .iter()
            .find(|s| s.label.eq_ignore_ascii_case(label))
    }

    pub fn state_by_id(&self, id: DomainStateId) -> Option<&DomainState> {
        self.states.iter().find(|s| s.id == id)
    }
}
