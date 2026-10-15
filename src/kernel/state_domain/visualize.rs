//! StateDomain visualization — maps, views, and transition glyphs.

use crate::core::gphase_core::PhaseState;
use crate::kernel::state_domain::domain::StateDomain;
use crate::kernel::state_domain::encoding::{
    render_phase_word, render_phase_word_ascii, PhaseWord,
};
use crate::kernel::state_domain::state_change::{StateChangeMode, VirtualStateInstance};
use crate::platform::{String, Vec};

pub fn render_root_alphabet() -> &'static str {
    "Φ0 ─ Φ1 ─ Φ2 ─ Φ3"
}

pub fn render_domains_summary(domains: &[StateDomain]) -> String {
    let mut out = String::new();
    for d in domains {
        out.push_str(&format!(
            "{} states={} word_len={} capacity={}\n",
            d.name, d.state_count, d.word_length, d.capacity
        ));
    }
    out
}

pub fn render_domain_map(domain: &StateDomain) -> String {
    let mut out = String::new();
    out.push_str(&format!("KairOS StateDomain: {}\n\n", domain.name));
    out.push_str("root alphabet:\n  Φ0 Φ1 Φ2 Φ3\n\n");
    out.push_str(&format!(
        "domain states: {}\nword length: {}\ncapacity: {}\n\n",
        domain.state_count, domain.word_length, domain.capacity
    ));
    out.push_str("STATE          WORD      ROOT VIEW\n");
    for state in &domain.states {
        let word = render_phase_word(&state.phase_word);
        let root = state.root_hint.map(root_view_label).unwrap_or("—");
        out.push_str(&format!("{:<14} {:<9} {}\n", state.label, word, root));
    }
    out
}

pub fn render_domain_map_table(domain: &StateDomain) -> String {
    let mut out = String::new();
    out.push_str("STATE WORD DESCRIPTION\n");
    for state in &domain.states {
        out.push_str(&format!(
            "{} {} {}\n",
            state.label,
            render_phase_word(&state.phase_word),
            state.description
        ));
    }
    out
}

pub fn render_object_view(
    object_key: &str,
    domain: &StateDomain,
    instance: &VirtualStateInstance,
    mode: StateChangeMode,
    chain_ok: bool,
    proof_ok: bool,
    snapshot: &crate::stateclock::StateClockSnapshot,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("object: {object_key}\n"));
    out.push_str(&format!("domain: {}\n", domain.name));
    out.push_str(&format!("mode: {}\n", mode_name(mode)));
    out.push_str(&format!(
        "virtual state: {}\n",
        instance.virtual_state_label
    ));
    out.push_str(&format!(
        "phase word: {}\n",
        render_phase_word(&instance.phase_word)
    ));
    out.push_str(&format!(
        "root phase: {}\n",
        phase_short(instance.current_root_phase)
    ));
    out.push_str(&format!(
        "stateclock: sector {}  x={:.3}\n",
        snapshot.sector_id, snapshot.x
    ));
    out.push_str(&format!(
        "chain: {}\n",
        if chain_ok { "OK" } else { "FAIL" }
    ));
    out.push_str(&format!(
        "proof: {}\n",
        if proof_ok { "OK" } else { "FAIL" }
    ));
    if mode != StateChangeMode::Root4 {
        out.push_str(&render_expanded_trail(
            &instance.virtual_state_label,
            &instance.phase_word,
        ));
    }
    out
}

pub fn render_expanded_trail(label: &str, word: &[PhaseState]) -> String {
    let mut out = String::new();
    out.push_str(&format!("\n{label}\n"));
    if word.len() >= 2 {
        out.push_str(&format!(
            "word: {} -> {}\n",
            phase_short(word[0]),
            phase_short(word[1])
        ));
    }
    let trail: String = word
        .iter()
        .map(|p| format!("[{}]", phase_short(*p)))
        .collect::<Vec<_>>()
        .join("");
    out.push_str(&format!("trail: {trail}\n"));
    if let Some(last) = word.last() {
        out.push_str(&format!("root hint: {}\n", phase_short(*last)));
    }
    out
}

pub fn render_transition(
    old_label: &str,
    old_word: &PhaseWord,
    new_label: &str,
    new_word: &PhaseWord,
) -> String {
    format!(
        "{old_label} -> {new_label}\n\n\
         old: {old_label} {}\n\
         new: {new_label} {}\n\
         change: {}\n",
        render_phase_word_ascii(old_word),
        render_phase_word_ascii(new_word),
        transition_glyph(old_word, new_word),
    )
}

fn transition_glyph(old_word: &PhaseWord, new_word: &PhaseWord) -> String {
    if old_word == new_word {
        return String::from("= =");
    }
    let mut parts: Vec<String> = Vec::new();
    let n = old_word.len().max(new_word.len());
    for i in 0..n {
        let o = old_word.get(i).copied();
        let nw = new_word.get(i).copied();
        match (o, nw) {
            (Some(a), Some(b)) if a == b => parts.push("--".into()),
            (Some(_), Some(_)) => parts.push("^".into()),
            (None, Some(_)) => parts.push("+".into()),
            (Some(_), None) => parts.push("-".into()),
            _ => parts.push("?".into()),
        }
    }
    parts.join(" ")
}

fn mode_name(mode: StateChangeMode) -> &'static str {
    match mode {
        StateChangeMode::Root4 => "Root4",
        StateChangeMode::DomainExpanded => "DomainExpanded",
        StateChangeMode::Adaptive => "Adaptive",
    }
}

fn phase_short(p: PhaseState) -> &'static str {
    match p {
        PhaseState::Phi0 => "Φ0",
        PhaseState::Phi1 => "Φ1",
        PhaseState::Phi2 => "Φ2",
        PhaseState::Phi3 => "Φ3",
    }
}

fn root_view_label(p: PhaseState) -> &'static str {
    match p {
        PhaseState::Phi0 => "stable",
        PhaseState::Phi1 => "ready",
        PhaseState::Phi2 => "active",
        PhaseState::Phi3 => "locked",
    }
}
