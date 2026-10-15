//! SSH remote operator channel tests.

#![cfg(feature = "ssh")]

use kairos::hosted::ssh::auth::{
    add_key, authenticate_password, authenticate_publickey, load_key_registry, parse_registry,
    save_key_registry,
};
use kairos::hosted::ssh::host_key::{ensure_host_key, generate_host_key, load_host_key};
use kairos::kernel::remote_shell::audit::redact_command;
use kairos::kernel::remote_shell::policy::{authorize_command, required_authority};
use kairos::kernel::remote_shell::session::{execute_remote_line, render_session_banner};
use kairos::kernel::remote_shell::types::{RemoteAuthority, SshServerConfig, DEFAULT_SSH_USER};
use kairos::userland::hosted::Hosted;
use kairos::userland::init::boot;
use kairos::userland::parser::{parse, Command};
use russh_keys::key::KeyPair;
use russh_keys::write_public_key_base64;
use std::fs;
use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_ssh_config() -> (SshServerConfig, std::path::PathBuf) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("kairos-ssh-test-{stamp}"));
    fs::create_dir_all(&root).unwrap();
    let base = root.to_string_lossy().into_owned();
    let config = SshServerConfig {
        listen_address: "127.0.0.1".into(),
        port: 0,
        host_key_path: format!("{base}/ssh_host_ed25519_key"),
        authorized_keys_path: format!("{base}/authorized_keys"),
        key_registry_path: format!("{base}/key_registry.json"),
        ..SshServerConfig::default()
    };
    (config, root)
}

fn ed25519_public_line() -> (KeyPair, String) {
    let key_pair = KeyPair::generate_ed25519().expect("ed25519 key");
    let public = key_pair.clone_public_key().expect("public key");
    let mut line = String::new();
    let mut buf = Cursor::new(Vec::new());
    write_public_key_base64(&mut buf, &public).expect("encode public key");
    line.push_str(std::str::from_utf8(buf.get_ref()).unwrap().trim());
    (key_pair, line)
}

#[test]
fn host_key_generate_load_and_fingerprint() {
    let (config, _root) = temp_ssh_config();
    let generated = generate_host_key(&config.host_key_path).expect("generate");
    assert!(!generated.fingerprint_sha256.is_empty());
    let loaded = load_host_key(&config.host_key_path).expect("load");
    assert_eq!(generated.fingerprint_sha256, loaded.fingerprint_sha256);
    let ensured = ensure_host_key(&config).expect("ensure");
    assert_eq!(generated.fingerprint_sha256, ensured.fingerprint_sha256);
}

#[test]
fn key_registry_roundtrip_and_auth() {
    let (config, _root) = temp_ssh_config();
    let (_client, line) = ed25519_public_line();
    let public = kairos::hosted::ssh::auth::parse_authorized_line(&line).expect("parse line");

    let mut hosted = Hosted::new();
    let _pid = boot(&mut hosted);
    let fp = add_key(
        &config,
        &mut hosted.kernel,
        &line,
        RemoteAuthority::Operator,
        "test-operator",
        1,
    )
    .expect("add key");

    let saved = fs::read_to_string(&config.key_registry_path).expect("registry file");
    let parsed = parse_registry(&saved).expect("parse registry");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].fingerprint, fp);

    let record = authenticate_publickey(&mut hosted.kernel, &config, DEFAULT_SSH_USER, &public, 2)
        .expect("auth ok");
    assert_eq!(record.authority, RemoteAuthority::Operator);

    let reloaded = load_key_registry(&config).expect("reload");
    assert_eq!(reloaded[0].last_used_tick, Some(2));
}

#[test]
fn password_auth_disabled_by_default() {
    let (config, _root) = temp_ssh_config();
    let mut hosted = Hosted::new();
    let _pid = boot(&mut hosted);
    let err = authenticate_password(&mut hosted.kernel, &config, "secret")
        .expect_err("password disabled");
    assert_eq!(
        err,
        kairos::kernel::remote_shell::error::SshError::AuthenticationFailed
    );
}

#[test]
fn policy_authority_matrix() {
    assert_eq!(required_authority(&Command::Help), RemoteAuthority::Viewer);
    assert_eq!(
        required_authority(&Command::Checkpoint),
        RemoteAuthority::Root
    );
    assert_eq!(
        required_authority(&Command::AnomProtect {
            object: "obj1".into(),
            passphrase: true,
        }),
        RemoteAuthority::AnomAdministrator
    );
    assert!(authorize_command(RemoteAuthority::Viewer, &Command::Help).is_ok());
    assert!(authorize_command(RemoteAuthority::Viewer, &Command::Checkpoint).is_err());
    assert!(authorize_command(RemoteAuthority::Root, &Command::Checkpoint).is_ok());
    assert!(authorize_command(
        RemoteAuthority::AnomAdministrator,
        &Command::AnomProtect {
            object: "obj1".into(),
            passphrase: true,
        }
    )
    .is_ok());
    assert!(authorize_command(RemoteAuthority::AnomAdministrator, &Command::Checkpoint).is_err());
}

#[test]
fn remote_shell_executes_help_for_viewer() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    let result =
        execute_remote_line(&mut hosted, pid, RemoteAuthority::Viewer, "help", 4096).expect("help");
    assert!(!result.exit);
    assert!(result.output.contains("spawn"));
}

#[test]
fn remote_shell_denies_checkpoint_for_viewer() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    let err = execute_remote_line(
        &mut hosted,
        pid,
        RemoteAuthority::Viewer,
        "checkpoint",
        4096,
    )
    .expect_err("denied");
    assert_eq!(
        err,
        kairos::kernel::remote_shell::error::RemoteShellError::AuthorityDenied
    );
}

#[test]
fn audit_redacts_sensitive_commands() {
    assert_eq!(
        redact_command("anom protect obj -p secret"),
        "anom:<redacted>"
    );
    assert_eq!(
        redact_command("ai ask what is the passphrase"),
        "ai ask:<redacted>"
    );
    let long = format!("{} {}", "cmd".repeat(20), "tail");
    assert!(redact_command(&long).ends_with(":<truncated>"));
}

#[test]
fn ssh_parser_and_banner() {
    assert_eq!(parse("ssh status"), Command::SshStatus);
    assert_eq!(parse("ssh key list"), Command::SshKeyList);
    let banner = render_session_banner(RemoteAuthority::Operator, 7, "transparent");
    assert!(banner.contains("Golden Phase Operating System Secure Operator Shell"));
    assert!(banner.contains("authority: operator"));
    assert!(banner.contains("session: 7"));
    println!("KAIROS SSH SMOKE OK");
}

#[test]
fn disabled_key_rejected() {
    let (config, _root) = temp_ssh_config();
    let (_client, line) = ed25519_public_line();
    let public = kairos::hosted::ssh::auth::parse_authorized_line(&line).expect("parse");

    let mut hosted = Hosted::new();
    let _pid = boot(&mut hosted);
    add_key(
        &config,
        &mut hosted.kernel,
        &line,
        RemoteAuthority::Viewer,
        "disabled-user",
        1,
    )
    .unwrap();

    let mut keys = load_key_registry(&config).unwrap();
    keys[0].enabled = false;
    save_key_registry(&config, &keys).unwrap();

    let err = authenticate_publickey(&mut hosted.kernel, &config, DEFAULT_SSH_USER, &public, 2)
        .expect_err("disabled");
    assert_eq!(
        err,
        kairos::kernel::remote_shell::error::SshError::AuthenticationFailed
    );
}
