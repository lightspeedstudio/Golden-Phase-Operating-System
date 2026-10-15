//! Remote transport abstraction — hosted SSH vs baremetal messaging targets.

use crate::kernel::remote_shell::types::RemoteTransportKind;

/// Transports compiled into this build.
pub fn available_transports() -> &'static [RemoteTransportKind] {
    #[cfg(feature = "ssh")]
    {
        &[
            RemoteTransportKind::Ssh,
            RemoteTransportKind::PhaseLinkGateway,
            RemoteTransportKind::UartConsole,
            RemoteTransportKind::RawTcp,
        ]
    }
    #[cfg(not(feature = "ssh"))]
    {
        &[
            RemoteTransportKind::PhaseLinkGateway,
            RemoteTransportKind::UartConsole,
            RemoteTransportKind::RawTcp,
        ]
    }
}

pub fn transport_label(kind: RemoteTransportKind) -> &'static str {
    match kind {
        #[cfg(feature = "ssh")]
        RemoteTransportKind::Ssh => "ssh",
        RemoteTransportKind::PhaseLinkGateway => "phaselink-gateway",
        RemoteTransportKind::UartConsole => "uart-console",
        RemoteTransportKind::RawTcp => "raw-tcp",
    }
}

/// True when the hosted russh server can be started on this target.
pub fn can_host_ssh() -> bool {
    cfg!(feature = "ssh")
}

/// Baremetal targets route operator traffic over non-SSH transports only.
pub fn baremetal_operator_transport() -> RemoteTransportKind {
    RemoteTransportKind::PhaseLinkGateway
}
