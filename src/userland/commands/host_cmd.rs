//! host — machine identity, devices, and NIC summary.

use crate::kernel::anom::{authority_label, visibility_label};
use crate::kernel::network::NetworkDriverState;
use crate::platform::String;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;
#[cfg(feature = "std")]
use crate::userland::terminal_fmt::short_hex;

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "std", not(feature = "baremetal")))]
    let hosted = true;
    #[cfg(not(all(feature = "std", not(feature = "baremetal"))))]
    let hosted = false;
    let target = crate::brand::build_target_label(hosted);

    let net = &ctx.kernel.network;
    let mut out = String::from("=== Golden Phase Operating System host ===\n");
    out.push_str(&format!(
        "authority: {}\nvisibility: {}\n",
        authority_label(ctx.kernel.authority.authority),
        visibility_label(ctx.kernel.authority.visibility),
    ));
    out.push_str(&format!(
        "anom: {}\n",
        if ctx.kernel.authority.is_anom_protected() {
            "active"
        } else {
            "standby"
        }
    ));
    out.push_str(&format!("target: {target}\n"));
    out.push_str(&format!("node: {}\n", net.node_name));
    #[cfg(feature = "std")]
    {
        out.push_str(&format!(
            "boot-session: {}\n",
            short_hex(&ctx.kernel.authority.boot_session_root)
        ));
        out.push_str(&format!(
            "equation-root: {}\n",
            short_hex(&ctx.kernel.statechain.equation_root())
        ));
    }
    out.push_str("\ndevices:\n");
    out.push_str("  ID           driver                 bus    state\n");
    out.push_str("  uart0        console-stdio          host   ready\n");
    let nic_state = match net.driver_state {
        NetworkDriverState::Unavailable => "absent",
        NetworkDriverState::Detected => "detected",
        NetworkDriverState::Initialized => "ready",
    };
    out.push_str(&format!(
        "  {:<12} {:<22} {:<6} {}\n",
        net.interface_name, net.driver_name, "mmio", nic_state
    ));
    out.push_str("\nnetwork:\n");
    out.push_str(&ctx.kernel.network.render_status());
    if net.mac_address.is_some() {
        out.push_str("locally-administered MAC derived from boot-session\n");
    }
    ctx.writeln(&out)?;
    Ok(())
}
