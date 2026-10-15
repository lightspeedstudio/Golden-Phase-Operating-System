//! SSH server configuration helpers.

pub use crate::kernel::remote_shell::types::SshServerConfig;

pub fn config_from_env() -> SshServerConfig {
    let mut config = SshServerConfig::default();
    if let Ok(addr) = std::env::var("KAIROS_SSH_LISTEN") {
        config.listen_address = addr;
    }
    if let Ok(port) = std::env::var("KAIROS_SSH_PORT") {
        if let Ok(port) = port.parse() {
            config.port = port;
        }
    }
    if std::env::var("KAIROS_SSH_ALLOW_PASSWORD").ok().as_deref() == Some("1") {
        config.allow_password = true;
    }
    config
}
