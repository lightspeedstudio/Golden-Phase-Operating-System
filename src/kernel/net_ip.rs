//! Bare-metal IPv4/ICMP/UDP/TCP for Golden Phase OS via smoltcp over GEM0.
//!
//! Mirrors the vendor lwIP `NO_SYS` model (poll loop + GEM netif) but Rust-native:
//! smoltcp is the TCP/IP engine, [`GemDevice`] is the netif shim over the GEM DMA,
//! and the Cortex-A9 global timer provides the monotonic clock smoltcp needs.
//!
//! M1: ARP + ICMP echo so the PC can ping the board (smoltcp auto-replies).
//! M2: a UDP echo socket on port 7 (bounces datagrams back to the sender).
//! M3: a TCP echo listener on port 7 (accepts a connection, echoes bytes) --
//!     this is the SSH-enabling milestone: real TCP on bare metal.

use crate::board::zynq7000::gem;
use crate::board::zynq7000::gem_phy::GemDevice;
use crate::platform::String;
use crate::protocol::pnw1_wire::BOARD_MAC;
use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::socket::{dhcpv4, dns, tcp, udp};
use smoltcp::time::Instant;
use smoltcp::wire::{DnsQueryType, EthernetAddress, HardwareAddress, IpAddress, IpCidr};

/// Echo port used for both the UDP (M2) and TCP (M3) echo services.
const ECHO_PORT: u16 = 7;

// Cortex-A9 private/global timer (SCU global timer). Runs at CPU_3x2x/2.
const GTC_LO: u32 = 0xF8F0_0200; // counter [31:0]
const GTC_HI: u32 = 0xF8F0_0204; // counter [63:32]
const GTC_CTRL: u32 = 0xF8F0_0208; // bit0 = timer enable
                                   // Approx for PZ7020 (~667 MHz CPU -> ~333 MHz global timer). Monotonic is what
                                   // matters for M1; refine for TCP retransmit timing.
const TICKS_PER_MS: u64 = 333_333;

#[inline]
unsafe fn read_gtc() -> u64 {
    loop {
        let hi = core::ptr::read_volatile(GTC_HI as *const u32);
        let lo = core::ptr::read_volatile(GTC_LO as *const u32);
        let hi2 = core::ptr::read_volatile(GTC_HI as *const u32);
        if hi == hi2 {
            return ((hi as u64) << 32) | lo as u64;
        }
    }
}

pub(crate) fn enable_gtc() {
    unsafe {
        // Enable the global timer (idempotent).
        core::ptr::write_volatile(GTC_CTRL as *mut u32, 1);
    }
}

pub(crate) fn now_ms() -> u64 {
    unsafe { read_gtc() / TICKS_PER_MS }
}

pub(crate) fn now() -> Instant {
    Instant::from_millis(now_ms() as i64)
}

/// Arm interrupt-driven idling for the serve loops: unmask the GEM0 RX
/// interrupt and start a ~5 ms private-timer tick. The tick bounds `wfi` so
/// smoltcp's own timers and the serve deadline still run even with no traffic.
/// (CPSR IRQs are already enabled by the board boot path.)
pub(crate) fn arm_irq_idle() {
    use crate::board::zynq7000::gic;
    gic::enable_gem0_rx_irq();
    gic::enable_tick((TICKS_PER_MS as u32).saturating_mul(5));
}

/// Stop the private-timer tick (leaves GEM/G1P interrupts as they are).
pub(crate) fn disarm_irq_idle() {
    crate::board::zynq7000::gic::disable_tick();
}

/// Sleep until the next event. Re-arms the one-shot GEM RX interrupt (frames
/// were drained by the just-completed poll), then `wfi` until a GEM RX frame or
/// the periodic tick wakes the CPU. A frame landing in the brief re-arm window
/// is not lost — it sits in the ring and is drained within one tick.
pub(crate) fn idle_wait() {
    gem::rearm_rx_irq();
    unsafe { core::arch::asm!("wfi", options(nomem, nostack)) };
    let _ = gem::take_rx_ready();
}

/// Bring up IPv4 at `addr`/24 on GEM0 and service ARP + ICMP (M1), a UDP echo
/// socket on port 7 (M2), and a TCP echo listener on port 7 (M3) for `seconds`.
pub fn serve(addr: [u8; 4], seconds: u32) -> Result<String, &'static str> {
    // Bring up the GEM0 wire path (requires PHY link UP).
    gem::wire_init().map_err(|_| "net ip: GEM0 wire init failed (PHY link down?)")?;
    enable_gtc();

    let mut device = GemDevice;
    let mac = EthernetAddress(BOARD_MAC);
    let config = Config::new(HardwareAddress::Ethernet(mac));
    let mut iface = Interface::new(config, &mut device, now());
    iface.update_ip_addrs(|addrs| {
        let _ = addrs.push(IpCidr::new(
            IpAddress::v4(addr[0], addr[1], addr[2], addr[3]),
            24,
        ));
    });

    let mut sockets = SocketSet::new(alloc::vec::Vec::new());

    // M2: UDP echo on ECHO_PORT.
    let udp_rx = udp::PacketBuffer::new(
        alloc::vec![udp::PacketMetadata::EMPTY; 8],
        alloc::vec![0u8; 4096],
    );
    let udp_tx = udp::PacketBuffer::new(
        alloc::vec![udp::PacketMetadata::EMPTY; 8],
        alloc::vec![0u8; 4096],
    );
    let mut udp_socket = udp::Socket::new(udp_rx, udp_tx);
    udp_socket
        .bind(ECHO_PORT)
        .map_err(|_| "net ip: UDP bind failed")?;
    let udp_handle = sockets.add(udp_socket);

    // M3: TCP echo listener on ECHO_PORT.
    let tcp_socket = tcp::Socket::new(
        tcp::SocketBuffer::new(alloc::vec![0u8; 4096]),
        tcp::SocketBuffer::new(alloc::vec![0u8; 4096]),
    );
    let tcp_handle = sockets.add(tcp_socket);

    arm_irq_idle();
    let deadline = now_ms() + (seconds as u64) * 1000;
    let mut polls: u64 = 0;
    let mut udp_echoes: u64 = 0;
    let mut tcp_bytes: u64 = 0;
    let mut tcp_conns: u64 = 0;
    let mut tcp_was_active = false;
    let mut scratch = [0u8; 1500];

    while now_ms() < deadline {
        let _ = iface.poll(now(), &mut device, &mut sockets);

        // --- UDP echo: bounce every datagram back to its sender ---
        {
            let s = sockets.get_mut::<udp::Socket>(udp_handle);
            while s.can_recv() {
                let (ep, n) = {
                    let (data, meta) = match s.recv() {
                        Ok(v) => v,
                        Err(_) => break,
                    };
                    let n = data.len().min(scratch.len());
                    scratch[..n].copy_from_slice(&data[..n]);
                    (meta.endpoint, n)
                };
                if s.can_send() {
                    let _ = s.send_slice(&scratch[..n], ep);
                    udp_echoes = udp_echoes.wrapping_add(1);
                }
            }
        }

        // --- TCP echo: listen, echo bytes, re-listen after the peer closes ---
        {
            let s = sockets.get_mut::<tcp::Socket>(tcp_handle);
            if !s.is_open() {
                let _ = s.listen(ECHO_PORT);
                tcp_was_active = false;
            }
            if s.is_active() && !tcp_was_active {
                tcp_conns = tcp_conns.wrapping_add(1);
                tcp_was_active = true;
            }
            if s.can_recv() {
                let n = s.recv_slice(&mut scratch).unwrap_or(0);
                if n > 0 && s.can_send() {
                    let _ = s.send_slice(&scratch[..n]);
                    tcp_bytes = tcp_bytes.wrapping_add(n as u64);
                }
            }
            if s.state() == tcp::State::CloseWait {
                s.close();
            }
        }

        polls = polls.wrapping_add(1);
        idle_wait();
    }
    disarm_irq_idle();

    Ok(alloc::format!(
        "net ip: served {}.{}.{}.{}/24 mac 02:00:00:47:01:50 on GEM0 for {}s\n\
         (polls={} icmp=auto udp_echoes={} tcp_conns={} tcp_bytes={} gem_rx_irq={} tick={})\n\
         ARP+ICMP, UDP echo :7, TCP echo :7 active (interrupt-driven RX)",
        addr[0],
        addr[1],
        addr[2],
        addr[3],
        seconds,
        polls,
        udp_echoes,
        tcp_conns,
        tcp_bytes,
        crate::board::zynq7000::gic::gem_rx_irq_count(),
        crate::board::zynq7000::gic::tick_count()
    ))
}

/// Build a fresh smoltcp interface on GEM0 with BOARD_MAC and no IP address
/// (the caller assigns one, statically or via DHCP).
fn new_iface(device: &mut GemDevice) -> Interface {
    let mac = EthernetAddress(BOARD_MAC);
    let config = Config::new(HardwareAddress::Ethernet(mac));
    Interface::new(config, device, now())
}

/// Apply a DHCP-provided config to the interface: set the leased address and
/// default route. Returns a human-readable lease summary.
fn apply_dhcp(iface: &mut Interface, cfg: &dhcpv4::Config) -> String {
    iface.update_ip_addrs(|addrs| {
        addrs.clear();
        let _ = addrs.push(IpCidr::Ipv4(cfg.address));
    });
    match cfg.router {
        Some(router) => {
            let _ = iface.routes_mut().add_default_ipv4_route(router);
        }
        None => {
            iface.routes_mut().remove_default_ipv4_route();
        }
    }
    let a = cfg.address.address().octets();
    let mut out = alloc::format!(
        "lease {}.{}.{}.{}/{}",
        a[0],
        a[1],
        a[2],
        a[3],
        cfg.address.prefix_len()
    );
    if let Some(r) = cfg.router {
        let r = r.octets();
        out.push_str(&alloc::format!(" gw {}.{}.{}.{}", r[0], r[1], r[2], r[3]));
    }
    for d in cfg.dns_servers.iter() {
        let d = d.octets();
        out.push_str(&alloc::format!(" dns {}.{}.{}.{}", d[0], d[1], d[2], d[3]));
    }
    out
}

/// M3 (feature 3): acquire an IPv4 lease over GEM0 via DHCP within `seconds`.
/// Needs a DHCP server on the LAN (a router or host ICS) — a bare point-to-point
/// cable has none. Returns the lease summary, or an error on timeout.
pub fn dhcp(seconds: u32) -> Result<String, &'static str> {
    gem::wire_init().map_err(|_| "net ip dhcp: GEM0 wire init failed (PHY link down?)")?;
    enable_gtc();

    let mut device = GemDevice;
    let mut iface = new_iface(&mut device);
    let mut sockets = SocketSet::new(alloc::vec::Vec::new());
    let dhcp_handle = sockets.add(dhcpv4::Socket::new());

    let deadline = now_ms() + (seconds as u64) * 1000;
    while now_ms() < deadline {
        let _ = iface.poll(now(), &mut device, &mut sockets);
        let event = sockets.get_mut::<dhcpv4::Socket>(dhcp_handle).poll();
        match event {
            Some(dhcpv4::Event::Configured(cfg)) => {
                let summary = apply_dhcp(&mut iface, &cfg);
                return Ok(alloc::format!("net ip dhcp: {summary}"));
            }
            Some(dhcpv4::Event::Deconfigured) => {
                iface.update_ip_addrs(|addrs| addrs.clear());
                iface.routes_mut().remove_default_ipv4_route();
            }
            None => {}
        }
    }
    Err("net ip dhcp: no lease within window (is a DHCP server on the LAN?)")
}

/// M3 (feature 3): acquire a lease via DHCP, then resolve `host` (A record)
/// through the DHCP-provided DNS server, all within `seconds`.
pub fn dns(host: &str, seconds: u32) -> Result<String, &'static str> {
    gem::wire_init().map_err(|_| "net dns: GEM0 wire init failed (PHY link down?)")?;
    enable_gtc();

    let mut device = GemDevice;
    let mut iface = new_iface(&mut device);
    let mut sockets = SocketSet::new(alloc::vec::Vec::new());
    let dhcp_handle = sockets.add(dhcpv4::Socket::new());

    let deadline = now_ms() + (seconds as u64) * 1000;

    // Phase 1: DHCP until we have an address + at least one DNS server.
    let mut dns_servers: alloc::vec::Vec<IpAddress> = alloc::vec::Vec::new();
    while now_ms() < deadline && dns_servers.is_empty() {
        let _ = iface.poll(now(), &mut device, &mut sockets);
        if let Some(dhcpv4::Event::Configured(cfg)) =
            sockets.get_mut::<dhcpv4::Socket>(dhcp_handle).poll()
        {
            apply_dhcp(&mut iface, &cfg);
            for d in cfg.dns_servers.iter() {
                dns_servers.push(IpAddress::Ipv4(*d));
            }
        }
    }
    if dns_servers.is_empty() {
        return Err("net dns: no DHCP lease / DNS server within window");
    }

    // Phase 2: DNS query for the A record.
    let dns_handle = sockets.add(dns::Socket::new(&dns_servers, alloc::vec::Vec::new()));
    let query = sockets
        .get_mut::<dns::Socket>(dns_handle)
        .start_query(iface.context(), host, DnsQueryType::A)
        .map_err(|_| "net dns: could not start query")?;

    while now_ms() < deadline {
        let _ = iface.poll(now(), &mut device, &mut sockets);
        match sockets
            .get_mut::<dns::Socket>(dns_handle)
            .get_query_result(query)
        {
            Ok(addrs) => {
                for a in addrs.iter() {
                    if let IpAddress::Ipv4(v4) = a {
                        let o = v4.octets();
                        return Ok(alloc::format!(
                            "net dns: {host} -> {}.{}.{}.{}",
                            o[0],
                            o[1],
                            o[2],
                            o[3]
                        ));
                    }
                }
                return Err("net dns: no A record in response");
            }
            Err(dns::GetQueryResultError::Pending) => {}
            Err(_) => return Err("net dns: query failed"),
        }
    }
    Err("net dns: query timed out")
}
