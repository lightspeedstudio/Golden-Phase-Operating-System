//! Shell commands for GPL Transfer Base (Project A).

use crate::kernel::phase_transport::lane::TransportPort;
use crate::kernel::phase_transport::packet::PacketOpcode;
use crate::kernel::phase_transport::TransportController;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

fn need_ctl<'a, 'k>(
    ctx: &'a mut ShellContext<'k>,
) -> Result<&'a mut TransportController, ShellError> {
    ctx.kernel.transport.as_mut().ok_or_else(|| {
        ShellError::Unknown("transport not initialized — run: transport init <size>".into())
    })
}

pub fn run_init(ctx: &mut ShellContext<'_>, size: usize) -> Result<(), ShellError> {
    let ctl = TransportController::init(size).map_err(|e| ShellError::Unknown(format!("{e:?}")))?;
    ctx.kernel.transport = Some(ctl);
    ctx.writeln(&format!("TRANSPORT INIT m={size}\n"))?;
    Ok(())
}

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let text = need_ctl(ctx)?.status();
    ctx.writeln(&text)?;
    Ok(())
}

pub fn run_send(
    ctx: &mut ShellContext<'_>,
    sr: usize,
    sc: usize,
    dr: usize,
    dc: usize,
    opcode: &str,
    payload: &str,
) -> Result<(), ShellError> {
    let op =
        PacketOpcode::parse(opcode).ok_or_else(|| ShellError::Unknown("unknown opcode".into()))?;
    let id = need_ctl(ctx)?
        .send((sr, sc), (dr, dc), op, payload.as_bytes(), 0)
        .map_err(|e| ShellError::Unknown(format!("{e:?}")))?;
    ctx.writeln(&format!("SENT packet_id={id}\n"))?;
    Ok(())
}

pub fn run_tick(ctx: &mut ShellContext<'_>, n: u64) -> Result<(), ShellError> {
    let notes = need_ctl(ctx)?
        .tick(n)
        .map_err(|e| ShellError::Unknown(format!("{e:?}")))?;
    for note in notes {
        ctx.writeln(&format!("{note}\n"))?;
    }
    ctx.writeln(&format!("TICKED {n}\n"))?;
    Ok(())
}

pub fn run_receive(ctx: &mut ShellContext<'_>, lane: u32) -> Result<(), ShellError> {
    let r = need_ctl(ctx)?
        .receive(TransportPort {
            lane_id: lane,
            coord: (0, 0),
        })
        .map_err(|e| ShellError::Unknown(format!("{e:?}")))?;
    ctx.writeln(&format!("RECEIVE {r:?}\n"))?;
    Ok(())
}

pub fn run_routes(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let text = {
        let ctl = need_ctl(ctx)?;
        let mut s = crate::platform::String::new();
        for r in &ctl.routers {
            s.push_str(&format!(
                "router id={} in={} out={} port={:?}\n",
                r.id, r.input_lane, r.output_lane, r.port
            ));
        }
        if ctl.routers.is_empty() {
            s.push_str("no routers configured\n");
        }
        s
    };
    ctx.writeln(&text)?;
    Ok(())
}

pub fn run_trace(ctx: &mut ShellContext<'_>, packet_id: u64) -> Result<(), ShellError> {
    let text = {
        let ctl = need_ctl(ctx)?;
        ctl.verify_trace(packet_id)
            .map_err(|e| ShellError::Unknown(format!("{e:?}")))?;
        let tr = ctl
            .traces
            .iter()
            .find(|t| t.packet_id == packet_id)
            .ok_or_else(|| ShellError::Unknown("trace not found".into()))?;
        format!(
            "TRACE id={} steps={} digest={:02x?} expected={} actual={:?}\n",
            tr.packet_id,
            tr.steps.len(),
            &tr.digest[..8],
            tr.expected_arrival,
            tr.actual_arrival
        )
    };
    ctx.writeln(&text)?;
    Ok(())
}

pub fn run_replay(ctx: &mut ShellContext<'_>, packet_id: u64) -> Result<(), ShellError> {
    let d = need_ctl(ctx)?
        .replay_trace(packet_id)
        .map_err(|e| ShellError::Unknown(format!("{e:?}")))?;
    ctx.writeln(&format!("REPLAY OK digest={:02x?}\n", &d[..8]))?;
    Ok(())
}

pub fn run_inject_fault(
    ctx: &mut ShellContext<'_>,
    packet_id: u64,
    fault: &str,
) -> Result<(), ShellError> {
    need_ctl(ctx)?
        .inject_fault(packet_id, fault)
        .map_err(|e| ShellError::Unknown(format!("{e:?}")))?;
    ctx.writeln(&format!("FAULT injected {fault} on {packet_id}\n"))?;
    Ok(())
}

pub fn run_reset(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    need_ctl(ctx)?.reset();
    ctx.writeln("TRANSPORT RESET\n")?;
    Ok(())
}

pub fn run_demo(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    run_init(ctx, 4)?;
    run_send(ctx, 2, 2, 0, 0, "data", "demo")?;
    run_tick(ctx, 2)?;
    run_status(ctx)?;
    let ack = {
        let ctl = need_ctl(ctx)?;
        let id = ctl.packets[0].packet_id;
        ctl.acknowledge(id)
            .map_err(|e| ShellError::Unknown(format!("{e:?}")))?
    };
    ctx.writeln(&format!("ACK packet={ack}\n"))?;
    run_tick(ctx, 2)?;
    run_status(ctx)?;
    ctx.writeln("TRANSPORT DEMO PASS\n")?;
    Ok(())
}
