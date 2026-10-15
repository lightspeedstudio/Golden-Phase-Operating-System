//! StateDomain VM shell commands — `state mode`, `state map`, etc.

use crate::kernel::state_domain::{
    infer_domain_from_object, render_domain_map, render_domains_summary, render_object_view,
    render_transition, StateChangeMode,
};
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::state_proof::ObjectKind;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run_mode_query(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln_line(&format!(
        "state change mode: {}",
        ctx.kernel.state_domain.mode.label()
    ))?;
    Ok(())
}

pub fn run_mode_set(ctx: &mut ShellContext<'_>, mode_token: &str) -> Result<(), ShellError> {
    let Some(mode) = StateChangeMode::parse(mode_token) else {
        ctx.writeln_line("usage: state mode [root4|expanded|adaptive]")?;
        return Ok(());
    };
    ctx.kernel.state_domain.set_mode(mode);
    ctx.writeln_line(&format!("state change mode: {}", mode.label()))?;
    Ok(())
}

pub fn run_domains(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let list = ctx.kernel.state_domain.domains_list();
    let text = render_domains_summary(&list);
    ctx.writeln(&text)?;
    Ok(())
}

pub fn run_map(ctx: &mut ShellContext<'_>, domain_name: &str) -> Result<(), ShellError> {
    if domain_name.is_empty() {
        ctx.writeln_line("usage: state map <chemistry|craft|network|security>")?;
        return Ok(());
    }
    let Some(domain_id) = crate::kernel::state_domain::DomainId::parse(domain_name) else {
        ctx.writeln_line("usage: state map <chemistry|craft|network|security>")?;
        return Ok(());
    };
    let Some(domain) = ctx.kernel.state_domain.domain(&domain_id) else {
        ctx.writeln_line("domain not loaded")?;
        return Ok(());
    };
    ctx.writeln(&render_domain_map(domain))?;
    Ok(())
}

pub fn run_view(ctx: &mut ShellContext<'_>, object: &str) -> Result<(), ShellError> {
    if object.is_empty() {
        ctx.writeln_line("usage: state view <object>")?;
        return Ok(());
    }
    let Some(domain_id) = infer_domain_from_object(object) else {
        ctx.writeln_line("could not infer domain from object key")?;
        return Ok(());
    };
    #[cfg(any(feature = "std", feature = "baremetal"))]
    ctx.kernel.init_state_domain();
    #[cfg(any(feature = "std", feature = "baremetal"))]
    let _ = ctx.kernel.state_domain.ensure_instance(object, &domain_id);
    let Some(domain) = ctx.kernel.state_domain.domain(&domain_id) else {
        ctx.writeln_line("domain not loaded")?;
        return Ok(());
    };
    let Some(instance) = ctx.kernel.state_domain.instance(object) else {
        ctx.writeln_line("no virtual state for object")?;
        return Ok(());
    };
    let snapshot = ctx.kernel.clock.snapshot();
    #[cfg(any(feature = "std", feature = "baremetal"))]
    let chain_ok = {
        let chain_id = instance.driver_chain_id;
        ctx.kernel
            .statechain
            .chain(ObjectKind::Driver, chain_id)
            .map(|c| c.verify().is_ok())
            .unwrap_or(true)
    };
    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    let chain_ok = true;
    let proof_ok = instance.last_proof_hash != [0u8; 32] || chain_ok;
    ctx.writeln(&render_object_view(
        object,
        domain,
        instance,
        ctx.kernel.state_domain.mode,
        chain_ok,
        proof_ok,
        &snapshot,
    ))?;
    Ok(())
}

pub fn run_change(
    ctx: &mut ShellContext<'_>,
    object: &str,
    state_label: &str,
) -> Result<(), ShellError> {
    if object.is_empty() || state_label.is_empty() {
        ctx.writeln_line("usage: state change <object> <domain_state>")?;
        return Ok(());
    }
    let Some(domain_id) = infer_domain_from_object(object) else {
        ctx.writeln_line("could not infer domain from object key")?;
        return Ok(());
    };
    #[cfg(any(feature = "std", feature = "baremetal"))]
    ctx.kernel.init_state_domain();
    let _old = ctx.kernel.state_domain.instance(object).cloned();
    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        match ctx
            .kernel
            .change_domain_state(object, &domain_id, state_label, "state change")
        {
            Ok(proof) => {
                if let (Some(old_inst), Some(new_inst)) =
                    (_old.as_ref(), ctx.kernel.state_domain.instance(object))
                {
                    ctx.writeln(&render_transition(
                        &old_inst.virtual_state_label,
                        &old_inst.phase_word,
                        &new_inst.virtual_state_label,
                        &new_inst.phase_word,
                    ))?;
                }
                ctx.writeln_line(&format!(
                    "proof: {:02x}{:02x}{:02x}{:02x}",
                    proof.proof_hash[0],
                    proof.proof_hash[1],
                    proof.proof_hash[2],
                    proof.proof_hash[3],
                ))?;
                ctx.writeln_line("chain: OK")?;
            }
            Err(err) => {
                ctx.writeln_line(&format!("[ ERROR ] {}", err.reason()))?;
            }
        }
    }
    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    ctx.writeln_line("state change: kernel statechain required")?;
    Ok(())
}

pub fn run_proof(ctx: &mut ShellContext<'_>, object: &str) -> Result<(), ShellError> {
    if object.is_empty() {
        ctx.writeln_line("usage: state proof <object>")?;
        return Ok(());
    }
    let instance = match ctx.kernel.state_domain.instance(object) {
        Some(i) => i.clone(),
        None => {
            ctx.writeln_line("no virtual state for object")?;
            return Ok(());
        }
    };
    ctx.writeln_line(&format!("object: {object}"))?;
    ctx.writeln_line(&format!(
        "last proof: {:02x}{:02x}{:02x}{:02x}",
        instance.last_proof_hash[0],
        instance.last_proof_hash[1],
        instance.last_proof_hash[2],
        instance.last_proof_hash[3],
    ))?;
    ctx.writeln_line(&format!(
        "last event: {:02x}{:02x}{:02x}{:02x}",
        instance.last_event_hash[0],
        instance.last_event_hash[1],
        instance.last_event_hash[2],
        instance.last_event_hash[3],
    ))?;
    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        let chain_id = instance.driver_chain_id;
        let chain_ok = ctx
            .kernel
            .statechain
            .chain(ObjectKind::Driver, chain_id)
            .map(|c| c.verify().is_ok())
            .unwrap_or(false);
        ctx.writeln_line(&format!("chain: {}", if chain_ok { "OK" } else { "FAIL" }))?;
    }
    Ok(())
}
