//! Russh session handler — bridges SSH channels to KairOS shell.

use crate::hosted::ssh::auth;
use crate::kernel::event_log::ProcessId;
use crate::kernel::remote_shell::audit::{record_ssh_event, redact_command};
use crate::kernel::remote_shell::session::{
    execute_remote_line, render_session_banner, visibility_label,
};
use crate::kernel::remote_shell::types::{
    AuthorizedKeyRecord, RemoteAuthority, RemoteSessionInfo, SshServerConfig,
};
use crate::platform::String;
use crate::userland::hosted::Hosted;
use async_trait::async_trait;
use russh::server::{Auth, Msg, Session};
use russh::{Channel, ChannelId, CryptoVec};
use russh_keys::key::PublicKey;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct HostState {
    pub hosted: Hosted,
    pub init_pid: ProcessId,
    pub config: SshServerConfig,
}

pub struct SessionHandler {
    pub state: Arc<Mutex<HostState>>,
    pub session_id: u64,
    pub remote_address: String,
    pub authority: Option<RemoteAuthority>,
    pub fingerprint: Option<String>,
    pub channel: Option<ChannelId>,
    pub line_buffer: String,
    pub interactive: bool,
    pub auth_attempts: u8,
}

fn send_data(session: &mut Session, channel: ChannelId, text: &str) {
    session.data(channel, CryptoVec::from_slice(text.as_bytes()));
}

#[async_trait]
impl russh::server::Handler for SessionHandler {
    type Error = russh::Error;

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        let mut guard = self.state.lock().await;
        self.auth_attempts += 1;
        if self.auth_attempts > guard.config.max_auth_attempts {
            record_ssh_event(
                &mut guard.hosted.kernel,
                "ssh_rate_limit_triggered",
                self.remote_address.as_bytes(),
            );
            return Ok(Auth::Reject {
                proceed_with_methods: None,
            });
        }
        let tick = guard.hosted.kernel.clock.snapshot().chronos_tick;
        let config = guard.config.clone();
        match auth::authenticate_publickey(&mut guard.hosted.kernel, &config, user, key, tick) {
            Ok(record) => {
                self.authority = Some(record.authority);
                self.fingerprint = Some(record.fingerprint.clone());
                register_session(self, &mut guard, &record, false);
                Ok(Auth::Accept)
            }
            Err(_) => Ok(Auth::Reject {
                proceed_with_methods: None,
            }),
        }
    }

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        let mut guard = self.state.lock().await;
        if !guard.config.allow_password {
            return Ok(Auth::Reject {
                proceed_with_methods: None,
            });
        }
        let config = guard.config.clone();
        match auth::authenticate_password(&mut guard.hosted.kernel, &config, password) {
            Ok(authority) => {
                if user != crate::kernel::remote_shell::types::DEFAULT_SSH_USER {
                    return Ok(Auth::Reject {
                        proceed_with_methods: None,
                    });
                }
                self.authority = Some(authority);
                self.fingerprint = Some("password-dev".into());
                let record = AuthorizedKeyRecord {
                    fingerprint: "password-dev".into(),
                    public_key_line: String::new(),
                    operator_identity: "dev-password".into(),
                    authority,
                    enabled: true,
                    created_tick: tick_now(&guard),
                    last_used_tick: Some(tick_now(&guard)),
                    expires_tick: None,
                };
                register_session(self, &mut guard, &record, false);
                Ok(Auth::Accept)
            }
            Err(_) => Ok(Auth::Reject {
                proceed_with_methods: None,
            }),
        }
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        if self.authority.is_none() {
            return Ok(false);
        }
        let guard = self.state.lock().await;
        if guard.hosted.kernel.ssh.sessions.len() >= guard.config.max_sessions {
            return Ok(false);
        }
        drop(guard);
        self.channel = Some(channel.id());
        Ok(true)
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.authority.is_none() {
            return Ok(());
        }
        self.channel = Some(channel);
        self.interactive = false;
        let command = String::from_utf8_lossy(data).trim().to_string();
        let output = self.run_command(&command).await;
        send_data(session, channel, &output);
        session.close(channel);
        self.close_session().await;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.authority.is_none() {
            return Ok(());
        }
        self.channel = Some(channel);
        self.interactive = true;
        let authority = self.authority.unwrap();
        let guard = self.state.lock().await;
        let banner = render_session_banner(
            authority,
            self.session_id,
            visibility_label(&guard.hosted.kernel),
        );
        drop(guard);
        send_data(session, channel, &banner);
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if !self.interactive {
            return Ok(());
        }
        if data == [3] {
            session.close(channel);
            self.close_session().await;
            return Ok(());
        }
        let chunk = String::from_utf8_lossy(data);
        self.line_buffer.push_str(&chunk);
        while let Some(pos) = self
            .line_buffer
            .find('\n')
            .or_else(|| self.line_buffer.find('\r'))
        {
            let line = self.line_buffer[..pos].trim().to_string();
            self.line_buffer.drain(..=pos);
            if line.eq_ignore_ascii_case("exit") || line.eq_ignore_ascii_case("quit") {
                send_data(session, channel, "session closed\n");
                session.close(channel);
                self.close_session().await;
                return Ok(());
            }
            let mut output = self.run_command(&line).await;
            output.push_str("gpos> ");
            send_data(session, channel, &output);
        }
        Ok(())
    }

    async fn channel_close(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.close_session().await;
        Ok(())
    }
}

impl SessionHandler {
    async fn run_command(&mut self, line: &str) -> String {
        let authority = match self.authority {
            Some(a) => a,
            None => return "authentication required\n".into(),
        };
        let mut guard = self.state.lock().await;
        record_ssh_event(
            &mut guard.hosted.kernel,
            "ssh_command_executed",
            redact_command(line).as_bytes(),
        );
        let pid = guard.init_pid;
        let max_len = guard.config.max_command_length;
        let tick = tick_now(&guard);
        if let Some(session) = guard
            .hosted
            .kernel
            .ssh
            .sessions
            .iter_mut()
            .find(|s| s.session_id == self.session_id)
        {
            session.last_activity_tick = tick;
            session.command_count += 1;
        }
        match execute_remote_line(&mut guard.hosted, pid, authority, line, max_len) {
            Ok(result) => result.output,
            Err(err) => {
                if let Some(session) = guard
                    .hosted
                    .kernel
                    .ssh
                    .sessions
                    .iter_mut()
                    .find(|s| s.session_id == self.session_id)
                {
                    session.failed_command_count += 1;
                }
                record_ssh_event(
                    &mut guard.hosted.kernel,
                    "ssh_command_denied",
                    err.reason().as_bytes(),
                );
                format!("{}\n", err.reason())
            }
        }
    }

    async fn close_session(&mut self) {
        let mut guard = self.state.lock().await;
        guard
            .hosted
            .kernel
            .ssh
            .sessions
            .retain(|s| s.session_id != self.session_id);
        record_ssh_event(
            &mut guard.hosted.kernel,
            "ssh_session_closed",
            self.session_id.to_string().as_bytes(),
        );
    }
}

fn tick_now(guard: &HostState) -> u64 {
    guard.hosted.kernel.clock.snapshot().chronos_tick
}

fn register_session(
    handler: &SessionHandler,
    guard: &mut HostState,
    record: &AuthorizedKeyRecord,
    interactive: bool,
) {
    let tick = tick_now(guard);
    let info = RemoteSessionInfo {
        session_id: handler.session_id,
        key_fingerprint: record.fingerprint.clone(),
        authority: record.authority,
        remote_address: handler.remote_address.clone(),
        connected_tick: tick,
        last_activity_tick: tick,
        command_count: 0,
        failed_command_count: 0,
        interactive,
    };
    guard.hosted.kernel.ssh.sessions.push(info);
    record_ssh_event(
        &mut guard.hosted.kernel,
        "ssh_session_opened",
        handler.session_id.to_string().as_bytes(),
    );
}

pub struct KairosSshServer {
    pub state: Arc<Mutex<HostState>>,
    pub next_session_id: u64,
}

impl russh::server::Server for KairosSshServer {
    type Handler = SessionHandler;

    fn new_client(&mut self, peer_addr: Option<std::net::SocketAddr>) -> Self::Handler {
        let session_id = self.next_session_id;
        self.next_session_id += 1;
        SessionHandler {
            state: self.state.clone(),
            session_id,
            remote_address: peer_addr
                .map(|a| a.to_string())
                .unwrap_or_else(|| "unknown".into()),
            authority: None,
            fingerprint: None,
            channel: None,
            line_buffer: String::new(),
            interactive: false,
            auth_attempts: 0,
        }
    }
}
