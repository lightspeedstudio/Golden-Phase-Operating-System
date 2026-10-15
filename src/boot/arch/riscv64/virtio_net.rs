//! Minimal VirtIO-net MMIO discovery for QEMU `virt`.

use crate::boot::memory_map::{VIRTIO_MMIO_BASE, VIRTIO_MMIO_SLOTS, VIRTIO_MMIO_STRIDE};
use crate::kernel::network::{NetworkDriverState, NetworkInterfaceStatus};
use crate::platform::String;
use core::mem::size_of;
use core::ptr::{read_volatile, write_volatile};

const VIRTIO_MAGIC: u32 = 0x7472_6976;
const VIRTIO_VERSION_LEGACY: u32 = 1;
const VIRTIO_VERSION_MODERN: u32 = 2;
const VIRTIO_DEVICE_ID_NET: u32 = 1;
const VIRTIO_VENDOR_ID: u32 = 0x554d_4551;
const VIRTIO_STATUS_ACKNOWLEDGE: u32 = 1;
const VIRTIO_STATUS_DRIVER: u32 = 2;
const VIRTIO_STATUS_DRIVER_OK: u32 = 4;
const VIRTIO_STATUS_FAILED: u32 = 0x80;

const VIRTIO_NET_QUEUE_RX: u32 = 0;
const VIRTIO_NET_QUEUE_TX: u32 = 1;
const VIRTIO_NET_QUEUE_SIZE: u16 = 8;
const VIRTIO_PAGE_SIZE: u32 = 4096;
const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

pub const PHASELINK_ETHERTYPE: u16 = 0x88B5;

const REG_MAGIC_VALUE: usize = 0x000;
const REG_VERSION: usize = 0x004;
const REG_DEVICE_ID: usize = 0x008;
const REG_VENDOR_ID: usize = 0x00c;
const REG_DEVICE_FEATURES: usize = 0x010;
const REG_DEVICE_FEATURES_SEL: usize = 0x014;
const REG_DRIVER_FEATURES: usize = 0x020;
const REG_DRIVER_FEATURES_SEL: usize = 0x024;
const REG_GUEST_PAGE_SIZE: usize = 0x028;
const REG_QUEUE_SEL: usize = 0x030;
const REG_QUEUE_NUM_MAX: usize = 0x034;
const REG_QUEUE_NUM: usize = 0x038;
const REG_QUEUE_ALIGN: usize = 0x03c;
const REG_QUEUE_PFN: usize = 0x040;
const REG_QUEUE_NOTIFY: usize = 0x050;
const REG_STATUS: usize = 0x070;
const REG_CONFIG_GENERATION: usize = 0x0fc;
const REG_CONFIG: usize = 0x100;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct VirtioNetHdr {
    flags: u8,
    gso_type: u8,
    hdr_len: u16,
    gso_size: u16,
    csum_start: u16,
    csum_offset: u16,
}

#[repr(C, align(4096))]
struct QueueMemory {
    desc: [VirtqDesc; VIRTIO_NET_QUEUE_SIZE as usize],
    avail_flags: u16,
    avail_idx: u16,
    avail_ring: [u16; VIRTIO_NET_QUEUE_SIZE as usize],
    avail_event: u16,
    padding: [u8; 4096
        - (size_of::<VirtqDesc>() * VIRTIO_NET_QUEUE_SIZE as usize
            + 6
            + 2 * VIRTIO_NET_QUEUE_SIZE as usize)],
    used_flags: u16,
    used_idx: u16,
    used_ring: [VirtqUsedElem; VIRTIO_NET_QUEUE_SIZE as usize],
    used_event: u16,
}

impl QueueMemory {
    const fn new() -> Self {
        Self {
            desc: [VirtqDesc {
                addr: 0,
                len: 0,
                flags: 0,
                next: 0,
            }; VIRTIO_NET_QUEUE_SIZE as usize],
            avail_flags: 0,
            avail_idx: 0,
            avail_ring: [0; VIRTIO_NET_QUEUE_SIZE as usize],
            avail_event: 0,
            padding: [0; 4096
                - (size_of::<VirtqDesc>() * VIRTIO_NET_QUEUE_SIZE as usize
                    + 6
                    + 2 * VIRTIO_NET_QUEUE_SIZE as usize)],
            used_flags: 0,
            used_idx: 0,
            used_ring: [VirtqUsedElem { id: 0, len: 0 }; VIRTIO_NET_QUEUE_SIZE as usize],
            used_event: 0,
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct PacketBuffer {
    header: VirtioNetHdr,
    frame: [u8; 1514],
}

impl PacketBuffer {
    const fn new() -> Self {
        Self {
            header: VirtioNetHdr {
                flags: 0,
                gso_type: 0,
                hdr_len: 0,
                gso_size: 0,
                csum_start: 0,
                csum_offset: 0,
            },
            frame: [0; 1514],
        }
    }
}

static mut RX_QUEUE_MEMORY: QueueMemory = QueueMemory::new();
static mut TX_QUEUE_MEMORY: QueueMemory = QueueMemory::new();
static mut RX_BUFFERS: [PacketBuffer; VIRTIO_NET_QUEUE_SIZE as usize] =
    [PacketBuffer::new(); VIRTIO_NET_QUEUE_SIZE as usize];
static mut TX_BUFFER: PacketBuffer = PacketBuffer::new();
static mut RX_USED_SEEN: u16 = 0;
static mut LAST_RX_COPY: [u8; 1514] = [0; 1514];
static mut LAST_RX_COPY_LEN: usize = 0;
static mut LAST_TX_COPY: [u8; 1514] = [0; 1514];
static mut LAST_TX_COPY_LEN: usize = 0;

pub fn probe() -> NetworkInterfaceStatus {
    let mut summary = String::new();
    for slot in 0..VIRTIO_MMIO_SLOTS {
        let base = VIRTIO_MMIO_BASE + slot * VIRTIO_MMIO_STRIDE;
        let magic = read32(base + REG_MAGIC_VALUE);
        let version = read32(base + REG_VERSION);
        let device_id = read32(base + REG_DEVICE_ID);
        let vendor_id = read32(base + REG_VENDOR_ID);
        if !summary.is_empty() {
            summary.push_str(" | ");
        }
        summary.push_str(&format!(
            "slot{}@0x{base:08x}:m=0x{magic:08x},v={},d={},vendor=0x{vendor_id:08x}",
            slot + 1,
            version,
            device_id
        ));
        if magic != VIRTIO_MAGIC
            || (version != VIRTIO_VERSION_LEGACY && version != VIRTIO_VERSION_MODERN)
        {
            continue;
        }
        if vendor_id != VIRTIO_VENDOR_ID || device_id != VIRTIO_DEVICE_ID_NET {
            continue;
        }
        let mut mac = [0u8; 6];
        for (i, b) in mac.iter_mut().enumerate() {
            *b = read8(base + REG_CONFIG + i);
        }
        let _status = read32(base + REG_STATUS);
        let _cfg_gen = read8(base + REG_CONFIG_GENERATION);
        return NetworkInterfaceStatus {
            interface_name: "phase0".into(),
            node_name: "baremetal".into(),
            driver_name: "virtio-net".into(),
            driver_version: Some(version),
            mac_address: Some(mac),
            mmio_base: Some(base),
            tx_queue_index: Some(VIRTIO_NET_QUEUE_TX as u16),
            tx_queue_size: None,
            rx_queue_index: Some(VIRTIO_NET_QUEUE_RX as u16),
            rx_queue_size: None,
            tx_frames: 0,
            rx_frames: 0,
            last_tx_len: 0,
            last_rx_len: 0,
            last_rx_ethertype: None,
            probe_summary: summary,
            detected: true,
            link_ready: false,
            driver_state: NetworkDriverState::Detected,
        };
    }
    NetworkInterfaceStatus {
        probe_summary: summary,
        ..NetworkInterfaceStatus::default()
    }
}

pub fn init(status: &mut NetworkInterfaceStatus) -> Result<(), &'static str> {
    let base = status.mmio_base.ok_or("virtio-net: no mmio base")?;
    let version = status.driver_version.ok_or("virtio-net: no version")?;
    if version != VIRTIO_VERSION_LEGACY && version != VIRTIO_VERSION_MODERN {
        return Err("virtio-net: unsupported mmio version");
    }

    write32(base + REG_STATUS, 0);
    write32(base + REG_STATUS, VIRTIO_STATUS_ACKNOWLEDGE);
    write32(
        base + REG_STATUS,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER,
    );

    write32(base + REG_DEVICE_FEATURES_SEL, 0);
    let _device_features_lo = read32(base + REG_DEVICE_FEATURES);
    write32(base + REG_DRIVER_FEATURES_SEL, 0);
    write32(base + REG_DRIVER_FEATURES, 0);
    write32(base + REG_DRIVER_FEATURES_SEL, 1);
    write32(base + REG_DRIVER_FEATURES, 0);

    if version == VIRTIO_VERSION_LEGACY {
        write32(base + REG_GUEST_PAGE_SIZE, VIRTIO_PAGE_SIZE);
    }

    setup_rx_queue(base)?;
    setup_tx_queue(base)?;

    write32(
        base + REG_STATUS,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK,
    );

    status.rx_queue_size = Some(VIRTIO_NET_QUEUE_SIZE);
    status.tx_queue_size = Some(VIRTIO_NET_QUEUE_SIZE);
    status.link_ready = true;
    status.driver_state = NetworkDriverState::Initialized;
    status.probe_summary.push_str(" | rxq=ready | txq=ready");
    Ok(())
}

pub fn transmit_test_frame(status: &mut NetworkInterfaceStatus) -> Result<usize, &'static str> {
    let src_mac = status.mac_address.ok_or("virtio-net: no mac address")?;
    let payload = b"KairOS VirtIO TX";
    let mut eth = [0u8; 1514];
    eth[0..6].copy_from_slice(&[0xff; 6]);
    eth[6..12].copy_from_slice(&src_mac);
    eth[12..14].copy_from_slice(&PHASELINK_ETHERTYPE.to_be_bytes());
    eth[14..14 + payload.len()].copy_from_slice(payload);
    transmit_ethernet(status, &eth[..14 + payload.len()])
}

pub fn transmit_ethernet(
    status: &mut NetworkInterfaceStatus,
    eth_frame: &[u8],
) -> Result<usize, &'static str> {
    let base = status.mmio_base.ok_or("virtio-net: no mmio base")?;
    if status.driver_state != NetworkDriverState::Initialized {
        return Err("virtio-net: driver not initialized");
    }
    if eth_frame.is_empty() || eth_frame.len() > 1514 {
        return Err("virtio-net: invalid ethernet frame");
    }

    unsafe {
        LAST_TX_COPY_LEN = eth_frame.len();
        LAST_TX_COPY[..eth_frame.len()].copy_from_slice(eth_frame);
        TX_BUFFER = PacketBuffer::new();
        TX_BUFFER.frame[..eth_frame.len()].copy_from_slice(eth_frame);

        let header_addr = (&raw const TX_BUFFER.header) as u64;
        let frame_addr = (&raw const TX_BUFFER.frame) as u64;
        TX_QUEUE_MEMORY.desc[0] = VirtqDesc {
            addr: header_addr,
            len: size_of::<VirtioNetHdr>() as u32,
            flags: VIRTQ_DESC_F_NEXT,
            next: 1,
        };
        TX_QUEUE_MEMORY.desc[1] = VirtqDesc {
            addr: frame_addr,
            len: eth_frame.len() as u32,
            flags: 0,
            next: 0,
        };
        let slot = (TX_QUEUE_MEMORY.avail_idx as usize) % VIRTIO_NET_QUEUE_SIZE as usize;
        TX_QUEUE_MEMORY.avail_ring[slot] = 0;
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        TX_QUEUE_MEMORY.avail_idx = TX_QUEUE_MEMORY.avail_idx.wrapping_add(1);
        write32(base + REG_QUEUE_NOTIFY, VIRTIO_NET_QUEUE_TX);
    }

    status.tx_frames = status.tx_frames.saturating_add(1);
    status.last_tx_len = eth_frame.len();
    Ok(eth_frame.len())
}

pub fn replay_last_ethernet(status: &mut NetworkInterfaceStatus) -> Result<usize, &'static str> {
    let len = unsafe { LAST_TX_COPY_LEN };
    if len == 0 {
        return Err("virtio-net: no frame to replay");
    }
    let mut frame = [0u8; 1514];
    unsafe {
        frame[..len].copy_from_slice(&LAST_TX_COPY[..len]);
    }
    transmit_ethernet(status, &frame[..len])
}

pub fn receive_frame(status: &mut NetworkInterfaceStatus) -> Result<usize, &'static str> {
    if status.driver_state != NetworkDriverState::Initialized {
        return Err("virtio-net: driver not initialized");
    }

    unsafe {
        let used_idx = RX_QUEUE_MEMORY.used_idx;
        if RX_USED_SEEN == used_idx {
            return Err("virtio-net: no packet received");
        }

        let slot = (RX_USED_SEEN as usize) % VIRTIO_NET_QUEUE_SIZE as usize;
        let elem = RX_QUEUE_MEMORY.used_ring[slot];
        let desc_id = elem.id as usize;
        if desc_id >= VIRTIO_NET_QUEUE_SIZE as usize {
            return Err("virtio-net: invalid rx descriptor");
        }

        let total_len = elem.len as usize;
        let header_len = size_of::<VirtioNetHdr>();
        if total_len <= header_len {
            return Err("virtio-net: rx packet too short");
        }
        let frame_len = total_len - header_len;
        if frame_len < 14 {
            return Err("virtio-net: rx frame too short");
        }

        let ethertype =
            u16::from_be_bytes([RX_BUFFERS[desc_id].frame[12], RX_BUFFERS[desc_id].frame[13]]);

        LAST_RX_COPY_LEN = frame_len;
        LAST_RX_COPY[..frame_len].copy_from_slice(&RX_BUFFERS[desc_id].frame[..frame_len]);

        status.rx_frames = status.rx_frames.saturating_add(1);
        status.last_rx_len = frame_len;
        status.last_rx_ethertype = Some(ethertype);

        RX_BUFFERS[desc_id] = PacketBuffer::new();
        let buf_addr = (&raw const RX_BUFFERS[desc_id]) as u64;
        RX_QUEUE_MEMORY.desc[desc_id] = VirtqDesc {
            addr: buf_addr,
            len: size_of::<PacketBuffer>() as u32,
            flags: VIRTQ_DESC_F_WRITE,
            next: 0,
        };

        let avail_slot = (RX_QUEUE_MEMORY.avail_idx as usize) % VIRTIO_NET_QUEUE_SIZE as usize;
        RX_QUEUE_MEMORY.avail_ring[avail_slot] = desc_id as u16;
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        RX_QUEUE_MEMORY.avail_idx = RX_QUEUE_MEMORY.avail_idx.wrapping_add(1);

        RX_USED_SEEN = RX_USED_SEEN.wrapping_add(1);
        Ok(frame_len)
    }
}

pub fn last_rx_ethernet() -> &'static [u8] {
    unsafe {
        let len = LAST_RX_COPY_LEN;
        &LAST_RX_COPY[..len]
    }
}

fn setup_rx_queue(base: usize) -> Result<(), &'static str> {
    let queue_max = select_queue(base, VIRTIO_NET_QUEUE_RX);
    if queue_max < VIRTIO_NET_QUEUE_SIZE as u32 {
        write32(base + REG_STATUS, VIRTIO_STATUS_FAILED);
        return Err("virtio-net: rx queue too small");
    }

    unsafe {
        RX_QUEUE_MEMORY = QueueMemory::new();
        RX_USED_SEEN = 0;
        #[allow(clippy::needless_range_loop)]
        for i in 0..VIRTIO_NET_QUEUE_SIZE as usize {
            RX_BUFFERS[i] = PacketBuffer::new();
            let buf_addr = (&raw const RX_BUFFERS[i]) as u64;
            RX_QUEUE_MEMORY.desc[i] = VirtqDesc {
                addr: buf_addr,
                len: size_of::<PacketBuffer>() as u32,
                flags: VIRTQ_DESC_F_WRITE,
                next: 0,
            };
            RX_QUEUE_MEMORY.avail_ring[i] = i as u16;
        }
        RX_QUEUE_MEMORY.avail_idx = VIRTIO_NET_QUEUE_SIZE;
        let queue_addr = (&raw const RX_QUEUE_MEMORY) as usize;
        write32(base + REG_QUEUE_NUM, VIRTIO_NET_QUEUE_SIZE as u32);
        write32(base + REG_QUEUE_ALIGN, VIRTIO_PAGE_SIZE);
        write32(
            base + REG_QUEUE_PFN,
            (queue_addr / VIRTIO_PAGE_SIZE as usize) as u32,
        );
    }
    Ok(())
}

fn setup_tx_queue(base: usize) -> Result<(), &'static str> {
    let queue_max = select_queue(base, VIRTIO_NET_QUEUE_TX);
    if queue_max < VIRTIO_NET_QUEUE_SIZE as u32 {
        write32(base + REG_STATUS, VIRTIO_STATUS_FAILED);
        return Err("virtio-net: tx queue too small");
    }

    unsafe {
        TX_QUEUE_MEMORY = QueueMemory::new();
        TX_BUFFER = PacketBuffer::new();
        let queue_addr = (&raw const TX_QUEUE_MEMORY) as usize;
        write32(base + REG_QUEUE_NUM, VIRTIO_NET_QUEUE_SIZE as u32);
        write32(base + REG_QUEUE_ALIGN, VIRTIO_PAGE_SIZE);
        write32(
            base + REG_QUEUE_PFN,
            (queue_addr / VIRTIO_PAGE_SIZE as usize) as u32,
        );
    }
    Ok(())
}

fn select_queue(base: usize, queue_index: u32) -> u32 {
    write32(base + REG_QUEUE_SEL, queue_index);
    read32(base + REG_QUEUE_NUM_MAX)
}

#[inline]
fn read32(addr: usize) -> u32 {
    unsafe { read_volatile(addr as *const u32) }
}

#[inline]
fn read8(addr: usize) -> u8 {
    unsafe { read_volatile(addr as *const u8) }
}

#[inline]
fn write32(addr: usize, value: u32) {
    unsafe { write_volatile(addr as *mut u32, value) }
}
