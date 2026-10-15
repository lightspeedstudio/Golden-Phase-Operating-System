//! AI job trace and statechain integration.

use crate::core::gphase_core::PhaseState;
use crate::kernel::ai::types::{
    AiHook, AiJob, AiTraceRecord, AI_STATECHAIN_OBJECT_ID, MAX_TRACE_RECORDS_PER_JOB,
};
use crate::kernel::hash::{tagged_hash, StateHash};
use crate::kernel::state_proof::ObjectKind;
use crate::kernel::state_root::SubsystemKind;
use crate::kernel::Kernel;
use crate::platform::String;

const TRACE_TAG: &str = "KAIROS_AI_TRACE_V1";

pub fn short_hash(hash: &StateHash) -> String {
    format!(
        "{:02X}{:02X}{:02X}{:02X}",
        hash[0], hash[1], hash[2], hash[3]
    )
}

pub fn record_event(
    kernel: &mut Kernel,
    job: &mut AiJob,
    hook: AiHook,
    phase_after: PhaseState,
    tool_name: &str,
    auth_result: &str,
    error_code: u16,
) {
    record_event_on_job(
        kernel,
        job,
        hook,
        phase_after,
        tool_name,
        auth_result,
        error_code,
    );
}

pub fn record_event_on_job(
    kernel: &mut Kernel,
    job: &mut AiJob,
    hook: AiHook,
    phase_after: PhaseState,
    tool_name: &str,
    auth_result: &str,
    error_code: u16,
) {
    let phase_before = job.phase;
    let tick = kernel.clock.snapshot().chronos_tick;
    let event = hook.label();
    let record_hash = tagged_hash(
        TRACE_TAG,
        &[
            &tick.to_be_bytes(),
            &job.job_id.to_be_bytes(),
            &job.process_id.to_be_bytes(),
            &[phase_before.as_u8(), phase_after.as_u8()],
            event.as_bytes(),
            &job.input_hash,
            &job.output_hash,
            tool_name.as_bytes(),
            auth_result.as_bytes(),
            &error_code.to_be_bytes(),
            &job.parent_chain_hash,
        ],
    );
    let trace = AiTraceRecord {
        tick,
        job_id: job.job_id,
        process_id: job.process_id,
        phase_before,
        phase_after,
        event: event.into(),
        server: job.server_address.clone(),
        input_hash: job.input_hash,
        output_hash: job.output_hash,
        tool_name: tool_name.into(),
        authorization_result: auth_result.into(),
        error_code,
        parent_chain_hash: job.parent_chain_hash,
        record_hash,
    };
    if phase_after != phase_before {
        match kernel.scheduler.get_process_mut(job.process_id) {
            Ok(p) => {
                if !p.transition(phase_after) {
                    return;
                }
            }
            Err(_) => return,
        }
    }
    if job.trace.len() >= MAX_TRACE_RECORDS_PER_JOB {
        job.trace.remove(0);
    }
    job.trace.push(trace);
    job.parent_chain_hash = record_hash;
    job.phase = phase_after;
    let _ = kernel.statechain.record_transition(
        ObjectKind::Driver,
        AI_STATECHAIN_OBJECT_ID,
        SubsystemKind::Driver,
        phase_before,
        phase_after,
        event,
        None,
        &job.job_id.to_be_bytes(),
    );
}

pub fn format_trace(job: &AiJob) -> String {
    let mut out = String::new();
    for rec in &job.trace {
        let line = match rec.event.as_str() {
            "AI_JOB_CREATED" => format!("[AI:{}] Phi1 created", job.job_id),
            "AI_BEFORE_REQUEST" => format!("[AI:{}] request validated", job.job_id),
            "AI_REQUEST_SENT" => format!("[AI:{}] Phi2 inference started", job.job_id),
            "AI_RESPONSE_RECEIVED" => format!("[AI:{}] response received", job.job_id),
            "AI_BEFORE_TOOL" => format!("[AI:{}] tool gate Phi3: {}", job.job_id, rec.tool_name),
            "AI_TOOL_ALLOWED" => format!("[AI:{}] tool allowed: {}", job.job_id, rec.tool_name),
            "AI_TOOL_DENIED" => format!("[AI:{}] tool denied: {}", job.job_id, rec.tool_name),
            "AI_AFTER_TOOL" => format!("[AI:{}] tool completed: {}", job.job_id, rec.tool_name),
            "AI_JOB_COMPLETED" => format!("[AI:{}] Phi0 completed", job.job_id),
            "AI_JOB_FAILED" => format!("[AI:{}] Phi0 failed ({})", job.job_id, rec.error_code),
            "AI_JOB_CANCELLED" => format!("[AI:{}] Phi0 cancelled", job.job_id),
            other => format!("[AI:{}] {}", job.job_id, other),
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}
