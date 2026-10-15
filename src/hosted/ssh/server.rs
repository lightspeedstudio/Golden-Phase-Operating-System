//! Hosted SSH server lifecycle.

use crate::hosted::ssh::auth::load_key_registry;
use crate::hosted::ssh::config::{config_from_env, SshServerConfig};
use crate::hosted::ssh::handler::{HostState, KairosSshServer};
use crate::hosted::ssh::host_key::ensure_host_key;
use crate::kernel::remote_shell::audit::record_ssh_event;
use crate::kernel::remote_shell::error::SshError;
use crate::runtime::loader::Loader;
use crate::userland::hosted::Hosted;
use russh::server::Config;
use russh::server::Server;
use russh::MethodSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub struct SshServerHandle {
    pub join: JoinHandle<()>,
    pub fingerprint: String,
    pub listen: String,
}

static RUNNING: std::sync::OnceLock<Mutex<Option<SshServerHandle>>> = std::sync::OnceLock::new();

fn running_slot() -> &'static Mutex<Option<SshServerHandle>> {
    RUNNING.get_or_init(|| Mutex::new(None))
}

pub async fn start_server(config: SshServerConfig) -> Result<String, SshError> {
    let mut slot = running_slot().lock().await;
    if slot.is_some() {
        return Err(SshError::AlreadyRunning);
    }

    let host_key = ensure_host_key(&config)?;
    let fingerprint = host_key.fingerprint_sha256.clone();

    let mut hosted = Hosted::new();
    for _ in 0..4 {
        let page = hosted.kernel.alloc_page();
        let _ = hosted.kernel.map_user(page);
    }
    let _ = hosted.kernel.fs.mkdir("/tmp");
    let pid = Loader::boot_init(&mut hosted);
    hosted.init_pid = Some(pid);
    hosted.kernel.init_state_domain();
    hosted.kernel.ssh.running = true;
    hosted.kernel.ssh.listen_address = config.listen_address.clone();
    hosted.kernel.ssh.port = config.port;
    hosted.kernel.ssh.host_key_fingerprint = fingerprint.clone();
    hosted.kernel.ssh.allow_password = config.allow_password;
    hosted.kernel.ssh.keys = load_key_registry(&config)?;

    record_ssh_event(
        &mut hosted.kernel,
        "ssh_server_started",
        config.listen_address.as_bytes(),
    );

    let state = Arc::new(Mutex::new(HostState {
        hosted,
        init_pid: pid,
        config: config.clone(),
    }));

    let mut methods = MethodSet::all();
    if !config.allow_password {
        methods -= MethodSet::PASSWORD;
    }

    let russh_config = Arc::new(Config {
        inactivity_timeout: Some(Duration::from_secs(config.idle_timeout_seconds)),
        auth_rejection_time: Duration::from_secs(1),
        max_auth_attempts: config.max_auth_attempts as usize,
        methods,
        keys: vec![host_key.key_pair],
        ..Default::default()
    });

    let addr = format!("{}:{}", config.listen_address, config.port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| SshError::BindFailed(e.to_string()))?;

    let mut server = KairosSshServer {
        state: state.clone(),
        next_session_id: 1,
    };

    let join = tokio::spawn(async move {
        let _ = server.run_on_socket(russh_config, &listener).await;
        let mut guard = state.lock().await;
        guard.hosted.kernel.ssh.running = false;
        record_ssh_event(&mut guard.hosted.kernel, "ssh_server_stopped", b"");
    });

    let listen = format!("{}:{}", config.listen_address, config.port);
    *slot = Some(SshServerHandle {
        join,
        fingerprint: fingerprint.clone(),
        listen: listen.clone(),
    });

    Ok(format!(
        "KairOS SSH server started\nlisten: {listen}\nhost key: {fingerprint}\n\nConnect with:\nssh kairos@{} -p {}",
        config.listen_address, config.port
    ))
}

pub async fn stop_server() -> Result<String, SshError> {
    let mut slot = running_slot().lock().await;
    let Some(handle) = slot.take() else {
        return Err(SshError::NotRunning);
    };
    handle.join.abort();
    Ok("SSH server stopped".into())
}

pub async fn start_from_env() -> Result<String, SshError> {
    start_server(config_from_env()).await
}

pub fn is_running() -> bool {
    RUNNING
        .get()
        .and_then(|m| m.try_lock().ok())
        .map(|g| g.is_some())
        .unwrap_or(false)
}
