//! Device and hardware tools — real PZ7020 MMIO / GIC, no virtio theater.

use crate::userland::irqreg_cmd;
use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::prelude::*;
use crate::userland::tools::types::AuthorityRequirement;

pub fn dev(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("list");
    match sub {
        "list" => {
            ctx.writeln(&render_dev_list())?;
        }
        "show" => {
            let id = args.get(1).map(|s| s.as_str()).unwrap_or("?");
            ctx.writeln(&render_dev_show(id))?;
        }
        "probe" => ctx.writeln(&render_dev_probe())?,
        "tree" => ctx.writeln(&render_dev_tree())?,
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn render_dev_list() -> String {
    #[cfg(all(target_arch = "arm", feature = "zynq-a9"))]
    {
        use crate::board::zynq7000::g1p::G1p;
        use crate::board::zynq7000::gem;
        use crate::board::zynq7000::memory_map::{G1P_AXI_BASE, GEM0_BASE, UART0_BASE};
        let uart_sr = unsafe { core::ptr::read_volatile((UART0_BASE + 0x2C) as *const u32) };
        let (nwctrl, nwcfg, nwsr) = gem::probe_registers();
        let g = G1p::default_window();
        let g1p = match g.probe_identity() {
            Ok(()) => format!("id=0x{:08X}", g.id()),
            Err(_) => format!("UNEXPECTED id=0x{:08X}", g.id()),
        };
        let mut out = String::from("device ID  driver        bus    state              base\n");
        out.push_str(&format!(
            "uart0      ps-uart       mmio   sr=0x{uart_sr:08X}      0x{UART0_BASE:08X}\n"
        ));
        out.push_str(&format!(
            "gem0       cadence-gem   mmio   nwctrl=0x{nwctrl:08X}  0x{GEM0_BASE:08X}\n"
        ));
        let _ = (nwcfg, nwsr);
        #[cfg(feature = "usb")]
        {
            let (report, st) = crate::board::zynq7000::usb::probe();
            let state = if st.connected {
                "connected"
            } else if report.contains("host up") {
                "host-up"
            } else {
                "probe-fail"
            };
            out.push_str(&format!(
                "usb0       ehci          mmio   {state:<18} 0xE0002000\n"
            ));
        }
        out.push_str(&format!(
            "g1p        axi-lite      mmio   {g1p:<18} 0x{G1P_AXI_BASE:08X}\n"
        ));
        #[cfg(feature = "g64-research")]
        {
            use crate::board::zynq7000::g64::G64;
            use crate::board::zynq7000::memory_map::G64_AXI_BASE;
            let g64 = G64::default_window();
            let st = match g64.probe_identity() {
                Ok(()) => format!("id=0x{:08X}", g64.id()),
                Err(_) => {
                    let id = g64.id();
                    if id == crate::board::zynq7000::g1p::EXPECTED_ID {
                        format!("absent(G1P alias) id=0x{id:08X}")
                    } else {
                        format!("UNEXPECTED id=0x{id:08X}")
                    }
                }
            };
            out.push_str(&format!(
                "g64        axi-lite      mmio   {st:<18} 0x{G64_AXI_BASE:08X}\n"
            ));
        }
        out
    }
    #[cfg(not(all(target_arch = "arm", feature = "zynq-a9")))]
    {
        "dev list: uart0/gem0/g1p are PZ7020 board devices (host build — no virtio)\n".into()
    }
}

fn render_dev_show(id: &str) -> String {
    #[cfg(all(target_arch = "arm", feature = "zynq-a9"))]
    {
        match id {
            "uart0" => {
                use crate::board::zynq7000::memory_map::UART0_BASE;
                let sr = unsafe { core::ptr::read_volatile((UART0_BASE + 0x2C) as *const u32) };
                format!("device: uart0\ndriver: ps-uart\nbase: 0x{UART0_BASE:08X}\nSR: 0x{sr:08X}\n")
            }
            "gem0" => {
                let (nwctrl, nwcfg, nwsr) = crate::board::zynq7000::gem::probe_registers();
                format!(
                    "device: gem0\ndriver: cadence-gem\nbase: 0xE000B000\nNWCTRL=0x{nwctrl:08X} NWCFG=0x{nwcfg:08X} NWSR=0x{nwsr:08X}\n"
                )
            }
            "g1p" => {
                use crate::board::zynq7000::g1p::G1p;
                let g = G1p::default_window();
                format!(
                    "device: g1p\nbase: 0x40000000\nid=0x{:08X} version=0x{:08X} caps=0x{:08X} status=0x{:08X}\n",
                    g.id(),
                    g.version(),
                    g.capabilities(),
                    g.status()
                )
            }
            other => format!("device: {other}\nunknown (not virtio)\n"),
        }
    }
    #[cfg(not(all(target_arch = "arm", feature = "zynq-a9")))]
    {
        format!("device: {id}\nPZ7020 board device (host build — no virtio)\n")
    }
}

fn render_dev_probe() -> String {
    #[cfg(all(target_arch = "arm", feature = "zynq-a9"))]
    {
        use crate::board::zynq7000::g1p::G1p;
        let (nwctrl, _, _) = crate::board::zynq7000::gem::probe_registers();
        let g1p_ok = G1p::default_window().probe_identity().is_ok();
        format!("dev probe: uart0 live; gem0 NWCTRL=0x{nwctrl:08X}; g1p identity={g1p_ok}\n")
    }
    #[cfg(not(all(target_arch = "arm", feature = "zynq-a9")))]
    {
        "dev probe: PZ7020 board only (host build — no virtio)\n".into()
    }
}

fn render_dev_tree() -> String {
    #[cfg(all(target_arch = "arm", feature = "zynq-a9"))]
    {
        let mut s = String::from("platform (PZ7020)\n  uart0\n  gem0\n");
        #[cfg(feature = "usb")]
        s.push_str("  usb0\n");
        s.push_str("  g1p @ 0x40000000\n");
        #[cfg(feature = "g64-research")]
        s.push_str("  g64 @ 0x40001000\n");
        s
    }
    #[cfg(not(all(target_arch = "arm", feature = "zynq-a9")))]
    {
        "platform (host)\n  (uart0/gem0/g1p exist on PZ7020 — no virtio)\n".into()
    }
}

pub fn irq(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("list");
    if !matches!(sub, "list" | "show" | "counters") {
        return Err(ToolError::InvalidArguments);
    }
    ctx.writeln_line(&format!("irq {sub}: GIC IDs 29 (timer) / 54 (GEM0 RX) / 61 (G1P)"))?;
    ctx.writeln(&irqreg_cmd::render(ctx.kernel))?;
    ctx.writeln("\n")?;
    Ok(0)
}

pub fn mmio(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Root)?;
    let (op, addr_s, width_s, value_s) = match args.first().map(|s| s.as_str()) {
        Some("write") => (
            "write",
            args.get(1).map(|s| s.as_str()),
            args.get(2).map(|s| s.as_str()),
            args.get(3).map(|s| s.as_str()),
        ),
        Some("read") => (
            "read",
            args.get(1).map(|s| s.as_str()),
            args.get(2).map(|s| s.as_str()),
            None,
        ),
        _ => (
            "read",
            args.first().map(|s| s.as_str()),
            args.get(1).map(|s| s.as_str()),
            None,
        ),
    };
    let addr = parse_u32(addr_s.unwrap_or("")).ok_or(ToolError::InvalidArguments)?;
    let width: u8 = width_s
        .unwrap_or("")
        .parse()
        .map_err(|_| ToolError::InvalidArguments)?;
    if !matches!(width, 8 | 16 | 32) {
        ctx.writeln_line("mmio: width must be 8, 16, or 32")?;
        return Ok(1);
    }
    let window = mmio_window(addr);
    if window.is_none() {
        ctx.writeln_line(&format!(
            "mmio: refuse 0x{addr:08X} — not in allow-list (UART0/GEM0/G1P/G64); DDR and unknown addresses are closed"
        ))?;
        return Ok(1);
    }
    let align = (width / 8) as u32;
    if addr % align != 0 {
        ctx.writeln_line(&format!("mmio: 0x{addr:08X} not aligned to {width}-bit"))?;
        return Ok(1);
    }
    #[cfg(all(target_arch = "arm", feature = "zynq-a9"))]
    {
        match op {
            "read" => {
                let val = unsafe { mmio_read(addr, width) };
                ctx.writeln_line(&format!(
                    "mmio read 0x{addr:08X} width {width} window={} : 0x{val:X}",
                    window.unwrap_or("?")
                ))?;
            }
            "write" => {
                let value = value_s.ok_or(ToolError::InvalidArguments)?;
                let v = parse_u32(value).ok_or(ToolError::InvalidArguments)?;
                unsafe { mmio_write(addr, width, v) };
                ctx.record_statechain("device_write", format!("{addr:#x}:{v:#x}").as_bytes());
                ctx.writeln_line(&format!(
                    "mmio write 0x{addr:08X} width {width} = 0x{v:X} (bus cycle)"
                ))?;
            }
            _ => return Err(ToolError::InvalidArguments),
        }
        return Ok(0);
    }
    #[cfg(not(all(target_arch = "arm", feature = "zynq-a9")))]
    {
        let _ = op;
        ctx.writeln_line("mmio: volatile access is PZ7020-only (host build)")?;
        Ok(1)
    }
}

fn parse_u32(s: &str) -> Option<u32> {
    let t = s.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u32::from_str_radix(h, 16).ok()
    } else {
        t.parse().ok()
    }
}

fn mmio_window(addr: u32) -> Option<&'static str> {
    #[cfg(feature = "zynq-a9")]
    {
        use crate::board::zynq7000::memory_map::{G1P_AXI_BASE, GEM0_BASE, UART0_BASE};
        const WIN: u32 = 0x1000;
        if addr >= UART0_BASE && addr < UART0_BASE + WIN {
            return Some("UART0");
        }
        if addr >= GEM0_BASE && addr < GEM0_BASE + WIN {
            return Some("GEM0");
        }
        if addr >= G1P_AXI_BASE && addr < G1P_AXI_BASE + WIN {
            return Some("G1P");
        }
        #[cfg(feature = "g64-research")]
        {
            use crate::board::zynq7000::memory_map::G64_AXI_BASE;
            if addr >= G64_AXI_BASE && addr < G64_AXI_BASE + WIN {
                return Some("G64");
            }
        }
        None
    }
    #[cfg(not(feature = "zynq-a9"))]
    {
        let _ = addr;
        None
    }
}

#[cfg(all(target_arch = "arm", feature = "zynq-a9"))]
unsafe fn mmio_read(addr: u32, width: u8) -> u32 {
    match width {
        8 => core::ptr::read_volatile(addr as *const u8) as u32,
        16 => core::ptr::read_volatile(addr as *const u16) as u32,
        _ => core::ptr::read_volatile(addr as *const u32),
    }
}

#[cfg(all(target_arch = "arm", feature = "zynq-a9"))]
unsafe fn mmio_write(addr: u32, width: u8, value: u32) {
    match width {
        8 => core::ptr::write_volatile(addr as *mut u8, value as u8),
        16 => core::ptr::write_volatile(addr as *mut u16, value as u16),
        _ => core::ptr::write_volatile(addr as *mut u32, value),
    }
}
