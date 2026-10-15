//! Bare-metal SSH server (feature `ssh-bm`) — a no_std `sunset` SSH engine
//! driven sans-io from the same NO_SYS poll loop as [`crate::kernel::net_ip`],
//! over a smoltcp TCP socket on GEM0.
//!
//! Auth is public-key only against a single compiled-in authorized ed25519 key.
//! An `exec` request (`ssh host "<command>"`) is bridged to the board's own
//! shell executor via a caller-supplied closure, so SSH runs the exact same
//! commands as the UART console. This is the SSH-enabling milestone realized:
//! real TCP + KEX + userauth + channel on bare metal, no OS underneath.
//!
//! Entropy: the CSPRNG comes from [`crate::board::zynq7000::rng`] — see the
//! caveat there; the Zynq-7000 PS has no TRNG, so these keys are proof-grade.

use crate::board::zynq7000::gem;
use crate::board::zynq7000::gem_phy::GemDevice;
use crate::kernel::net_ip::{enable_gtc, now, now_ms};
use crate::platform::String;
use crate::protocol::pnw1_wire::BOARD_MAC;
use alloc::vec::Vec;
use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::socket::tcp;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr};
use sunset::{ChanData, ChanHandle, Event, KeyType, PubKey, Runner, ServEvent, Server, SignKey};

/// Authorized ed25519 public key (raw 32 bytes). All zeros = no login (fail closed).
/// Put your own public key here locally to enable `ssh`; do not commit a login key.
pub const AUTHORIZED_PUBKEY: [u8; 32] = [0u8; 32];
const MAX_SSH_SESSIONS: u32 = 1;
const SSH_LINE_MAX: usize = 128;
const SSH_IDLE_MS: u64 = 30_000;

static mut AUTH_RUNTIME: [u8; 32] = [0u8; 32];

pub fn set_authorized_key(key: [u8; 32]) {
    unsafe {
        AUTH_RUNTIME = key;
    }
}

fn effective_key(compiled: &[u8; 32]) -> [u8; 32] {
    unsafe {
        if AUTH_RUNTIME.iter().any(|&b| b != 0) {
            AUTH_RUNTIME
        } else {
            *compiled
        }
    }
}

fn ssh_audit(msg: &str) {
    crate::board::zynq7000::uart::write_str("ssh audit: ");
    crate::board::zynq7000::uart::write_str(msg);
    crate::board::zynq7000::uart::write_str("\n");
}

/// Runner packet buffers — must fit the largest SSH packet (KEX with ML-KEM
/// hybrid is the big one). Generous; the board has ample RAM.
const SSH_PKT_BUF: usize = 4200;
/// TCP socket ring buffers.
const TCP_RX: usize = 4096;
const TCP_TX: usize = 4096;

/// Extract the 32-byte ed25519 public key, if this is an ed25519 key.
fn ed25519_bytes(pk: &PubKey) -> Option<[u8; 32]> {
    match pk {
        PubKey::Ed25519(e) => Some(e.key.0),
        _ => None,
    }
}

/// Serve SSH on `octets`/24 : `port` for `seconds`. Public-key auth accepts
/// only `authorized_pubkey` (raw ed25519). `run_cmd` executes one shell command
/// line and returns its full textual output (bridged to the board shell).
pub fn serve<F>(
    octets: [u8; 4],
    port: u16,
    seconds: u32,
    authorized_pubkey: &[u8; 32],
    mut run_cmd: F,
) -> Result<String, &'static str>
where
    F: FnMut(&str) -> String,
{
    gem::wire_init().map_err(|_| "net ssh: GEM0 wire init failed (PHY link down?)")?;
    enable_gtc();
    crate::kernel::net_ip::arm_irq_idle();
    let authorized_pubkey = effective_key(authorized_pubkey);

    let mut device = GemDevice;
    let mac = EthernetAddress(BOARD_MAC);
    let config = Config::new(HardwareAddress::Ethernet(mac));
    let mut iface = Interface::new(config, &mut device, now());
    iface.update_ip_addrs(|addrs| {
        let _ = addrs.push(IpCidr::new(
            IpAddress::v4(octets[0], octets[1], octets[2], octets[3]),
            24,
        ));
    });

    let mut sockets = SocketSet::new(Vec::new());
    let tcp_handle = sockets.add(tcp::Socket::new(
        tcp::SocketBuffer::new(alloc::vec![0u8; TCP_RX]),
        tcp::SocketBuffer::new(alloc::vec![0u8; TCP_TX]),
    ));

    // Per-boot ed25519 host key (uses the board CSPRNG via getrandom).
    let host_key = SignKey::generate(KeyType::Ed25519, None).map_err(|_| "net ssh: hostkey")?;

    let mut inbuf = alloc::vec![0u8; SSH_PKT_BUF];
    let mut outbuf = alloc::vec![0u8; SSH_PKT_BUF];

    let mut authed_ever = false;
    let mut conns: u32 = 0;
    let mut execs: u32 = 0;
    let mut shells_total: u32 = 0;
    let deadline = now_ms() + (seconds as u64) * 1000;

    while now_ms() < deadline {
        let _ = iface.poll(now(), &mut device, &mut sockets);
        {
            let sock = sockets.get_mut::<tcp::Socket>(tcp_handle);
            if !sock.is_open() {
                let _ = sock.listen(port);
            }
            if !sock.is_active() {
                crate::kernel::net_ip::idle_wait();
                continue;
            }
        }
        conns = conns.wrapping_add(1);
        if drive_ssh_conn(
            &mut iface,
            &mut device,
            &mut sockets,
            tcp_handle,
            &mut inbuf,
            &mut outbuf,
            &host_key,
            &authorized_pubkey,
            &mut run_cmd,
            &mut execs,
            &mut shells_total,
            deadline,
        ) {
            authed_ever = true;
        }
        ssh_audit("session close");
        {
            let sock = sockets.get_mut::<tcp::Socket>(tcp_handle);
            if sock.is_active() {
                sock.close();
            }
        }
        for _ in 0..32 {
            let _ = iface.poll(now(), &mut device, &mut sockets);
            let sock = sockets.get_mut::<tcp::Socket>(tcp_handle);
            if !sock.is_open() {
                break;
            }
            crate::kernel::net_ip::idle_wait();
        }
    }

    Ok(alloc::format!(
        "net ssh: served {}.{}.{}.{}/24 :{} for {}s (conns={} authed={} execs={} shells={})",
        octets[0],
        octets[1],
        octets[2],
        octets[3],
        port,
        seconds,
        conns,
        authed_ever as u32,
        execs,
        shells_total
    ))
}

fn drive_ssh_conn<F>(
    iface: &mut Interface,
    device: &mut GemDevice,
    sockets: &mut SocketSet,
    tcp_handle: smoltcp::iface::SocketHandle,
    inbuf: &mut [u8],
    outbuf: &mut [u8],
    host_key: &SignKey,
    authorized_pubkey: &[u8; 32],
    run_cmd: &mut F,
    execs: &mut u32,
    shells_total: &mut u32,
    deadline: u64,
) -> bool
where
    F: FnMut(&str) -> String,
{
    let mut runner: Runner<Server> = Runner::new_server(inbuf, outbuf);
    let mut chan: Option<ChanHandle> = None;
    let mut authed = false;
    let mut shells_live: u32 = 0;
    let mut pty = false;
    let mut line_buf: Vec<u8> = Vec::new();
    let mut last_rx = now_ms();
    let mut out_pending: Vec<u8> = Vec::new();
    let mut out_off: usize = 0;
    let mut want_close = false;

    while now_ms() < deadline {
        let _ = iface.poll(now(), device, sockets);
        {
            let sock = sockets.get_mut::<tcp::Socket>(tcp_handle);
            if !sock.is_active() {
                break;
            }
            if sock.can_recv() {
                let _ = sock.recv(|data| {
                    let n = runner.input(data).unwrap_or(0);
                    (n, ())
                });
            }
        }

        loop {
            match runner.progress() {
                Ok(Event::Serv(ev)) => match ev {
                    ServEvent::Hostkeys(h) => {
                        let _ = h.hostkeys(&[host_key]);
                    }
                    ServEvent::FirstAuth(_fa) => {}
                    ServEvent::PasswordAuth(pw) => {
                        let _ = pw.reject();
                    }
                    ServEvent::PubkeyAuth(pk) => {
                        let configured = authorized_pubkey.iter().any(|&b| b != 0);
                        let ok = configured
                            && pk
                                .pubkey()
                                .ok()
                                .and_then(|p| ed25519_bytes(&p))
                                .map(|b| b == *authorized_pubkey)
                                .unwrap_or(false);
                        if ok {
                            let _ = pk.allow();
                            authed = true;
                        } else {
                            let _ = pk.reject();
                        }
                    }
                    ServEvent::OpenSession(s) => {
                        if chan.is_some() || shells_live >= MAX_SSH_SESSIONS {
                            let _ = s;
                        } else if let Ok(h) = s.accept() {
                            chan = Some(h);
                        }
                    }
                    ServEvent::SessionExec(ex) => {
                        if let Ok(cmd) = ex.command() {
                            ssh_audit("exec");
                            let output = run_cmd(cmd);
                            out_pending = output.into_bytes();
                            out_off = 0;
                            want_close = true;
                            *execs = execs.wrapping_add(1);
                        }
                        let _ = ex.succeed();
                        break;
                    }
                    ServEvent::SessionShell(sh) => {
                        if !authed || shells_live >= MAX_SSH_SESSIONS {
                            let _ = sh.fail();
                        } else {
                            let _ = sh.succeed();
                            pty = true;
                            shells_live = shells_live.wrapping_add(1);
                            *shells_total = shells_total.wrapping_add(1);
                            out_pending = b"gpos> ".to_vec();
                            out_off = 0;
                            want_close = false;
                            last_rx = now_ms();
                            ssh_audit("pty open");
                        }
                    }
                    ServEvent::SessionSubsystem(sub) => {
                        let _ = sub.fail();
                    }
                    ServEvent::SessionPty(req) => {
                        let _ = req.succeed();
                    }
                    ServEvent::SessionEnv(_) => {}
                    ServEvent::Defunct => {}
                    ServEvent::PollAgain => break,
                },
                Ok(Event::Progressed) => continue,
                Ok(Event::None) => break,
                Ok(Event::Cli(_)) => break,
                Err(_) => break,
            }
        }

        if pty {
            if let Some(h) = chan.as_ref() {
                let mut tmp = [0u8; 64];
                match runner.read_channel(h, ChanData::Normal, &mut tmp) {
                    Ok(0) => {}
                    Ok(n) => {
                        last_rx = now_ms();
                        for &b in &tmp[..n] {
                            match b {
                                b'\r' | b'\n' => {
                                    let cmd = core::str::from_utf8(&line_buf).unwrap_or("");
                                    ssh_audit("pty cmd");
                                    let mut output = run_cmd(cmd);
                                    if !output.ends_with('\n') && !output.is_empty() {
                                        output.push('\n');
                                    }
                                    output.push_str("gpos> ");
                                    out_pending.extend_from_slice(output.as_bytes());
                                    line_buf.clear();
                                }
                                0x08 | 0x7f => {
                                    if !line_buf.is_empty() {
                                        line_buf.pop();
                                        out_pending.extend_from_slice(b"\x08 \x08");
                                    }
                                }
                                0x03 => {
                                    want_close = true;
                                    ssh_audit("pty interrupt");
                                }
                                c if c >= 0x20 && c < 0x7f => {
                                    if line_buf.len() < SSH_LINE_MAX {
                                        line_buf.push(c);
                                        out_pending.push(c);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(_) => {
                        pty = false;
                        shells_live = 0;
                        ssh_audit("pty eof");
                    }
                }
            }
            if now_ms().saturating_sub(last_rx) > SSH_IDLE_MS {
                want_close = true;
                ssh_audit("pty timeout");
            }
        }

        if let Some(h) = chan.as_ref() {
            while out_off < out_pending.len() {
                match runner.write_channel(h, ChanData::Normal, &out_pending[out_off..]) {
                    Ok(0) => break,
                    Ok(n) => out_off += n,
                    Err(_) => {
                        out_pending.clear();
                        out_off = 0;
                        break;
                    }
                }
            }
            if want_close && out_off >= out_pending.len() {
                let _ = runner.channel_eof(h);
                let _ = runner.channel_exit_status(h, 0);
                let _ = runner.channel_close(h);
                if let Some(h) = chan.take() {
                    let _ = runner.channel_done(h);
                }
                want_close = false;
                out_pending.clear();
                out_off = 0;
            }
        }

        let sock = sockets.get_mut::<tcp::Socket>(tcp_handle);
        loop {
            let out = runner.output_buf();
            if out.is_empty() || !sock.can_send() {
                break;
            }
            let n = sock.send_slice(out).unwrap_or(0);
            if n == 0 {
                break;
            }
            runner.consume_output(n);
        }
    }
    authed
}
