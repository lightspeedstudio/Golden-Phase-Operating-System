//! Minimal network status model for hosted and baremetal targets.

use crate::platform::{String, ToString};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkDriverState {
    Unavailable,
    Detected,
    Initialized,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkInterfaceStatus {
    pub interface_name: String,
    pub node_name: String,
    pub driver_name: String,
    pub driver_version: Option<u32>,
    pub mac_address: Option<[u8; 6]>,
    pub mmio_base: Option<usize>,
    pub tx_queue_index: Option<u16>,
    pub tx_queue_size: Option<u16>,
    pub rx_queue_index: Option<u16>,
    pub rx_queue_size: Option<u16>,
    pub tx_frames: u64,
    pub rx_frames: u64,
    pub last_tx_len: usize,
    pub last_rx_len: usize,
    pub last_rx_ethertype: Option<u16>,
    pub probe_summary: String,
    pub detected: bool,
    pub link_ready: bool,
    pub driver_state: NetworkDriverState,
}

impl Default for NetworkInterfaceStatus {
    fn default() -> Self {
        Self {
            interface_name: "phase0".into(),
            node_name: "local".into(),
            driver_name: "unavailable".into(),
            driver_version: None,
            mac_address: None,
            mmio_base: None,
            tx_queue_index: None,
            tx_queue_size: None,
            rx_queue_index: None,
            rx_queue_size: None,
            tx_frames: 0,
            rx_frames: 0,
            last_tx_len: 0,
            last_rx_len: 0,
            last_rx_ethertype: None,
            probe_summary: String::new(),
            detected: false,
            link_ready: false,
            driver_state: NetworkDriverState::Unavailable,
        }
    }
}

impl NetworkInterfaceStatus {
    pub fn render_status(&self) -> String {
        let mac = self
            .mac_address
            .map(format_mac)
            .unwrap_or_else(|| "unknown".into());
        let mmio = self
            .mmio_base
            .map(|base| format!("0x{base:08x}"))
            .unwrap_or_else(|| "none".into());
        let version = self
            .driver_version
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".into());
        let tx_queue = match (self.tx_queue_index, self.tx_queue_size) {
            (Some(index), Some(size)) => format!("{index}/{size}"),
            _ => "none".into(),
        };
        let rx_queue = match (self.rx_queue_index, self.rx_queue_size) {
            (Some(index), Some(size)) => format!("{index}/{size}"),
            _ => "none".into(),
        };
        let ethertype = self
            .last_rx_ethertype
            .map(|value| format!("0x{value:04x}"))
            .unwrap_or_else(|| "none".into());
        format!(
            "interface: {}\nnode: {}\ndriver: {}\nversion: {}\nstate: {}\nlink: {}\nmac: {}\nmmio: {}\ntx_queue: {}\nrx_queue: {}\ntx_frames: {}\nrx_frames: {}\nlast_tx_len: {}\nlast_rx_len: {}\nlast_rx_ethertype: {}\nprobe: {}\n",
            self.interface_name,
            self.node_name,
            self.driver_name,
            version,
            match self.driver_state {
                NetworkDriverState::Unavailable => "unavailable",
                NetworkDriverState::Detected => "detected",
                NetworkDriverState::Initialized => "initialized",
            },
            if self.link_ready { "ready" } else { "not initialized" },
            mac,
            mmio,
            tx_queue,
            rx_queue,
            self.tx_frames,
            self.rx_frames,
            self.last_tx_len,
            self.last_rx_len,
            ethertype,
            if self.probe_summary.is_empty() {
                "none"
            } else {
                &self.probe_summary
            },
        )
    }
}

pub fn format_mac(mac: [u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

/// Seed loopback NIC identity on hosted targets (after boot session root is sealed).
#[cfg(feature = "std")]
pub fn configure_hosted_identity(kernel: &mut crate::kernel::Kernel) {
    let bsr = kernel.authority.boot_session_root;
    let net = &mut kernel.network;
    net.interface_name = "phase0".into();
    net.node_name = "gpos-host".into();
    net.driver_name = "hosted-phaselink-loopback".into();
    net.driver_state = NetworkDriverState::Initialized;
    net.detected = true;
    net.link_ready = true;
    net.mac_address = Some([0x02, bsr[0], bsr[1], bsr[2], bsr[3], bsr[4]]);
    net.probe_summary =
        "PhaseLink loopback on host; run net status for detail; virtio-net on QEMU baremetal"
            .into();
}
