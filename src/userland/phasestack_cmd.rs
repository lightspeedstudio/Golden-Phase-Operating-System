//! `app` / `plugin` / `mod app` / `mod cert developer-revoke`.

use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;
use crate::platform::String;
#[cfg(feature = "phasestack")]
use crate::platform::Vec;

#[cfg(feature = "phasestack")]
use crate::kernel::phasestack::capability::GrantSet;
#[cfg(feature = "phasestack")]
use crate::kernel::phasestack::install::{self, authority_allows_system_scope};
#[cfg(feature = "phasestack")]
use crate::kernel::phasestack::types::{
    hex_lower, parse_hex32, InstallScope, PhaseStackError, StagedPackage,
};

pub fn run_app(ctx: &mut ShellContext<'_>, args: &[String]) -> Result<(), ShellError> {
    #[cfg(feature = "phasestack")]
    {
        dispatch_app(ctx, args, InstallScope::User)
    }
    #[cfg(not(feature = "phasestack"))]
    {
        let _ = args;
        ctx.writeln_line("app unavailable (phasestack feature not enabled)")?;
        Ok(())
    }
}

pub fn run_plugin(ctx: &mut ShellContext<'_>, args: &[String]) -> Result<(), ShellError> {
    #[cfg(feature = "phasestack")]
    {
        dispatch_plugin(ctx, args)
    }
    #[cfg(not(feature = "phasestack"))]
    {
        let _ = args;
        ctx.writeln_line("plugin unavailable (phasestack feature not enabled)")?;
        Ok(())
    }
}

pub fn run_mod_app(ctx: &mut ShellContext<'_>, args: &[String]) -> Result<(), ShellError> {
    #[cfg(feature = "phasestack")]
    {
        if let Err(e) = authority_allows_system_scope(ctx.kernel) {
            ctx.writeln_line(&format!("mod app denied: {e:?}"))?;
            return Ok(());
        }
        dispatch_app(ctx, args, InstallScope::System)
    }
    #[cfg(not(feature = "phasestack"))]
    {
        let _ = args;
        ctx.writeln_line("mod app unavailable (phasestack feature not enabled)")?;
        Ok(())
    }
}

pub fn run_mod_cert_revoke(ctx: &mut ShellContext<'_>, hash_hex: &str) -> Result<(), ShellError> {
    #[cfg(feature = "phasestack")]
    {
        if let Err(e) = authority_allows_system_scope(ctx.kernel) {
            ctx.writeln_line(&format!("mod cert developer-revoke denied: {e:?}"))?;
            return Ok(());
        }
        match parse_hex32(hash_hex) {
            Ok(h) => {
                let previous = ctx.kernel.phasestack.revocation.clone();
                ctx.kernel.phasestack.revocation.revoke_certificate(h);
                match crate::kernel::phasestack::install::persist_revocation(ctx.kernel) {
                    Ok(()) => ctx.writeln_line(&format!("revoked developer cert {}", hex_lower(&h)))?,
                    Err(e) => {
                        ctx.kernel.phasestack.revocation = previous;
                        ctx.writeln_line(&format!("revoke persist failed: {e:?}"))?;
                    }
                }
            }
            Err(e) => ctx.writeln_line(&format!("revoke failed: {e:?}"))?,
        }
        Ok(())
    }
    #[cfg(not(feature = "phasestack"))]
    {
        let _ = hash_hex;
        ctx.writeln_line("mod cert developer-revoke unavailable (phasestack feature not enabled)")?;
        Ok(())
    }
}

#[cfg(feature = "phasestack")]
fn parse_grants(args: &[String], start: usize) -> GrantSet {
    let mut grants = GrantSet::empty();
    let mut i = start;
    while i < args.len() {
        match args[i].as_str() {
            "--grant-caps" => {
                if let Some(v) = args.get(i + 1).and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok()) {
                    grants.caps = v;
                    i += 1;
                }
            }
            "--grant-hooks" => {
                if let Some(v) = args.get(i + 1).and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok()) {
                    grants.hooks = v;
                    i += 1;
                }
            }
            "--allow-privileged" => grants.allow_privileged_priority = true,
            _ => {}
        }
        i += 1;
    }
    grants
}

#[cfg(feature = "phasestack")]
fn resolve_app_id(
    kernel: &crate::kernel::Kernel,
    token: &str,
) -> Result<crate::kernel::phasestack::types::AppId, PhaseStackError> {
    if let Ok(id) = parse_hex32(token) {
        return Ok(id);
    }
    for (id, inst) in kernel.phasestack.registry.iter() {
        if inst.package().name() == token {
            return Ok(*id);
        }
    }
    Err(PhaseStackError::NotFound)
}

#[cfg(feature = "phasestack")]
fn dispatch_app(
    ctx: &mut ShellContext<'_>,
    args: &[String],
    scope: InstallScope,
) -> Result<(), ShellError> {
    let verb = args.first().map(|s| s.as_str()).unwrap_or("");
    match verb {
        "verify" => {
            let Some(path) = args.get(1) else {
                ctx.writeln_line("usage: app verify <gpa-path>")?;
                return Ok(());
            };
            match load_staged(ctx, path) {
                Ok(staged) => match crate::kernel::phasestack::verifier::verify_package(
                    crate::kernel::phasestack::verifier::VerifyInputs {
                        staged: &staged,
                        certs: &ctx.kernel.phasestack.certs,
                        revocation: &ctx.kernel.phasestack.revocation,
                        now_generation: 0,
                        grants: parse_grants(args, 2),
                        scope,
                        installed_version: None,
                    },
                ) {
                    Ok(pkg) => ctx.writeln_line(&format!(
                        "verified app={} root={}",
                        pkg.name(),
                        hex_lower(&pkg.package_root())
                    ))?,
                    Err(e) => ctx.writeln_line(&format!("verify failed: {e:?}"))?,
                },
                Err(e) => ctx.writeln_line(&format!("verify failed: {e:?}"))?,
            }
        }
        "install" => {
            let Some(path) = args.get(1) else {
                ctx.writeln_line("usage: app install <gpa-path> [--grant-caps <hex>]")?;
                return Ok(());
            };
            match load_staged(ctx, path) {
                Ok(staged) => match install::install(ctx.kernel, &staged, parse_grants(args, 2), scope)
                {
                    Ok(inst) => {
                        ctx.writeln_line(&format!(
                            "installed app={} root={}",
                            inst.package().name(),
                            hex_lower(&inst.package().package_root())
                        ))?;
                        ctx.writeln_line(&format!("user_map: bound scope={:?}", inst.scope()))?;
                    }
                    Err(e) => ctx.writeln_line(&format!("install failed: {e:?}"))?,
                },
                Err(e) => ctx.writeln_line(&format!("install failed: {e:?}"))?,
            }
        }
        "list" | "registry" => {
            ctx.writeln_line(&format!(
                "AppRegistryRoot: {} generation={}",
                hex_lower(&ctx.kernel.phasestack.registry_root()),
                ctx.kernel.phasestack.registry.generation()
            ))?;
            let rows: Vec<String> = ctx
                .kernel
                .phasestack
                .registry
                .iter()
                .map(|(id, inst)| {
                    format!(
                        "{} {} {:?} gen={} seq={}",
                        hex_lower(id),
                        inst.package().name(),
                        inst.state(),
                        inst.registry_generation(),
                        inst.install_sequence()
                    )
                })
                .collect();
            for row in rows {
                ctx.writeln_line(&row)?;
            }
        }
        "info" | "status" => {
            let Some(tok) = args.get(1) else {
                ctx.writeln_line("usage: app status <app-id-hex|name>")?;
                return Ok(());
            };
            match resolve_app_id(ctx.kernel, tok) {
                Ok(id) => match ctx.kernel.phasestack.registry.get(id) {
                    Some(inst) => ctx.writeln_line(&format!(
                        "name={} state={:?} pid={:?} root={} publisher={} manifest={} payload={} gen={} seq={} granted={:#x} scope={:?}",
                        inst.package().name(),
                        inst.state(),
                        inst.process_id(),
                        hex_lower(&inst.package().package_root()),
                        hex_lower(&inst.package().publisher_key_id()),
                        hex_lower(&inst.package().manifest_hash()),
                        hex_lower(&inst.package().code_hash()),
                        inst.registry_generation(),
                        inst.install_sequence(),
                        inst.granted_caps(),
                        inst.scope()
                    ))?,
                    None => ctx.writeln_line("not found")?,
                },
                Err(e) => ctx.writeln_line(&format!("{e:?}"))?,
            }
        }
        "run" => {
            let Some(tok) = args.get(1) else {
                ctx.writeln_line("usage: app run <app-id-hex|name>")?;
                return Ok(());
            };
            match resolve_app_id(ctx.kernel, tok) {
                Ok(id) => match install::launch(ctx.kernel, id, 10_000) {
                    Ok(result) => {
                        ctx.writeln_line("SENTINEL_APP_OUTPUT:")?;
                        let name = ctx
                            .kernel
                            .phasestack
                            .registry
                            .get(id)
                            .map(|i| i.package().name().into())
                            .unwrap_or_else(String::new);
                        ctx.writeln_line(&format!("package={name}"))?;
                        ctx.writeln_line(&format!("pid={}", result.pid))?;
                        ctx.writeln_line(&format!(
                            "launch_nonce={}",
                            hex_lower(&result.launch_nonce)
                        ))?;
                        ctx.writeln_line(&format!(
                            "vm_output={}",
                            hex_lower(&result.byte_output)
                        ))?;
                        #[cfg(all(feature = "phasesentinel"))]
                        {
                            use crate::kernel::crypto::phase_hash::ph_sentinel_launch_receipt;
                            let output_root = ph_sentinel_launch_receipt(&[
                                name.as_bytes(),
                                &result.pid.to_le_bytes(),
                                &result.launch_nonce,
                                &result.byte_output,
                            ]);
                            ctx.writeln_line(&format!("output_root={}", hex_lower(&output_root)))?;
                            ctx.writeln_line("sentinel=Allow")?;
                        }
                        ctx.writeln_line(&format!(
                            "running pid={} retired={} payload={} gen={}",
                            result.pid,
                            result.retired_instructions,
                            hex_lower(&result.payload_root),
                            result.registry_generation
                        ))?;
                    }
                    Err(e) => ctx.writeln_line(&format!("run failed: {e:?}"))?,
                },
                Err(e) => ctx.writeln_line(&format!("{e:?}"))?,
            }
        }
        "stop" => {
            let Some(tok) = args.get(1) else {
                ctx.writeln_line("usage: app stop <app-id-hex|name>")?;
                return Ok(());
            };
            match resolve_app_id(ctx.kernel, tok) {
                Ok(id) => match install::stop(ctx.kernel, id) {
                    Ok(()) => ctx.writeln_line("stopped")?,
                    Err(e) => ctx.writeln_line(&format!("stop failed: {e:?}"))?,
                },
                Err(e) => ctx.writeln_line(&format!("{e:?}"))?,
            }
        }
        "revoke" => {
            let Some(tok) = args.get(1) else {
                ctx.writeln_line("usage: app revoke <app-id-hex|name>")?;
                return Ok(());
            };
            match resolve_app_id(ctx.kernel, tok) {
                Ok(id) => match install::revoke(ctx.kernel, id) {
                    Ok(()) => {
                        ctx.writeln_line("revoked")?;
                        ctx.writeln_line("user_map: removed granted=0x0")?;
                    }
                    Err(e) => ctx.writeln_line(&format!("revoke failed: {e:?}"))?,
                },
                Err(e) => ctx.writeln_line(&format!("{e:?}"))?,
            }
        }
        "reconstruct" => {
            let report = install::rebuild_from_store(ctx.kernel);
            ctx.writeln_line(&format!(
                "reconstructed recovered={} skipped={}",
                report.recovered, report.skipped
            ))?;
            ctx.writeln_line(&format!(
                "AppRegistryRoot: {} generation={}",
                hex_lower(&ctx.kernel.phasestack.registry_root()),
                ctx.kernel.phasestack.registry.generation()
            ))?;
        }
        _ => ctx.writeln_line("usage: app verify|install|registry|status|run|stop|revoke|reconstruct")?,
    }
    Ok(())
}

#[cfg(feature = "phasestack")]
fn dispatch_plugin(ctx: &mut ShellContext<'_>, args: &[String]) -> Result<(), ShellError> {
    match args.first().map(|s| s.as_str()).unwrap_or("") {
        "install" | "verify" | "list" | "run" | "stop" => dispatch_app(ctx, args, InstallScope::User),
        _ => {
            ctx.writeln_line("usage: plugin verify|install|list|run|stop")?;
            Ok(())
        }
    }
}

#[cfg(feature = "phasestack")]
fn load_staged(ctx: &mut ShellContext<'_>, path: &str) -> Result<StagedPackage, PhaseStackError> {
    match ctx.kernel.fs.read_file(path) {
        Ok(bytes) => return Ok(StagedPackage { bytes }),
        Err(_) => {}
    }
    #[cfg(all(target_arch = "arm", feature = "baremetal"))]
    {
        if let Ok(bytes) = crate::userland::fs_sd::read_bytes(path) {
            return Ok(StagedPackage { bytes });
        }
    }
    Err(PhaseStackError::NotFound)
}
