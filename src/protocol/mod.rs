pub mod checksum;
pub mod encoder;
pub mod hybrid_frame;
pub mod pnw1_wire;
pub mod topology;

pub use checksum::{
    accumulate_compose, branch_swap_y, bytes_stream_checksum, chi_normalized_pair, gpl_checksum,
    gpl_verify, phase_stream_checksum, state_sheet, verify_bytes_checksum,
};
pub use hybrid_frame::{
    crc16_ccitt, detect, diagnose, quantize_chi, topology_parity, DetectionVerdict, Diagnosis,
    ErrorClass, FrameError, HybridFrame, CHI_SCALE,
};
pub use topology::{
    reflection_count, scan_non_overlapping_triples, scan_sliding_triples, StreamTopologyEvent,
};
