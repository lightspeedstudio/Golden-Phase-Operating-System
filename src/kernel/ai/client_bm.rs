//! Bare-metal AI transport (feature `ssh-bm`) — a smoltcp TCP **client** to an
//! external inference server over GEM0. This is the baremetal counterpart to the
//! std `ai/client.rs` (which uses `std::net::TcpStream`): same portable protocol
//! (`ai/protocol.rs`), different transport. Mirrors the `net_ssh.rs` poll loop.
//!
//! The board has no `std::net`; TCP comes from the same smoltcp stack that serves
//! ping/UDP/TCP/SSH/DHCP. A live `ai ask` needs an inference server reachable on
//! the LAN (on-demand, like the SSH proof).

use crate::board::zynq7000::gem;
use crate::board::zynq7000::gem_phy::GemDevice;
use crate::kernel::ai::protocol::{
    decode_response, encode_request, AiRequestMessage, AiResponseMessage,
};
use crate::kernel::net_ip::{enable_gtc, now, now_ms};
use crate::protocol::pnw1_wire::BOARD_MAC;
use alloc::vec::Vec;
use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::socket::tcp;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr};

const TCP_RX: usize = 8192;
const TCP_TX: usize = 4096;
const LOCAL_PORT: u16 = 49_500;

/// Send `req` to `server:port` from `local`/24 and return the decoded response.
/// Blocks (polling the stack) up to `timeout_s`.
pub fn ask(
    local: [u8; 4],
    server: [u8; 4],
    port: u16,
    req: &AiRequestMessage,
    timeout_s: u32,
) -> Result<AiResponseMessage, &'static str> {
    gem::wire_init().map_err(|_| "ai: GEM0 wire init failed (PHY link down?)")?;
    enable_gtc();

    let mut device = GemDevice;
    let mac = EthernetAddress(BOARD_MAC);
    let config = Config::new(HardwareAddress::Ethernet(mac));
    let mut iface = Interface::new(config, &mut device, now());
    iface.update_ip_addrs(|addrs| {
        let _ = addrs.push(IpCidr::new(
            IpAddress::v4(local[0], local[1], local[2], local[3]),
            24,
        ));
    });

    let mut sockets = SocketSet::new(Vec::new());
    let handle = sockets.add(tcp::Socket::new(
        tcp::SocketBuffer::new(alloc::vec![0u8; TCP_RX]),
        tcp::SocketBuffer::new(alloc::vec![0u8; TCP_TX]),
    ));

    // Initiate the connection.
    {
        let sock = sockets.get_mut::<tcp::Socket>(handle);
        let remote = (
            IpAddress::v4(server[0], server[1], server[2], server[3]),
            port,
        );
        sock.connect(iface.context(), remote, LOCAL_PORT)
            .map_err(|_| "ai: tcp connect failed")?;
    }

    let payload = encode_request(req).into_bytes();
    let mut sent = false;
    let mut resp: Vec<u8> = Vec::new();
    let deadline = now_ms() + (timeout_s as u64) * 1000;

    while now_ms() < deadline {
        let _ = iface.poll(now(), &mut device, &mut sockets);
        let sock = sockets.get_mut::<tcp::Socket>(handle);

        if !sent && sock.can_send() {
            let _ = sock.send_slice(&payload);
            sent = true;
        }
        if sock.can_recv() {
            let _ = sock.recv(|data| {
                resp.extend_from_slice(data);
                (data.len(), ())
            });
            // Response is one newline-delimited JSON object.
            if resp.contains(&b'\n') {
                break;
            }
        }
        // Server closed after replying.
        if sent && !sock.is_active() && !resp.is_empty() {
            break;
        }
    }

    if resp.is_empty() {
        return Err("ai: no response (server unreachable / timeout)");
    }
    let text = core::str::from_utf8(&resp).map_err(|_| "ai: malformed response (utf8)")?;
    let line = text.lines().next().unwrap_or(text);
    decode_response(line).map_err(|_| "ai: malformed response (decode)")
}
