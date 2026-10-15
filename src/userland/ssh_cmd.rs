//! SSH management shell commands.

use crate::hosted::ssh::config::SshServerConfig;
use crate::hosted::ssh::{auth, host_key};
use crate::kernel::remote_shell::error::SshError;
use crate::kernel::remote_shell::types::RemoteAuthority;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

fn ssh_err(err: SshError) -> String {
    err.reason()
}

#[cfg(feature = "ssh")]
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime")
        .block_on(future)
}

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let ssh = &ctx.kernel.ssh;
    #[cfg(feature = "ssh")]
    {
        let running = crate::hosted::ssh::is_running() || ssh.running;
        ctx.writeln_line(&format!(
            "SSH STATUS\n\nrunning: {}\ntransport: {}\nlisten: {}:{}\nhost key: {}\nsessions: {}\nallow_password: {}\nlast error: {}",
            if running { "yes" } else { "no" },
            if running { "ssh" } else { "stopped" },
            ssh.listen_address,
            ssh.port,
            if ssh.host_key_fingerprint.is_empty() { "-" } else { &ssh.host_key_fingerprint },
            ssh.sessions.len(),
            ssh.allow_password,
            ssh.last_error.as_deref().unwrap_or("-"),
        ))?;
        Ok(())
    }
    #[cfg(not(feature = "ssh"))]
    {
        ctx.writeln_line(&ssh.transport_status())?;
        Ok(())
    }
}

pub fn run_start(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(feature = "ssh")]
    {
        let config = SshServerConfig::default();
        match block_on(crate::hosted::ssh::start_server(config)) {
            Ok(msg) => {
                ctx.kernel.ssh.running = true;
                ctx.writeln(&msg)?;
            }
            Err(err) => ctx.writeln_line(&ssh_err(err))?,
        }
        Ok(())
    }
    #[cfg(not(feature = "ssh"))]
    {
        ctx.writeln_line(&SshError::HostedOnly.reason())?;
        Ok(())
    }
}

pub fn run_stop(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(feature = "ssh")]
    {
        match block_on(crate::hosted::ssh::stop_server()) {
            Ok(msg) => {
                ctx.kernel.ssh.running = false;
                ctx.writeln_line(&msg)?;
            }
            Err(err) => ctx.writeln_line(&ssh_err(err))?,
        }
        Ok(())
    }
    #[cfg(not(feature = "ssh"))]
    {
        ctx.writeln_line(&SshError::HostedOnly.reason())?;
        Ok(())
    }
}

pub fn run_restart(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    run_stop(ctx)?;
    run_start(ctx)
}

pub fn run_sessions(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let mut out = String::from("ID    AUTHORITY   ADDRESS         COMMANDS  FAILED\n");
    for s in &ctx.kernel.ssh.sessions {
        out.push_str(&format!(
            "{:<5} {:<11} {:<15} {:<9} {}\n",
            s.session_id,
            s.authority.label(),
            s.remote_address,
            s.command_count,
            s.failed_command_count
        ));
    }
    ctx.writeln(&out)?;
    Ok(())
}

pub fn run_session_show(ctx: &mut ShellContext<'_>, id: u64) -> Result<(), ShellError> {
    let Some(s) = ctx.kernel.ssh.sessions.iter().find(|s| s.session_id == id) else {
        ctx.writeln_line(&SshError::SessionNotFound.reason())?;
        return Ok(());
    };
    ctx.writeln(&format!(
        "SSH SESSION {id}\n\nauthority: {}\naddress: {}\nconnected_tick: {}\nlast_activity: {}\ncommands: {}\nfailed: {}\ninteractive: {}\nkey: {}\n",
        s.authority.label(),
        s.remote_address,
        s.connected_tick,
        s.last_activity_tick,
        s.command_count,
        s.failed_command_count,
        s.interactive,
        s.key_fingerprint,
    ))?;
    Ok(())
}

pub fn run_session_close(ctx: &mut ShellContext<'_>, id: u64) -> Result<(), ShellError> {
    if ctx.kernel.authority.is_anom_protected() {
        ctx.writeln_line(&SshError::NotPrivileged.reason())?;
        return Ok(());
    }
    let before = ctx.kernel.ssh.sessions.len();
    ctx.kernel.ssh.sessions.retain(|s| s.session_id != id);
    if ctx.kernel.ssh.sessions.len() == before {
        ctx.writeln_line(&SshError::SessionNotFound.reason())?;
    } else {
        ctx.writeln_line(&format!("SSH session {id} closed"))?;
    }
    Ok(())
}

pub fn run_config(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let config = SshServerConfig::default();
    ctx.writeln(&format!(
        "SSH CONFIG\n\nlisten: {}:{}\nhost_key: {}\nauthorized_keys: {}\nmax_sessions: {}\nmax_auth_attempts: {}\nidle_timeout: {}s\nmax_command_length: {}\nbanner: {}\nallow_password: {}\n",
        config.listen_address,
        config.port,
        config.host_key_path,
        config.authorized_keys_path,
        config.max_sessions,
        config.max_auth_attempts,
        config.idle_timeout_seconds,
        config.max_command_length,
        config.banner,
        config.allow_password,
    ))?;
    Ok(())
}

pub fn run_key_list(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let config = SshServerConfig::default();
    match auth::load_key_registry(&config) {
        Ok(keys) => {
            ctx.kernel.ssh.keys = keys;
        }
        Err(err) => {
            ctx.writeln_line(&ssh_err(err))?;
            return Ok(());
        }
    }
    let mut out = String::from("FINGERPRINT          AUTHORITY    ENABLED  IDENTITY\n");
    for key in &ctx.kernel.ssh.keys {
        out.push_str(&format!(
            "{:<20} {:<12} {:<8} {}\n",
            key.fingerprint,
            key.authority.label(),
            if key.enabled { "yes" } else { "no" },
            key.operator_identity
        ));
    }
    ctx.writeln(&out)?;
    Ok(())
}

pub fn run_key_add(ctx: &mut ShellContext<'_>, public_key: &str) -> Result<(), ShellError> {
    if ctx.kernel.authority.is_anom_protected() {
        ctx.writeln_line(&SshError::NotPrivileged.reason())?;
        return Ok(());
    }
    let config = SshServerConfig::default();
    let tick = ctx.kernel.clock.snapshot().chronos_tick;
    match auth::add_key(
        &config,
        ctx.kernel,
        public_key,
        RemoteAuthority::Operator,
        "operator",
        tick,
    ) {
        Ok(fp) => ctx.writeln_line(&format!("ssh key added: {fp}"))?,
        Err(err) => ctx.writeln_line(&ssh_err(err))?,
    }
    Ok(())
}

pub fn run_key_remove(ctx: &mut ShellContext<'_>, fingerprint: &str) -> Result<(), ShellError> {
    if ctx.kernel.authority.is_anom_protected() {
        ctx.writeln_line(&SshError::NotPrivileged.reason())?;
        return Ok(());
    }
    let config = SshServerConfig::default();
    let mut keys = auth::load_key_registry(&config).unwrap_or_default();
    let before = keys.len();
    keys.retain(|k| k.fingerprint != fingerprint);
    if keys.len() == before {
        ctx.writeln_line(&SshError::KeyNotFound.reason())?;
        return Ok(());
    }
    let _ = auth::save_key_registry(&config, &keys);
    ctx.kernel.ssh.keys = keys;
    ctx.writeln_line(&format!("ssh key removed: {fingerprint}"))?;
    Ok(())
}

pub fn run_key_enable(ctx: &mut ShellContext<'_>, fingerprint: &str) -> Result<(), ShellError> {
    toggle_key(ctx, fingerprint, true)
}

pub fn run_key_disable(ctx: &mut ShellContext<'_>, fingerprint: &str) -> Result<(), ShellError> {
    toggle_key(ctx, fingerprint, false)
}

fn toggle_key(
    ctx: &mut ShellContext<'_>,
    fingerprint: &str,
    enabled: bool,
) -> Result<(), ShellError> {
    if ctx.kernel.authority.is_anom_protected() {
        ctx.writeln_line(&SshError::NotPrivileged.reason())?;
        return Ok(());
    }
    let config = SshServerConfig::default();
    let mut keys = auth::load_key_registry(&config).unwrap_or_default();
    let Some(key) = keys.iter_mut().find(|k| k.fingerprint == fingerprint) else {
        ctx.writeln_line(&SshError::KeyNotFound.reason())?;
        return Ok(());
    };
    key.enabled = enabled;
    let _ = auth::save_key_registry(&config, &keys);
    ctx.kernel.ssh.keys = keys;
    ctx.writeln_line(&format!(
        "ssh key {}: {}",
        if enabled { "enabled" } else { "disabled" },
        fingerprint
    ))?;
    Ok(())
}

pub fn run_host_key_fingerprint(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let config = SshServerConfig::default();
    match host_key::ensure_host_key(&config) {
        Ok(material) => {
            ctx.kernel.ssh.host_key_fingerprint = material.fingerprint_sha256.clone();
            ctx.writeln_line(&material.fingerprint_sha256)?;
        }
        Err(err) => ctx.writeln_line(&ssh_err(err))?,
    }
    Ok(())
}

pub fn run_host_key_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let config = SshServerConfig::default();
    let exists = std::path::Path::new(&config.host_key_path).exists();
    ctx.writeln_line(&format!(
        "host key path: {}\nexists: {}\nfingerprint: {}",
        config.host_key_path,
        if exists { "yes" } else { "no" },
        if ctx.kernel.ssh.host_key_fingerprint.is_empty() {
            "-"
        } else {
            &ctx.kernel.ssh.host_key_fingerprint
        }
    ))?;
    Ok(())
}

pub fn run_host_key_regenerate(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    if ctx.kernel.authority.is_anom_protected() {
        ctx.writeln_line(&SshError::NotPrivileged.reason())?;
        return Ok(());
    }
    let config = SshServerConfig::default();
    let _ = std::fs::remove_file(&config.host_key_path);
    match host_key::generate_host_key(&config.host_key_path) {
        Ok(material) => {
            ctx.kernel.ssh.host_key_fingerprint = material.fingerprint_sha256.clone();
            ctx.writeln_line(&format!(
                "ssh host key regenerated\n{}",
                material.fingerprint_sha256
            ))?;
        }
        Err(err) => ctx.writeln_line(&ssh_err(err))?,
    }
    Ok(())
}

pub fn run_audit(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    use crate::kernel::remote_shell::types::SSH_STATECHAIN_OBJECT_ID;
    use crate::kernel::state_proof::ObjectKind;
    let count = ctx
        .kernel
        .statechain
        .chain(ObjectKind::Driver, SSH_STATECHAIN_OBJECT_ID)
        .map(|c| c.events.len())
        .unwrap_or(0);
    ctx.writeln_line(&format!("SSH statechain events: {count}"))?;
    Ok(())
}
