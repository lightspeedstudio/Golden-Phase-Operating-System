//! Phase-native AI control: each job is a kernel process under phi.

use crate::core::gphase_core::PhaseState;
use crate::kernel::ai::error::AiError;
use crate::kernel::ai::permissions::{allow_tool, deny_tool, format_tools_table};
use crate::kernel::ai::tools::{authorize_tool_gate, execute_tool};
use crate::kernel::ai::trace::{format_trace, record_event, short_hash};
use crate::kernel::ai::types::{
    AiHook, AiJob, AiJobStatus, MAX_ACTIVE_JOBS, MAX_PROMPT_BYTES,
};
use crate::kernel::hash::{tagged_hash, StateHash};
use crate::kernel::phaselink::frame::{compute_frame_hash, serialize_frame_without_hash};
use crate::kernel::phaselink::proof::build_output_proof;
use crate::kernel::phaselink::types::{
    Hash, NodeId, PayloadEncoding, PhaseLinkFlags, PhaseLinkFrame, PhaseLinkObjectKind,
    PHASELINK_STATECHAIN_OBJECT_ID, PHASELINK_VERSION,
};
use crate::kernel::phaselink::{accept_or_quarantine, FrameValidationResult};
use crate::kernel::process::VALID_TRANSITIONS;
use crate::kernel::state_proof::{hash_payload, ObjectKind};
use crate::kernel::Kernel;
use crate::platform::{String, Vec};

pub const OP_JOB: &str = "AI_JOB";
pub const OP_ACT: &str = "AI_ACT";
pub const OP_CANCEL: &str = "AI_CANCEL";
pub const LOCAL_NODE_NAME: &str = "board";
pub const PEER_NODE_NAME: &str = "ai";

pub fn ensure_node(kernel: &mut Kernel) {
    if kernel.phaselink.local_node == [0u8; 32] {
        let equation_root = tagged_hash("KAIROS_BAREMETAL_EQUATION_V1", &[b"cluster"]);
        let local_bsr = tagged_hash(
            "KAIROS_BAREMETAL_BOOT_SESSION_V1",
            &[LOCAL_NODE_NAME.as_bytes()],
        );
        kernel
            .phaselink
            .init(equation_root, local_bsr, LOCAL_NODE_NAME);
    }
    kernel
        .phaselink
        .trusted_nodes
        .insert(peer_node_id(&kernel.phaselink.equation_root));
}

pub fn peer_node_id(equation_root: &Hash) -> NodeId {
    let peer_bsr = tagged_hash(
        "KAIROS_BAREMETAL_BOOT_SESSION_V1",
        &[PEER_NODE_NAME.as_bytes()],
    );
    tagged_hash(
        "KAIROS_PHASELINK_NODE_V1",
        &[PEER_NODE_NAME.as_bytes(), equation_root, &peer_bsr],
    )
}

pub fn legal_transition(from: PhaseState, to: PhaseState) -> bool {
    if from == to {
        return true;
    }
    VALID_TRANSITIONS
        .iter()
        .find(|(s, _)| *s == from)
        .map(|(_, tos)| tos.contains(&to))
        .unwrap_or(false)
}

pub fn status(kernel: &Kernel) -> String {
    let (active, queued, completed, failed, cancelled) = job_counts(&kernel.ai);
    format!(
        "AI CONTROL (PhaseLink inbound)\n\n\
inbound:                EtherType 0x88B5 AI_JOB/AI_ACT/AI_CANCEL\n\
active jobs:            {active}\n\
queued jobs:            {queued}\n\
completed jobs:         {completed}\n\
failed jobs:            {failed}\n\
cancelled jobs:         {cancelled}\n\
last error:             {}\n",
        kernel.ai.last_error.as_deref().unwrap_or("-"),
    )
}

pub fn jobs_table(kernel: &Kernel) -> String {
    let mut out = String::from("JOB   PID   PHASE   STATUS      AGE\n");
    let now = kernel.clock.snapshot().chronos_tick;
    for job in &kernel.ai.jobs {
        let age = now.saturating_sub(job.created_tick);
        out.push_str(&format!(
            "{:<5} {:<5} {:<7} {:<11} {}\n",
            job.job_id,
            job.process_id,
            phase_label(job.phase),
            job.status.label(),
            age
        ));
    }
    out
}

pub fn show_job(kernel: &Kernel, job_id: u64) -> Result<String, AiError> {
    let job = find_job(&kernel.ai.jobs, job_id).ok_or(AiError::JobNotFound)?;
    Ok(format!(
        "AI JOB {job_id}\n\n\
Status: {}\n\
Phase: {}\n\
PID: {}\n\
Input hash: {}\n\
Output hash: {}\n\
Parent chain: {}\n\
Error: {}\n",
        job.status.label(),
        phase_label(job.phase),
        job.process_id,
        short_hash(&job.input_hash),
        short_hash(&job.output_hash),
        hex32(&job.parent_chain_hash),
        job.error_message
    ))
}

pub fn trace_job(kernel: &Kernel, job_id: u64) -> Result<String, AiError> {
    let job = find_job(&kernel.ai.jobs, job_id).ok_or(AiError::JobNotFound)?;
    Ok(format_trace(job))
}

pub fn tools_table(kernel: &Kernel) -> String {
    format_tools_table(&kernel.ai)
}

pub fn allow(kernel: &mut Kernel, tool: &str) -> Result<String, AiError> {
    allow_tool(&mut kernel.ai, tool)
}

pub fn deny(kernel: &mut Kernel, tool: &str) -> Result<String, AiError> {
    deny_tool(&mut kernel.ai, tool)
}

/// Accept a PhaseLink frame (must already have passed accept_or_quarantine)
/// and apply AI_JOB / AI_ACT / AI_CANCEL.
pub fn dispatch_frame(kernel: &mut Kernel, frame: &PhaseLinkFrame) -> Result<String, AiError> {
    let msg = match frame.operation.as_str() {
        OP_JOB => accept_job(kernel, frame)?,
        OP_ACT => apply_act(kernel, frame)?,
        OP_CANCEL => cancel_from_frame(kernel, frame)?,
        other => return Ok(format!("phaselink op {other} (not AI)")),
    };
    Ok(format!(
        "{}seq={} frame={}\n",
        if msg.ends_with('\n') {
            msg
        } else {
            format!("{msg}\n")
        },
        frame.sequence_number,
        hex32(&frame.frame_hash)
    ))
}

pub fn ingest_frame(kernel: &mut Kernel, frame: &PhaseLinkFrame) -> Result<String, AiError> {
    match accept_or_quarantine(kernel, frame) {
        FrameValidationResult::Accepted => dispatch_frame(kernel, frame),
        FrameValidationResult::Quarantined(reason) => {
            kernel.ai.last_error = Some(reason.label().into());
            Err(AiError::InvalidPhase)
        }
        FrameValidationResult::Rejected(_) => Err(AiError::MalformedResponse),
    }
}

pub fn accept_job(kernel: &mut Kernel, frame: &PhaseLinkFrame) -> Result<String, AiError> {
    if frame.previous_phase != PhaseState::Phi0 || frame.next_phase != PhaseState::Phi1 {
        return Err(AiError::InvalidPhase);
    }
    if !legal_transition(PhaseState::Phi0, PhaseState::Phi1) {
        return Err(AiError::InvalidPhase);
    }
    if frame.payload.len() > MAX_PROMPT_BYTES {
        return Err(AiError::PromptTooLarge);
    }
    let active = kernel
        .ai
        .jobs
        .iter()
        .filter(|j| matches!(j.status, AiJobStatus::Queued | AiJobStatus::Running))
        .count();
    if active >= MAX_ACTIVE_JOBS {
        return Err(AiError::JobTableFull);
    }
    let job_id = kernel.ai.next_job_id;
    kernel.ai.next_job_id += 1;
    let name = format!("ai-job-{job_id}");
    let pid = kernel.spawn_process(&name);
    let tick = kernel.clock.snapshot().chronos_tick;
    let input_hash = tagged_hash("KAIROS_AI_INPUT_V1", &[&frame.payload]);
    let mut job = AiJob {
        job_id,
        process_id: pid,
        parent_job_id: None,
        server_address: "phaselink:0x88B5".into(),
        model_name: "inbound".into(),
        status: AiJobStatus::Queued,
        phase: PhaseState::Phi0,
        created_tick: tick,
        started_tick: None,
        completed_tick: None,
        deadline_tick: tick,
        input_hash,
        output_hash: [0u8; 32],
        error_code: 0,
        error_message: String::new(),
        requested_tools: Vec::new(),
        approved_tools: kernel.ai.allowed_tool_names(),
        denied_tools: Vec::new(),
        cancel_requested: false,
        prompt: String::new(),
        response: String::new(),
        store_content: false,
        trace: Vec::new(),
        parent_chain_hash: [0u8; 32],
    };
    record_event(
        kernel,
        &mut job,
        AiHook::JobCreated,
        PhaseState::Phi1,
        "",
        "created",
        0,
    );
    if job.phase != PhaseState::Phi1 {
        return Err(AiError::InvalidPhase);
    }
    record_event(
        kernel,
        &mut job,
        AiHook::BeforeRequest,
        PhaseState::Phi2,
        "",
        "running",
        0,
    );
    if job.phase != PhaseState::Phi2 {
        return Err(AiError::InvalidPhase);
    }
    job.status = AiJobStatus::Running;
    job.started_tick = Some(kernel.clock.snapshot().chronos_tick);
    let pid = job.process_id;
    let parent = job.parent_chain_hash;
    kernel.ai.jobs.push(job);
    Ok(format!(
        "AI_JOB accepted job={job_id} pid={pid} phase=Phi2 in={} parent={}\n",
        short_hash(&input_hash),
        hex32(&parent)
    ))
}

pub fn apply_act(kernel: &mut Kernel, frame: &PhaseLinkFrame) -> Result<String, AiError> {
    let (job_id, tool) = parse_act_object(&frame.object_id)?;
    let idx = kernel
        .ai
        .jobs
        .iter()
        .position(|j| j.job_id == job_id)
        .ok_or(AiError::JobNotFound)?;
    let mut job = kernel.ai.jobs.remove(idx);
    if job.cancel_requested || job.status == AiJobStatus::Cancelled {
        kernel.ai.jobs.push(job);
        return Err(AiError::Cancelled);
    }
    if job.status != AiJobStatus::Running {
        kernel.ai.jobs.push(job);
        return Err(AiError::JobNotFound);
    }
    let proc_phase = kernel
        .scheduler
        .get_process(job.process_id)
        .map(|p| p.state)
        .unwrap_or(job.phase);
    if proc_phase != frame.previous_phase || job.phase != frame.previous_phase {
        kernel.ai.jobs.push(job);
        return Err(AiError::InvalidPhase);
    }
    if !legal_transition(frame.previous_phase, frame.next_phase) {
        kernel.ai.jobs.push(job);
        return Err(AiError::InvalidPhase);
    }
    if frame.next_phase != PhaseState::Phi3 {
        kernel.ai.jobs.push(job);
        return Err(AiError::InvalidPhase);
    }
    let (parent, args) = split_parent_payload(&frame.payload);
    if parent != job.parent_chain_hash {
        kernel.ai.jobs.push(job);
        return Err(AiError::InvalidPhase);
    }
    job.requested_tools.push(tool.clone());
    record_event(
        kernel,
        &mut job,
        AiHook::BeforeTool,
        PhaseState::Phi3,
        &tool,
        "pending",
        0,
    );
    if job.phase != PhaseState::Phi3 {
        kernel.ai.jobs.push(job);
        return Err(AiError::InvalidPhase);
    }
    if let Err(e) = authorize_tool_gate(job.phase) {
        kernel.ai.jobs.push(job);
        return Err(e);
    }
    let args_s = core::str::from_utf8(args).unwrap_or("");
    match execute_tool(kernel, &kernel.ai, &tool, args_s) {
        Ok(out) => {
            job.output_hash = tagged_hash("KAIROS_AI_OUTPUT_V1", &[out.as_bytes()]);
            job.response = out.clone();
            job.approved_tools.push(tool.clone());
            record_event(
                kernel,
                &mut job,
                AiHook::ToolAllowed,
                PhaseState::Phi3,
                &tool,
                "allowed",
                0,
            );
            record_event(
                kernel,
                &mut job,
                AiHook::AfterTool,
                PhaseState::Phi1,
                &tool,
                "ok",
                0,
            );
            let msg = format!(
                "AI_ACT allowed job={job_id} tool={tool} phase=Phi1 out={}\n{out}\n",
                short_hash(&job.output_hash)
            );
            kernel.ai.jobs.push(job);
            Ok(msg)
        }
        Err(AiError::ToolDenied) | Err(AiError::ToolNotFound) => {
            job.denied_tools.push(tool.clone());
            record_event(
                kernel,
                &mut job,
                AiHook::ToolDenied,
                PhaseState::Phi1,
                &tool,
                "denied",
                AiError::ToolDenied.code(),
            );
            kernel.ai.last_error = Some(AiError::ToolDenied.reason());
            kernel.ai.jobs.push(job);
            Err(AiError::ToolDenied)
        }
        Err(err) => {
            kernel.ai.jobs.push(job);
            Err(err)
        }
    }
}

pub fn cancel_job(kernel: &mut Kernel, job_id: u64) -> Result<String, AiError> {
    let idx = kernel
        .ai
        .jobs
        .iter()
        .position(|j| j.job_id == job_id)
        .ok_or(AiError::JobNotFound)?;
    let mut job = kernel.ai.jobs.remove(idx);
    if matches!(
        job.status,
        AiJobStatus::Completed | AiJobStatus::Failed | AiJobStatus::Cancelled
    ) {
        kernel.ai.jobs.push(job);
        return Err(AiError::JobNotFound);
    }
    isolate_to_phi0(kernel, &mut job);
    job.cancel_requested = true;
    job.status = AiJobStatus::Cancelled;
    job.error_code = AiError::Cancelled.code();
    job.error_message = AiError::Cancelled.reason();
    let err_code = job.error_code;
    record_event(
        kernel,
        &mut job,
        AiHook::JobCancelled,
        PhaseState::Phi0,
        "",
        "cancelled",
        err_code,
    );
    kernel.ai.jobs.push(job);
    Ok(format!("AI job {job_id} cancelled\n"))
}

fn cancel_from_frame(kernel: &mut Kernel, frame: &PhaseLinkFrame) -> Result<String, AiError> {
    let job_id: u64 = frame
        .object_id
        .parse()
        .map_err(|_| AiError::MalformedResponse)?;
    if !legal_transition(frame.previous_phase, frame.next_phase)
        || frame.next_phase != PhaseState::Phi0
    {
        return Err(AiError::InvalidPhase);
    }
    cancel_job(kernel, job_id)
}

fn isolate_to_phi0(kernel: &mut Kernel, job: &mut AiJob) {
    let pid = job.process_id;
    let state = kernel
        .scheduler
        .get_process(pid)
        .map(|p| p.state)
        .unwrap_or(job.phase);
    match state {
        PhaseState::Phi0 => {}
        PhaseState::Phi3 => {
            let _ = kernel.scheduler.exit(pid);
        }
        PhaseState::Phi2 => {
            let _ = kernel.scheduler.preempt(pid);
            let _ = kernel.scheduler.exit(pid);
        }
        PhaseState::Phi1 => {
            let _ = kernel.scheduler.exit(pid);
        }
    }
    if let Ok(p) = kernel.scheduler.get_process(pid) {
        job.phase = p.state;
    }
}

/// Build a locally sourced AI PhaseLink frame for loopback / selftest.
pub fn build_ai_frame(
    kernel: &mut Kernel,
    operation: &str,
    previous: PhaseState,
    next: PhaseState,
    object_id: &str,
    payload: &[u8],
) -> Result<PhaseLinkFrame, AiError> {
    ensure_node(kernel);
    if !legal_transition(previous, next) && previous != next {
        // Still build: validator will quarantine illegal hops. Caller may want that.
    }
    let source = kernel.phaselink.local_node;
    let (sequence, previous_frame_hash) =
        match kernel.phaselink.receiver.remotes.get(&source) {
            Some(r) => (r.last_sequence_number.saturating_add(1), r.last_frame_hash),
            None => (1u64, [0u8; 32]),
        };
    let snapshot = kernel.clock.snapshot();
    let payload_hash = hash_payload(payload);
    let dest = peer_node_id(&kernel.phaselink.equation_root);
    let proof = build_output_proof(
        kernel.phaselink.equation_root,
        kernel.phaselink.boot_session_root,
        source,
        dest,
        ObjectKind::Driver,
        PHASELINK_STATECHAIN_OBJECT_ID,
        operation,
        previous,
        next,
        sequence,
        previous_frame_hash,
        payload,
        snapshot.proper_tick,
        snapshot.kairos_tick,
        snapshot.sector_id as i8,
    );
    let public_commitment = tagged_hash(
        "KAIROS_PHASELINK_PUBLIC_COMMITMENT_V1",
        &[
            &payload_hash,
            &previous_frame_hash,
            &sequence.to_be_bytes(),
        ],
    );
    let mut frame = PhaseLinkFrame {
        version: PHASELINK_VERSION,
        flags: PhaseLinkFlags {
            proof_required: true,
            loopback: true,
            ..PhaseLinkFlags::default()
        },
        source_node: source,
        destination_node: dest,
        equation_root: kernel.phaselink.equation_root,
        boot_session_root: kernel.phaselink.boot_session_root,
        sequence_number: sequence,
        chronos_tick: snapshot.proper_tick,
        kairos_tick: snapshot.kairos_tick,
        stateclock_sector: snapshot.sector_id as i8,
        object_kind: PhaseLinkObjectKind::Driver,
        object_id: object_id.into(),
        operation: operation.into(),
        previous_phase: previous,
        next_phase: next,
        previous_frame_hash,
        transition_proof: proof.transition,
        payload_encoding: PayloadEncoding::RawBinary,
        payload: payload.to_vec(),
        payload_hash,
        public_commitment,
        frame_hash: [0u8; 32],
    };
    let encoded = serialize_frame_without_hash(&frame).map_err(|_| AiError::MalformedResponse)?;
    frame.frame_hash = compute_frame_hash(&encoded);
    Ok(frame)
}

pub fn hex32(h: &StateHash) -> String {
    let mut s = String::with_capacity(64);
    for b in h {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

pub fn parent_prefixed(parent: &StateHash, args: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(32 + args.len());
    v.extend_from_slice(parent);
    v.extend_from_slice(args);
    v
}

fn split_parent_payload(payload: &[u8]) -> (StateHash, &[u8]) {
    if payload.len() >= 32 {
        let mut h = [0u8; 32];
        h.copy_from_slice(&payload[..32]);
        (h, &payload[32..])
    } else {
        ([0u8; 32], payload)
    }
}

fn parse_act_object(object_id: &str) -> Result<(u64, String), AiError> {
    let (id_s, tool) = object_id.split_once('/').ok_or(AiError::MalformedResponse)?;
    let job_id: u64 = id_s.parse().map_err(|_| AiError::MalformedResponse)?;
    if tool.is_empty() {
        return Err(AiError::MalformedResponse);
    }
    Ok((job_id, tool.into()))
}

fn find_job(jobs: &[AiJob], job_id: u64) -> Option<&AiJob> {
    jobs.iter().find(|j| j.job_id == job_id)
}

fn job_counts(ai: &crate::kernel::ai::AiBridgeController) -> (usize, usize, usize, usize, usize) {
    let mut active = 0;
    let mut queued = 0;
    let mut completed = 0;
    let mut failed = 0;
    let mut cancelled = 0;
    for j in &ai.jobs {
        match j.status {
            AiJobStatus::Running => active += 1,
            AiJobStatus::Queued => queued += 1,
            AiJobStatus::Completed => completed += 1,
            AiJobStatus::Failed => failed += 1,
            AiJobStatus::Cancelled => cancelled += 1,
        }
    }
    (active, queued, completed, failed, cancelled)
}

fn phase_label(p: PhaseState) -> &'static str {
    match p {
        PhaseState::Phi0 => "Phi0",
        PhaseState::Phi1 => "Phi1",
        PhaseState::Phi2 => "Phi2",
        PhaseState::Phi3 => "Phi3",
    }
}
