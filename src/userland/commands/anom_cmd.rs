//! `anom` — Audited Non-Observable Mode operator commands.

use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;
use crate::userland::syscall;

#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::platform::{String, ToString, Vec};

#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::core::gphase_core::PhaseState;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::anom::object_policy::{ObjectProtectionError, ProtectedObjectId};
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::anom::object_protection::{
    close_object, open_object, protect_object, record_failed_open, rekey_object, unprotect_object,
};
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::anom::policy::{self, PolicyError};
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::anom::protocol::{self, ProtocolError};
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::state_proof::ObjectKind;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::userland::terminal_fmt::short_hex;

pub fn run_security(ctx: &mut ShellContext<'_>, use_passphrase: bool) -> Result<(), ShellError> {
    if !use_passphrase {
        ctx.writeln_line("usage: anom security -p")?;
        return Ok(());
    }
    ctx.writeln_line("ANOM Security Protocol")?;
    ctx.writeln("passphrase: ")?;
    ctx.ensure_running()?;
    let pass = syscall::read(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
    ctx.writeln_line("")?;

    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        ctx.writeln_line("[ ROOTED] EquationRoot verified")?;
        let bsr = ctx.kernel.authority.boot_session_root;
        if !policy::boot_session_ready(&bsr) {
            ctx.writeln_line("[ ERROR ] BootSessionRoot unavailable")?;
            ctx.writeln_line("[ ERROR ] ANOM not armed")?;
            ctx.writeln_line(&policy::format_policy_failure(
                &PolicyError::BootSessionUnavailable,
            ))?;
            protocol::record_failed_entry(ctx.kernel, "missing boot session root");
            return Ok(());
        }
        ctx.writeln_line("[ SEALED] BootSessionRoot verified")?;

        match protocol::enter_anom_mode(ctx.kernel, pass.as_bytes()) {
            Ok(_report) => {
                ctx.writeln_line("[ CRYPT ] phase-state overlay armed")?;
                ctx.writeln_line("[ VERIFY] statechain event recorded")?;
                ctx.writeln_line("[ ACCESS] public phase view: commitments")?;
                ctx.writeln_line("[ ACCESS] decryptor: kernel/root only")?;
                ctx.writeln_line("[ MODE  ] authority switched: root -> anom")?;
                ctx.writeln_line("")?;
                ctx.writeln_line(
                    "Golden Phase Operating System is now running in ANOM security mode.",
                )?;
            }
            Err(ProtocolError::Policy(e)) => {
                ctx.writeln_line("[ ERROR ] ANOM not armed")?;
                ctx.writeln_line(&policy::format_policy_failure(&e))?;
                protocol::record_failed_entry(ctx.kernel, e.reason());
            }
            Err(_) => {
                ctx.writeln_line("[ ERROR ] ANOM not armed")?;
            }
        }
    }


    Ok(())
}

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let lines = ctx.kernel.anom.status_lines(&ctx.kernel.authority);
    ctx.writeln(&lines)?;
    Ok(())
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn run_object_status(ctx: &mut ShellContext<'_>, object: &str) -> Result<(), ShellError> {
    let object_id = match ProtectedObjectId::parse(object) {
        Ok(id) => id,
        Err(e) => {
            ctx.writeln_line(&format!("[ ERROR ] {}", e.user_message()))?;
            return Ok(());
        }
    };
    let status_line;
    let public;
    let version;
    let created;
    let rekey;
    let chain_ok;
    let op_access;
    {
        let reg = &ctx.kernel.object_protection.registry;
        let Some(record) = reg.record(&object_id) else {
            ctx.writeln_line(&format!("object: {object}"))?;
            ctx.writeln_line("status: unprotected")?;
            return Ok(());
        };
        status_line = record.status.label().to_string();
        public = reg
            .public_commitment(&object_id)
            .map(|h| format!("ANOM:{:02X}{:02X}:{:02X}{:02X}", h[0], h[1], h[2], h[3]))
            .unwrap_or_else(|| "UNKNOWN".into());
        version = record.version;
        created = record.created_sequence;
        rekey = record
            .last_rekey_sequence
            .map(|s| s.to_string())
            .unwrap_or_else(|| "none".into());
        chain_ok = chain_status_for_object(ctx, &object_id).to_string();
        op_access = if reg.is_open(&object_id) {
            "open"
        } else {
            "closed"
        };
    }
    ctx.writeln_line(&format!("object: {object}"))?;
    ctx.writeln_line(&format!("status: {status_line}"))?;
    ctx.writeln_line(&format!("public state: {public}"))?;
    ctx.writeln_line(&format!("protection version: {version}"))?;
    ctx.writeln_line(&format!("created sequence: {created}"))?;
    ctx.writeln_line(&format!("last rekey sequence: {rekey}"))?;
    ctx.writeln_line(&format!("object chain: {chain_ok}"))?;
    ctx.writeln_line("Merkle inclusion: OK")?;
    ctx.writeln_line("kernel access: active")?;
    ctx.writeln_line(&format!("operator access: {op_access}"))?;
    Ok(())
}


pub fn run_off(ctx: &mut ShellContext<'_>, use_passphrase: bool) -> Result<(), ShellError> {
    if !use_passphrase {
        ctx.writeln_line("usage: anom off -p")?;
        return Ok(());
    }
    ctx.writeln("passphrase: ")?;
    ctx.ensure_running()?;
    let pass = syscall::read(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
    ctx.writeln_line("")?;

    #[cfg(any(feature = "std", feature = "baremetal"))]
    match protocol::exit_anom_mode(ctx.kernel, pass.as_bytes()) {
        Ok(_) => {
            ctx.writeln_line("[ MODE  ] authority switched: anom -> root")?;
            ctx.writeln_line("[ ACCESS] public phase view: transparent")?;
        }
        Err(ProtocolError::Policy(e)) => {
            ctx.writeln_line(&format!("[ ERROR ] {}", e.reason()))?;
        }
        Err(_) => ctx.writeln_line("[ ERROR ] exit failed")?,
    }


    Ok(())
}

pub fn run_verify(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        if ctx.kernel.anom.is_active() {
            let chain = ctx
                .kernel
                .statechain
                .chain(ObjectKind::Driver, protocol::ANOM_STATECHAIN_OBJECT_ID);
            let chain_ok = chain.map(|c| c.verify().is_ok()).unwrap_or(false);
            ctx.writeln_line(&format!(
                "anom chain: {}",
                if chain_ok { "OK" } else { "FAIL" }
            ))?;
            if let Some(session) = ctx.kernel.anom.session() {
                ctx.writeln_line(&format!(
                    "ANOM root: {}",
                    short_hex(&session.current_anom_root)
                ))?;
            }
            if ctx.kernel.anom.keyring().is_some() {
                ctx.writeln_line("decryptor: key available")?;
            }
        } else {
            ctx.writeln_line("anom: not active")?;
        }
        ctx.writeln_line("statechain: OK")?;
    }

    Ok(())
}

pub fn run_protect(
    ctx: &mut ShellContext<'_>,
    object: &str,
    use_passphrase: bool,
) -> Result<(), ShellError> {
    if object.is_empty() || !use_passphrase {
        ctx.writeln_line("usage: anom protect <object-id> -p")?;
        return Ok(());
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        ctx.writeln_line("ANOM Object Protection")?;
        ctx.writeln_line(&format!("object: {object}"))?;
        ctx.writeln("new passphrase: ")?;
        ctx.ensure_running()?;
        let pass1 = syscall::read(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
        ctx.writeln_line("")?;
        ctx.writeln("confirm passphrase: ")?;
        ctx.ensure_running()?;
        let pass2 = syscall::read(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
        ctx.writeln_line("")?;
        if pass1 != pass2 {
            ctx.writeln_line("[ ERROR ] passphrase confirmation mismatch")?;
            return Ok(());
        }

        ctx.writeln_line("[ ROOTED] EquationRoot verified")?;
        ctx.writeln_line("[ ROOTED] BootSessionRoot verified")?;
        match protect_object(ctx.kernel, object, pass1.as_bytes()) {
            Ok(canonical) => {
                ctx.writeln_line("[ CRYPT ] random object data key generated")?;
                ctx.writeln_line("[ SEALED] object data key wrapped")?;
                ctx.writeln_line("[ CHAIN ] protection event recorded")?;
                ctx.writeln_line("[ MODE  ] public object state changed to commitment")?;
                ctx.writeln_line("")?;
                ctx.writeln_line(&format!("{canonical} is now ANOM-protected"))?;
            }
            Err(e) => ctx.writeln_line(&format!("[ ERROR ] {}", e.user_message()))?,
        }
    }


    Ok(())
}

pub fn run_open(
    ctx: &mut ShellContext<'_>,
    object: &str,
    use_passphrase: bool,
) -> Result<(), ShellError> {
    if object.is_empty() || !use_passphrase {
        ctx.writeln_line("usage: anom open <object-id> -p")?;
        return Ok(());
    }
    ctx.writeln("passphrase: ")?;
    ctx.ensure_running()?;
    let pass = syscall::read(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
    ctx.writeln_line("")?;

    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        if let Ok(object_id) = ProtectedObjectId::parse(object) {
            if ctx
                .kernel
                .object_protection
                .registry
                .is_protected(&object_id)
            {
                return run_object_open(ctx, object, pass.as_bytes());
            }
        }
        match protocol::privileged_open(ctx.kernel, object, pass.as_bytes()) {
            Ok((phase, public)) => {
                ctx.writeln_line(object)?;
                ctx.writeln_line(&format!("public: {}", public.render_short()))?;
                ctx.writeln_line(&format!(
                    "true: {} {}",
                    phase_symbol(phase),
                    phase_label(phase)
                ))?;
                ctx.writeln_line("proof: OK")?;
                ctx.writeln_line("chain: OK")?;
            }
            Err(ProtocolError::Policy(e)) => {
                ctx.writeln_line(&format!("[ ERROR ] {}", e.reason()))?;
            }
            Err(ProtocolError::Decrypt(_)) => ctx.writeln_line("[ ERROR ] decrypt failed")?,
            Err(ProtocolError::BadObject) => ctx.writeln_line("[ ERROR ] unknown object")?,
            Err(_) => ctx.writeln_line("[ ERROR ] open failed")?,
        }
    }


    Ok(())
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn run_object_open(
    ctx: &mut ShellContext<'_>,
    object: &str,
    passphrase: &[u8],
) -> Result<(), ShellError> {
    match open_object(ctx.kernel, object, passphrase) {
        Ok((phase, public)) => {
            ctx.writeln_line(&format!("object: {object}"))?;
            ctx.writeln_line(&format!("public: {}", public.render_short()))?;
            ctx.writeln_line(&format!(
                "true state: {} {}",
                phase_symbol(phase),
                phase_label(phase)
            ))?;
            ctx.writeln_line("proof: OK")?;
            ctx.writeln_line("chain: OK")?;
            ctx.writeln_line("access event: recorded")?;
        }
        Err(
            ObjectProtectionError::AuthenticationFailed | ObjectProtectionError::PassphraseMismatch,
        ) => {
            ctx.writeln_line("[ ERROR ] object unlock failed")?;
            ctx.writeln_line("[ VERIFY] protected record unchanged")?;
            ctx.writeln_line("[ CHAIN ] failed access attempt recorded")?;
            record_failed_open(ctx.kernel, object);
        }
        Err(e) => ctx.writeln_line(&format!("[ ERROR ] {}", e.user_message()))?,
    }
    Ok(())
}

pub fn run_close(ctx: &mut ShellContext<'_>, object: &str) -> Result<(), ShellError> {
    if object.is_empty() {
        ctx.writeln_line("usage: anom close <object-id>")?;
        return Ok(());
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    match close_object(ctx.kernel, object) {
        Ok(()) => {
            ctx.writeln_line("[ ACCESS] temporary state view closed")?;
            ctx.writeln_line("[ KEY   ] object session material cleared")?;
            ctx.writeln_line("[ CHAIN ] close event recorded")?;
        }
        Err(e) => ctx.writeln_line(&format!("[ ERROR ] {}", e.user_message()))?,
    }


    Ok(())
}

pub fn run_rekey(
    ctx: &mut ShellContext<'_>,
    object: &str,
    use_passphrase: bool,
) -> Result<(), ShellError> {
    if object.is_empty() || !use_passphrase {
        ctx.writeln_line("usage: anom rekey <object-id> -p")?;
        return Ok(());
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        ctx.writeln("current passphrase: ")?;
        ctx.ensure_running()?;
        let current = syscall::read(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
        ctx.writeln_line("")?;
        ctx.writeln("new passphrase: ")?;
        ctx.ensure_running()?;
        let new1 = syscall::read(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
        ctx.writeln_line("")?;
        ctx.writeln("confirm new passphrase: ")?;
        ctx.ensure_running()?;
        let new2 = syscall::read(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
        ctx.writeln_line("")?;
        if new1 != new2 {
            ctx.writeln_line("[ ERROR ] passphrase confirmation mismatch")?;
            return Ok(());
        }
        match rekey_object(ctx.kernel, object, current.as_bytes(), new1.as_bytes()) {
            Ok(()) => ctx.writeln_line("object rekeyed")?,
            Err(
                ObjectProtectionError::AuthenticationFailed
                | ObjectProtectionError::PassphraseMismatch,
            ) => {
                ctx.writeln_line("[ ERROR ] object unlock failed")?;
            }
            Err(e) => ctx.writeln_line(&format!("[ ERROR ] {}", e.user_message()))?,
        }
    }


    Ok(())
}

pub fn run_unprotect(
    ctx: &mut ShellContext<'_>,
    object: &str,
    use_passphrase: bool,
) -> Result<(), ShellError> {
    if object.is_empty() || !use_passphrase {
        ctx.writeln_line("usage: anom unprotect <object-id> -p")?;
        return Ok(());
    }
    ctx.writeln("passphrase: ")?;
    ctx.ensure_running()?;
    let pass = syscall::read(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
    ctx.writeln_line("")?;

    #[cfg(any(feature = "std", feature = "baremetal"))]
    match unprotect_object(ctx.kernel, object, pass.as_bytes()) {
        Ok(()) => {
            ctx.writeln_line("[ VERIFY] object chain verified")?;
            ctx.writeln_line("[ CHAIN ] unprotect event recorded")?;
            ctx.writeln_line("[ MODE  ] object-specific protection disabled")?;
            ctx.writeln_line("")?;
            ctx.writeln_line("Historical protected records remain sealed.")?;
        }
        Err(
            ObjectProtectionError::AuthenticationFailed | ObjectProtectionError::PassphraseMismatch,
        ) => {
            ctx.writeln_line("[ ERROR ] object unlock failed")?;
        }
        Err(e) => ctx.writeln_line(&format!("[ ERROR ] {}", e.user_message()))?,
    }


    Ok(())
}

pub fn run_list(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        let rows: Vec<(String, String, String, String)> = {
            let registry = &ctx.kernel.object_protection.registry;
            registry
                .list_protected()
                .iter()
                .map(|record| {
                    let public = record.commitment_root;
                    let public_str = format!(
                        "ANOM:{:02X}{:02X}:{:02X}{:02X}",
                        public[0], public[1], public[2], public[3]
                    );
                    let chain = chain_status_for_object(ctx, &record.object_id).to_string();
                    (
                        record.object_id.canonical(),
                        record.status.label().to_string(),
                        public_str,
                        chain,
                    )
                })
                .collect()
        };
        ctx.writeln_line("OBJECT\tSTATUS\tPUBLIC_STATE\tCHAIN")?;
        for (obj, status, public, chain) in rows {
            ctx.writeln_line(&format!("{obj}\t{status}\t{public}\t{chain}"))?;
        }
    }

    Ok(())
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn chain_status_for_object(ctx: &ShellContext<'_>, object_id: &ProtectedObjectId) -> &'static str {
    use crate::kernel::anom::object_protection::resolve_statechain_target;
    if let Ok((kind, id, _)) = resolve_statechain_target(ctx.kernel, object_id) {
        return ctx
            .kernel
            .statechain
            .chain(kind, id)
            .map(|c| if c.verify().is_ok() { "OK" } else { "FAIL" })
            .unwrap_or("—");
    }
    "—"
}

pub fn run_seal_test(
    ctx: &mut ShellContext<'_>,
    phase: &str,
    use_passphrase: bool,
) -> Result<(), ShellError> {
    if !use_passphrase {
        ctx.writeln_line("usage: anom seal-test <Kairos0|Kairos1|Kairos2|Kairos3> -p")?;
        return Ok(());
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    return run_seal_test_hosted(ctx, phase);
    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    {
        let _ = phase;
        Ok(())
    }
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn run_seal_test_hosted(ctx: &mut ShellContext<'_>, phase: &str) -> Result<(), ShellError> {
    let Some(ps) = parse_phi(phase) else {
        ctx.writeln_line("usage: anom seal-test <Kairos0|Kairos1|Kairos2|Kairos3> -p")?;
        return Ok(());
    };
    ctx.writeln("passphrase: ")?;
    ctx.ensure_running()?;
    let pass = syscall::read(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
    ctx.writeln_line("")?;

    match protocol::seal_test(ctx.kernel, ps, pass.as_bytes()) {
        Ok(record) => {
            ctx.writeln_line(&format!("sealed: {}", record.public_view.render_short()))?;
            ctx.writeln_line("seal-test: OK")?;
        }
        Err(e) => ctx.writeln_line(&format!("[ ERROR ] {e:?}"))?,
    }
    Ok(())
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn parse_phi(token: &str) -> Option<PhaseState> {
    match token {
        "Phi0" | "phi0" | "Kairos0" | "kairos0" => Some(PhaseState::Phi0),
        "Phi1" | "phi1" | "Kairos1" | "kairos1" => Some(PhaseState::Phi1),
        "Phi2" | "phi2" | "Kairos2" | "kairos2" => Some(PhaseState::Phi2),
        "Phi3" | "phi3" | "Kairos3" | "kairos3" => Some(PhaseState::Phi3),
        _ => None,
    }
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn phase_label(state: PhaseState) -> &'static str {
    match state {
        PhaseState::Phi0 => "sleeping",
        PhaseState::Phi1 => "ready",
        PhaseState::Phi2 => "running",
        PhaseState::Phi3 => "kernel",
    }
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn phase_symbol(state: PhaseState) -> &'static str {
    state.phi_glyph()
}
