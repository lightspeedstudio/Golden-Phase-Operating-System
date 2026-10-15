//! Network shell commands — VirtIO on RISC-V; GEM0 ladder on PZ7020.

use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        // Prefer live GEM ladder summary when on PZ7020.
        ctx.writeln(&crate::board::zynq7000::gem::run_ladder_report())?;
        ctx.writeln_line("--- kernel network model ---")?;
    }
    let status = ctx.kernel.network.render_status();
    ctx.writeln(&status)?;
    Ok(())
}

pub fn run_init(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        ctx.writeln(&crate::board::zynq7000::gem::run_ladder_report())?;
        return Ok(());
    }
    #[cfg(all(
        feature = "baremetal",
        target_arch = "riscv64",
        not(feature = "std"),
        feature = "bm-net"
    ))]
    {
        match crate::boot::arch::riscv64::virtio_net::init(&mut ctx.kernel.network) {
            Ok(()) => ctx.writeln_line("virtio-net initialized")?,
            Err(err) => ctx.writeln_line(err)?,
        }
        return Ok(());
    }
    #[cfg(not(any(
        all(feature = "zynq-a9", target_arch = "arm"),
        all(
            feature = "baremetal",
            target_arch = "riscv64",
            not(feature = "std"),
            feature = "bm-net"
        )
    )))]
    {
        ctx.writeln_line("net init: baremetal-only")?;
        Ok(())
    }
}

pub fn run_probe(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let (c, cfg, s) = crate::board::zynq7000::gem::probe_registers();
        ctx.writeln_line(&format!(
            "gem probe: NWCTRL=0x{c:08X} NWCFG=0x{cfg:08X} NWSR=0x{s:08X} @ 0xE000B000"
        ))?;
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("net probe: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

pub fn run_phy(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        match crate::board::zynq7000::gem::scan_phy() {
            Ok(p) => ctx.writeln_line(&format!(
                "gem phy: addr={} id1=0x{:04X} id2=0x{:04X}",
                p.addr, p.id1, p.id2
            ))?,
            Err(e) => ctx.writeln_line(&format!("gem phy: FAIL {}", e.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("net phy: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

pub fn run_link(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        match crate::board::zynq7000::gem::scan_phy() {
            Ok(p) => match crate::board::zynq7000::gem::link_status(p.addr) {
                Ok(link) => ctx.writeln_line(&format!(
                    "gem link: phy={} {} bmsr=0x{:04X} speed={} duplex={}",
                    p.addr,
                    if link.up { "UP" } else { "DOWN" },
                    link.bmsr,
                    link.speed_str(),
                    link.duplex_str()
                ))?,
                Err(e) => ctx.writeln_line(&format!("gem link: FAIL {}", e.as_str()))?,
            },
            Err(e) => ctx.writeln_line(&format!("gem link: FAIL {}", e.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("net link: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

pub fn run_loopback(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        match crate::board::zynq7000::gem::mac_loopback_test() {
            Ok(n) => ctx.writeln_line(&format!(
                "gem loopback: PASS rx_len={n} (MAC local only — not wire Ethernet)"
            ))?,
            Err(e) => ctx.writeln_line(&format!("gem loopback: FAIL {}", e.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("net loopback: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

pub fn run_wire_init(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        match crate::board::zynq7000::gem::wire_init() {
            Ok(s) => ctx.writeln_line(&format!(
                "gem wire init: OK 1gprime seq={} live={} ring={} init_count={} tx_head={} rx_head={} loop={}",
                s.seq, s.live, s.ring, s.init_count, s.tx_head, s.rx_head, s.loop_bit
            ))?,
            Err(e) => ctx.writeln_line(&format!("gem wire init: FAIL {}", e.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("net wire: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

pub fn run_wire_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let s = crate::board::zynq7000::gem::wire_status();
        ctx.writeln_line(&format!(
            "gem wire: live={} ring={} init_count={} tx_head={} rx_head={} tx_wraps={} rx_wraps={} seq={} loop={}",
            s.live,
            s.ring,
            s.init_count,
            s.tx_head,
            s.rx_head,
            s.tx_wraps,
            s.rx_wraps,
            s.seq,
            s.loop_bit
        ))?;
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("net wire: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

pub fn run_wire_ping(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        match crate::board::zynq7000::gem::wire_roundtrip() {
            Ok(r) => ctx.writeln_line(&format!(
                "gem wire ping: PASS ethertype=0x88B5 seq={} tx_len={} rx_len={} init={}->{} tx_bd={} rx_bd={} tx_wraps={} rx_wraps={} loop={} (no GEM reinit)",
                r.seq,
                r.tx_len,
                r.rx_len,
                r.init_count_before,
                r.init_count_after,
                r.tx_bd,
                r.rx_bd,
                r.tx_wraps,
                r.rx_wraps,
                r.loop_bit
            ))?,
            Err(e) => ctx.writeln_line(&format!("gem wire ping: FAIL {}", e.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("net wire: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

/// TX-leaves-GEM0 diagnostic: frame dump + BD/TXSR evidence (no peer required).
pub fn run_wire_ping_debug(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        match crate::board::zynq7000::gem::wire_tx_debug_report() {
            Ok(report) => ctx.writeln(&report)?,
            Err(e) => ctx.writeln_line(&format!("gem wire ping debug: FAIL {}", e.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("net wire: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

pub fn run_wire_wrap(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        match crate::board::zynq7000::gem::wire_tx_wrap_test() {
            Ok(s) => ctx.writeln_line(&format!(
                "gem wire wrap: PASS ring={} tx_head={} tx_wraps={} init_count={} loop={}",
                s.ring, s.tx_head, s.tx_wraps, s.init_count, s.loop_bit
            ))?,
            Err(e) => ctx.writeln_line(&format!("gem wire wrap: FAIL {}", e.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("net wire: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

pub fn run_wire_phyloop(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        match crate::board::zynq7000::gem::wire_phy_pcs_loopback() {
            Ok(report) => ctx.writeln(&report)?,
            Err(e) => ctx.writeln_line(&format!("gem wire phyloop: FAIL {}", e.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("net wire: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

pub fn run_wire_txburst(ctx: &mut ShellContext<'_>, count: u32) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        match crate::board::zynq7000::gem::wire_tx_burst(count) {
            Ok(s) => ctx.writeln_line(&s)?,
            Err(e) => ctx.writeln_line(&format!("gem wire txburst: FAIL {}", e.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        let _ = count;
        ctx.writeln_line("net wire: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

pub fn run_wire_recover(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        match crate::board::zynq7000::gem::wire_recover_link() {
            Ok(s) => ctx.writeln_line(&format!(
                "gem wire recover: OK live={} init_count={} (rings preserved)",
                s.live, s.init_count
            ))?,
            Err(e) => ctx.writeln_line(&format!("gem wire recover: FAIL {}", e.as_str()))?,
        }
        return Ok(());
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        ctx.writeln_line("net wire: requires zynq-a9 ARM board build")?;
        Ok(())
    }
}

/// QEMU virtio PN-WIRE-1A peer: serve PNW1 REQ→REP (not a two-board seal).
pub fn run_peer(ctx: &mut ShellContext<'_>, count: u32) -> Result<(), ShellError> {
    #[cfg(all(
        feature = "baremetal",
        target_arch = "riscv64",
        not(feature = "std"),
        feature = "bm-net"
    ))]
    {
        if ctx.kernel.network.driver_state
            != crate::kernel::network::NetworkDriverState::Initialized
        {
            match crate::boot::arch::riscv64::virtio_net::init(&mut ctx.kernel.network) {
                Ok(()) => ctx.writeln_line("virtio-net initialized for peer")?,
                Err(err) => {
                    ctx.writeln_line(err)?;
                    return Ok(());
                }
            }
        }
        // ~few seconds of spin per exchange on QEMU virt.
        const SPINS: u32 = 5_000_000;
        match crate::boot::arch::riscv64::pnw1_peer::serve_many(
            &mut ctx.kernel.network,
            count.max(1),
            SPINS,
        ) {
            Ok(n) => ctx.writeln_line(&format!(
                "net peer: OK replied={n} (PN-WIRE-1A QEMU peer — not two-board seal)"
            ))?,
            Err(err) => ctx.writeln_line(&format!("net peer: FAIL {err}"))?,
        }
        return Ok(());
    }
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let _ = count;
        ctx.writeln_line(
            "net peer: QEMU virtio only — on PZ7020 use `net wire ping` against the peer",
        )?;
        return Ok(());
    }
    #[cfg(not(any(
        all(
            feature = "baremetal",
            target_arch = "riscv64",
            not(feature = "std"),
            feature = "bm-net"
        ),
        all(feature = "zynq-a9", target_arch = "arm")
    )))]
    {
        let _ = count;
        ctx.writeln_line("net peer: requires QEMU bm-net build")?;
        Ok(())
    }
}

pub fn run_tx_test(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        ctx.writeln_line(
            "net tx-test: use `net wire ping` for external 0x88B5 (requires host peer); MAC-local remains `net loopback`",
        )?;
        return Ok(());
    }
    #[cfg(all(
        feature = "baremetal",
        target_arch = "riscv64",
        not(feature = "std"),
        feature = "bm-net"
    ))]
    {
        match crate::boot::arch::riscv64::virtio_net::transmit_test_frame(&mut ctx.kernel.network) {
            Ok(len) => ctx.writeln_line(&format!("virtio-net tx-test sent {len} bytes"))?,
            Err(err) => ctx.writeln_line(err)?,
        }
        return Ok(());
    }
    #[cfg(not(any(
        all(feature = "zynq-a9", target_arch = "arm"),
        all(
            feature = "baremetal",
            target_arch = "riscv64",
            not(feature = "std"),
            feature = "bm-net"
        )
    )))]
    {
        ctx.writeln_line("net tx-test: baremetal-only")?;
        Ok(())
    }
}

pub fn run_rx_test(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        ctx.writeln_line(
            "net rx-test: use `net wire ping` for external 0x88B5 RX (requires host peer); MAC-local remains `net loopback`",
        )?;
        return Ok(());
    }
    #[cfg(all(
        feature = "baremetal",
        target_arch = "riscv64",
        not(feature = "std"),
        feature = "bm-net"
    ))]
    {
        match crate::boot::arch::riscv64::virtio_net::receive_frame(&mut ctx.kernel.network) {
            Ok(len) => {
                let ethertype = ctx
                    .kernel
                    .network
                    .last_rx_ethertype
                    .map(|value| format!("0x{value:04x}"))
                    .unwrap_or_else(|| "unknown".into());
                ctx.writeln_line(&format!(
                    "virtio-net rx-test received {len} bytes ethertype={ethertype}"
                ))?;
            }
            Err(err) => ctx.writeln_line(err)?,
        }
        return Ok(());
    }
    #[cfg(not(any(
        all(feature = "zynq-a9", target_arch = "arm"),
        all(
            feature = "baremetal",
            target_arch = "riscv64",
            not(feature = "std"),
            feature = "bm-net"
        )
    )))]
    {
        ctx.writeln_line("net rx-test: baremetal-only")?;
        Ok(())
    }
}
