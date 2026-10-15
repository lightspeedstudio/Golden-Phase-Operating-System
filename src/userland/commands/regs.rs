//! regs — VM register file snapshot.

use crate::kernel::event_log::ProcessId;
use crate::platform::{HashMap, String, Vec};
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;
use crate::vm::instruction::Register;
use crate::vm::Cpu;

pub fn render_vms(vms: &HashMap<ProcessId, Cpu>) -> String {
    let mut out = String::new();
    let mut pids: Vec<_> = vms.keys().copied().collect();
    pids.sort_unstable();
    for pid in pids {
        let cpu = vms.get(&pid).expect("pid in keys");
        out.push_str(&format!("pid {pid} pc={} halted={}\n", cpu.pc, cpu.halted));
        for reg in Register::ALL {
            let value = cpu.registers[reg.as_usize()];
            out.push_str(&format!("  {reg:?} = {value:?}\n"));
        }
    }
    out
}

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    if ctx.vms.is_empty() {
        ctx.writeln_line("register system not initialized")?;
        return Ok(());
    }
    ctx.writeln(&render_vms(ctx.vms))?;
    Ok(())
}
