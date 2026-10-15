//! Authority and visibility modes — root transparent vs ANOM protected.

pub type StateHash = [u8; 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityMode {
    Root,
    Anom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityMode {
    Transparent,
    Sealed,
    AnomProtected,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicPhaseView {
    Visible(crate::core::gphase_core::PhaseState),
    AnomCommitment(StateHash),
    Locked,
    Unknown,
}

impl PublicPhaseView {
    pub fn render_short(&self) -> crate::platform::String {
        use crate::platform::String;
        match self {
            PublicPhaseView::Visible(state) => String::from(phase_label(*state)),
            PublicPhaseView::AnomCommitment(hash) => {
                format!(
                    "ANOM:{:02X}{:02X}:{:02X}{:02X}",
                    hash[0], hash[1], hash[2], hash[3]
                )
            }
            PublicPhaseView::Locked => String::from("LOCKED"),
            PublicPhaseView::Unknown => String::from("UNKNOWN"),
        }
    }
}

fn phase_label(state: crate::core::gphase_core::PhaseState) -> &'static str {
    match state {
        crate::core::gphase_core::PhaseState::Phi0 => "Phi0",
        crate::core::gphase_core::PhaseState::Phi1 => "Phi1",
        crate::core::gphase_core::PhaseState::Phi2 => "Phi2",
        crate::core::gphase_core::PhaseState::Phi3 => "Phi3",
    }
}

#[derive(Debug, Clone)]
pub struct RootFileHistory {
    pub last_authority_mode: AuthorityMode,
    pub last_visibility_mode: VisibilityMode,
    pub anom_available: bool,
    pub last_anom_session_root: StateHash,
    pub last_anom_enter_event: StateHash,
    pub last_anom_exit_event: StateHash,
}

impl Default for RootFileHistory {
    fn default() -> Self {
        Self {
            last_authority_mode: AuthorityMode::Root,
            last_visibility_mode: VisibilityMode::Transparent,
            anom_available: false,
            last_anom_session_root: [0u8; 32],
            last_anom_enter_event: [0u8; 32],
            last_anom_exit_event: [0u8; 32],
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthorityState {
    pub authority: AuthorityMode,
    pub visibility: VisibilityMode,
    pub boot_session_root: StateHash,
    pub history: RootFileHistory,
}

impl Default for AuthorityState {
    fn default() -> Self {
        Self {
            authority: AuthorityMode::Root,
            visibility: VisibilityMode::Transparent,
            boot_session_root: [0u8; 32],
            history: RootFileHistory::default(),
        }
    }
}

impl AuthorityState {
    pub fn is_anom_protected(&self) -> bool {
        self.authority == AuthorityMode::Anom && self.visibility == VisibilityMode::AnomProtected
    }

    pub fn public_phase(
        &self,
        true_phase: crate::core::gphase_core::PhaseState,
    ) -> PublicPhaseView {
        if self.is_anom_protected() {
            PublicPhaseView::Unknown
        } else {
            PublicPhaseView::Visible(true_phase)
        }
    }
}
