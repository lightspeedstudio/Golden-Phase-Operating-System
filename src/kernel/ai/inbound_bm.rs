//! Bounded PhaseLink RX poll for inbound AI frames (EtherType 0x88B5).

use crate::board::zynq7000::gem;
use crate::board::zynq7000::phaselink_eth;
use crate::kernel::ai::control::{self, OP_ACT, OP_CANCEL, OP_JOB};
use crate::kernel::net_ip::{enable_gtc, now_ms};
use crate::kernel::phaselink::decoder::PhaseLinkDecoder;
use crate::kernel::phaselink::{accept_or_quarantine, FrameValidationResult};
use crate::kernel::Kernel;
use crate::platform::String;

const PHASELINK_ETHERTYPE: u16 = 0x88B5;

/// Poll GEM0 for `seconds`. Accepted `AI_*` frames run the control layer.
/// Armed from UART (`ai serve`); not a background daemon.
pub fn serve(kernel: &mut Kernel, seconds: u32) -> String {
    control::ensure_node(kernel);
    if gem::wire_init().is_err() {
        return "ai serve: GEM0 wire init failed (PHY link down?)\n".into();
    }
    enable_gtc();
    let mut out = String::from("ai serve: PhaseLink 0x88B5 poll\n");
    let deadline = now_ms().saturating_add((seconds as u64).saturating_mul(1000));
    let mut accepted = 0u32;
    let mut quarantined = 0u32;
    let mut ai_ok = 0u32;
    while now_ms() < deadline {
        match phaselink_eth::recv_payload() {
            Ok(payload) => {
                let frame = match PhaseLinkDecoder::decode_frame(&payload) {
                    Ok(f) => f,
                    Err(_) => {
                        out.push_str("ai serve: decode failed\n");
                        continue;
                    }
                };
                match accept_or_quarantine(kernel, &frame) {
                    FrameValidationResult::Accepted => {
                        accepted += 1;
                        if matches!(
                            frame.operation.as_str(),
                            OP_JOB | OP_ACT | OP_CANCEL
                        ) {
                            match control::dispatch_frame(kernel, &frame) {
                                Ok(msg) => {
                                    ai_ok += 1;
                                    out.push_str(&msg);
                                }
                                Err(e) => {
                                    out.push_str(&format!("ai serve: {}\n", e.reason()));
                                }
                            }
                        }
                    }
                    FrameValidationResult::Quarantined(reason) => {
                        quarantined += 1;
                        out.push_str(&format!("ai serve: quarantine {}\n", reason.label()));
                    }
                    FrameValidationResult::Rejected(_) => {
                        out.push_str("ai serve: rejected\n");
                    }
                }
            }
            Err(_) => {
                for _ in 0..8_000 {
                    core::hint::spin_loop();
                }
            }
        }
    }
    out.push_str(&format!(
        "ai serve: done accepted={accepted} quarantined={quarantined} ai_ok={ai_ok} ethertype=0x{PHASELINK_ETHERTYPE:04X}\n"
    ));
    out
}
