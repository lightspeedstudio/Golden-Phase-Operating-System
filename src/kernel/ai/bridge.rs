//! AI bridge service — job lifecycle, network I/O, authorization.

use crate::core::gphase_core::PhaseState;
use crate::kernel::ai::client;
use crate::kernel::ai::error::AiError;
use crate::kernel::ai::permissions::{allow_tool, deny_tool, format_tools_table};
use crate::kernel::ai::protocol::{encode_request, AiRequestMessage};
use crate::kernel::ai::tools::{authorize_tool_gate, execute_tool};
use crate::kernel::ai::trace::{format_trace, record_event, short_hash};
use crate::kernel::ai::types::{
    AiBridgeController, AiHook, AiJob, AiJobStatus, CompletionChannel, ToolPermission,
    CONNECT_TIMEOUT_MS, DEFAULT_DEADLINE_MS, DEFAULT_MODEL, MAX_ACTIVE_JOBS, MAX_PROMPT_BYTES,
    MAX_QUEUED_JOBS, MAX_RESPONSE_BYTES, MAX_TOOL_REQUESTS_PER_JOB,
};
use crate::kernel::hash::tagged_hash;
use crate::kernel::Kernel;
use crate::platform::{String, Vec};

#[cfg(feature = "std")]
use crate::kernel::ai::protocol::AiResponseMessage;
#[cfg(feature = "std")]
use crate::kernel::ai::types::AiJobCompletion;

#[derive(Debug, Clone)]
pub struct AiAskSummary {
    pub job_id: u64,
    pub process_id: u32,
    pub phase: PhaseState,
    pub status: AiJobStatus,
}

pub fn connect(kernel: &mut Kernel, host: &str, port: u16) -> Result<String, AiError> {
    if kernel.ai.connected {
        return Err(AiError::AlreadyConnected);
    }
    if host == "mock" {
        kernel.ai.use_mock_transport = true;
        kernel.ai.connected = true;
        kernel.ai.host = "mock".into();
        kernel.ai.port = port;
        kernel.ai.last_error = None;
        kernel.ai.set_completion_channel(CompletionChannel::new());
        return Ok(format!("AI bridge connected to mock:{port}"));
    }
    kernel.ai.use_mock_transport = false;
    client::verify_connection(host, port, CONNECT_TIMEOUT_MS)?;
    kernel.ai.set_completion_channel(CompletionChannel::new());
    kernel.ai.connected = true;
    kernel.ai.host = host.into();
    kernel.ai.port = port;
    kernel.ai.last_error = None;
    Ok(format!("AI bridge connected to {host}:{port}"))
}

pub fn disconnect(kernel: &mut Kernel) -> String {
    kernel.ai.connected = false;
    kernel.ai.use_mock_transport = false;
    kernel.ai.host.clear();
    kernel.ai.port = 0;
    #[cfg(feature = "std")]
    {
        kernel.ai.completion_tx = None;
        kernel.ai.completion_rx = None;
    }
    "AI bridge disconnected".into()
}

pub fn poll_completions(kernel: &mut Kernel) {
    let completions: Vec<_> = {
        let Some(rx) = kernel.ai.completion_rx.as_ref() else {
            return;
        };
        let mut batch = Vec::new();
        while let Ok(done) = rx.try_recv() {
            batch.push(done);
        }
        batch
    };
    for done in completions {
        apply_completion(kernel, done.job_id, done.result);
    }
}

pub fn status(kernel: &Kernel) -> String {
    let ai = &kernel.ai;
    let (active, queued, completed, failed, cancelled) = job_counts(ai);
    let allowed = ai
        .tool_permissions
        .iter()
        .filter(|(_, p)| *p == ToolPermission::Allowed)
        .count();
    format!(
        "AI BRIDGE STATUS\n\n\
connected:              {}\n\
server:                 {}\n\
active jobs:            {active}\n\
queued jobs:            {queued}\n\
completed jobs:         {completed}\n\
failed jobs:            {failed}\n\
cancelled jobs:         {cancelled}\n\
allowed tools:          {allowed}\n\
last error:             {}\n",
        if ai.connected { "yes" } else { "no" },
        if ai.connected {
            format!("{}:{}", ai.host, ai.port)
        } else {
            "-".into()
        },
        ai.last_error.as_deref().unwrap_or("-"),
    )
}

pub fn ask(kernel: &mut Kernel, prompt: &str) -> Result<AiAskSummary, AiError> {
    if !kernel.ai.connected {
        return Err(AiError::NotConnected);
    }
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(AiError::PromptTooLarge);
    }
    poll_completions(kernel);
    let counts = job_counts(&kernel.ai);
    if counts.0 >= MAX_ACTIVE_JOBS {
        return Err(AiError::JobTableFull);
    }
    if counts.1 >= MAX_QUEUED_JOBS {
        return Err(AiError::QueueFull);
    }

    let job_id = kernel.ai.next_job_id;
    kernel.ai.next_job_id += 1;
    let name = format!("ai-job-{job_id}");
    let pid = kernel.spawn_process(&name);
    let tick = kernel.clock.snapshot().chronos_tick;
    let input_hash = tagged_hash("KAIROS_AI_INPUT_V1", &[prompt.as_bytes()]);
    let server_address = format!("{}:{}", kernel.ai.host, kernel.ai.port);
    let mut job = AiJob {
        job_id,
        process_id: pid,
        parent_job_id: None,
        server_address,
        model_name: DEFAULT_MODEL.into(),
        status: AiJobStatus::Queued,
        phase: PhaseState::Phi1,
        created_tick: tick,
        started_tick: None,
        completed_tick: None,
        deadline_tick: tick + DEFAULT_DEADLINE_MS,
        input_hash,
        output_hash: [0u8; 32],
        error_code: 0,
        error_message: String::new(),
        requested_tools: Vec::new(),
        approved_tools: kernel.ai.allowed_tool_names(),
        denied_tools: Vec::new(),
        cancel_requested: false,
        prompt: if kernel.ai.store_prompts {
            prompt.into()
        } else {
            String::new()
        },
        response: String::new(),
        store_content: kernel.ai.store_prompts,
        trace: Vec::new(),
        parent_chain_hash: [0u8; 32],
    };
    record_event(
        kernel,
        &mut job,
        AiHook::JobCreated,
        PhaseState::Phi1,
        "",
        "",
        0,
    );
    record_event(
        kernel,
        &mut job,
        AiHook::BeforeRequest,
        PhaseState::Phi1,
        "",
        "validated",
        0,
    );
    kernel.ai.jobs.push(job);
    dispatch_job(kernel, job_id)?;
    let job = find_job(&kernel.ai, job_id).ok_or(AiError::JobNotFound)?;
    Ok(AiAskSummary {
        job_id,
        process_id: job.process_id,
        phase: job.phase,
        status: job.status,
    })
}

fn dispatch_job(kernel: &mut Kernel, job_id: u64) -> Result<(), AiError> {
    update_job_status(
        &mut kernel.ai,
        job_id,
        AiJobStatus::Running,
        PhaseState::Phi2,
    );
    let request = {
        let job = find_job(&kernel.ai, job_id).ok_or(AiError::JobNotFound)?;
        build_request_message(&kernel.ai, job)
    };
    if let Some(mut job) = remove_job(&mut kernel.ai, job_id) {
        record_event(
            kernel,
            &mut job,
            AiHook::RequestSent,
            PhaseState::Phi2,
            "",
            "sent",
            0,
        );
        kernel.ai.jobs.push(job);
    }

    if kernel.ai.use_mock_transport {
        let server = kernel.ai.mock_server.clone();
        let line = encode_request(&request);
        let result = server.process_line(line.trim());
        apply_completion(kernel, job_id, result);
        return Ok(());
    }

    #[cfg(feature = "std")]
    {
        let host = kernel.ai.host.clone();
        let port = kernel.ai.port;
        let tx = kernel
            .ai
            .completion_tx
            .as_ref()
            .ok_or(AiError::NotConnected)?
            .clone();
        std::thread::spawn(move || {
            let result = client::send_request(
                &host,
                port,
                &request,
                DEFAULT_DEADLINE_MS,
                MAX_RESPONSE_BYTES,
            );
            let _ = tx.send(AiJobCompletion { job_id, result });
        });
        return Ok(());
    }
    #[cfg(not(feature = "std"))]
    Err(AiError::NotConnected)
}

#[cfg(feature = "std")]
fn apply_completion(kernel: &mut Kernel, job_id: u64, result: Result<AiResponseMessage, AiError>) {
    match result {
        Ok(response) => {
            if let Err(err) = finish_response(kernel, job_id, response) {
                fail_job(kernel, job_id, err);
            }
        }
        Err(err) => fail_job(kernel, job_id, err),
    }
}

#[cfg(feature = "std")]
fn finish_response(
    kernel: &mut Kernel,
    job_id: u64,
    response: AiResponseMessage,
) -> Result<(), AiError> {
    if response.request_id != job_id {
        return Err(AiError::RequestIdMismatch);
    }
    let mut job = remove_job(&mut kernel.ai, job_id).ok_or(AiError::JobNotFound)?;
    if job.cancel_requested {
        return Err(AiError::Cancelled);
    }
    record_event(
        kernel,
        &mut job,
        AiHook::ResponseReceived,
        PhaseState::Phi2,
        "",
        "",
        0,
    );
    if response.output.len() > MAX_RESPONSE_BYTES {
        return Err(AiError::ResponseTooLarge);
    }
    job.response = if job.store_content {
        response.output.clone()
    } else {
        String::new()
    };
    job.output_hash = tagged_hash("KAIROS_AI_OUTPUT_V1", &[response.output.as_bytes()]);

    let tool_requests = response.tool_requests;
    if tool_requests.len() > MAX_TOOL_REQUESTS_PER_JOB {
        return Err(AiError::MalformedResponse);
    }
    for tool_req in tool_requests {
        job.requested_tools.push(tool_req.tool.clone());
        record_event(
            kernel,
            &mut job,
            AiHook::BeforeTool,
            PhaseState::Phi3,
            &tool_req.tool,
            "pending",
            0,
        );
        authorize_tool_gate(job.phase)?;
        match execute_tool(kernel, &kernel.ai, &tool_req.tool, &tool_req.arguments) {
            Ok(_) => {
                job.approved_tools.push(tool_req.tool.clone());
                record_event(
                    kernel,
                    &mut job,
                    AiHook::ToolAllowed,
                    PhaseState::Phi2,
                    &tool_req.tool,
                    "allowed",
                    0,
                );
                record_event(
                    kernel,
                    &mut job,
                    AiHook::AfterTool,
                    PhaseState::Phi2,
                    &tool_req.tool,
                    "ok",
                    0,
                );
            }
            Err(AiError::ToolDenied) | Err(AiError::ToolNotFound) => {
                job.denied_tools.push(tool_req.tool.clone());
                record_event(
                    kernel,
                    &mut job,
                    AiHook::ToolDenied,
                    PhaseState::Phi0,
                    &tool_req.tool,
                    "denied",
                    AiError::ToolDenied.code(),
                );
                job.status = AiJobStatus::Failed;
                job.error_code = AiError::ToolDenied.code();
                job.error_message = AiError::ToolDenied.reason();
                job.completed_tick = Some(kernel.clock.snapshot().chronos_tick);
                kernel.ai.jobs.push(job);
                kernel.ai.last_error = Some(AiError::ToolDenied.reason());
                return Ok(());
            }
            Err(err) => return Err(err),
        }
    }

    job.status = AiJobStatus::Completed;
    job.completed_tick = Some(kernel.clock.snapshot().chronos_tick);
    record_event(
        kernel,
        &mut job,
        AiHook::JobCompleted,
        PhaseState::Phi0,
        "",
        "",
        0,
    );
    kernel.ai.jobs.push(job);
    Ok(())
}

fn fail_job(kernel: &mut Kernel, job_id: u64, err: AiError) {
    let Some(mut job) = remove_job(&mut kernel.ai, job_id) else {
        return;
    };
    job.status = if err == AiError::Cancelled {
        AiJobStatus::Cancelled
    } else {
        AiJobStatus::Failed
    };
    job.error_code = err.code();
    job.error_message = err.reason();
    job.completed_tick = Some(kernel.clock.snapshot().chronos_tick);
    let error_code = job.error_code;
    let hook = if err == AiError::Cancelled {
        AiHook::JobCancelled
    } else {
        AiHook::JobFailed
    };
    record_event(
        kernel,
        &mut job,
        hook,
        PhaseState::Phi0,
        "",
        "failed",
        error_code,
    );
    kernel.ai.last_error = Some(job.error_message.clone());
    kernel.ai.jobs.push(job);
}

pub fn cancel(kernel: &mut Kernel, job_id: u64) -> Result<String, AiError> {
    poll_completions(kernel);
    let mut job = remove_job(&mut kernel.ai, job_id).ok_or(AiError::JobNotFound)?;
    if matches!(
        job.status,
        AiJobStatus::Completed | AiJobStatus::Failed | AiJobStatus::Cancelled
    ) {
        kernel.ai.jobs.push(job);
        return Err(AiError::JobNotFound);
    }
    job.cancel_requested = true;
    job.status = AiJobStatus::Cancelled;
    job.error_code = AiError::Cancelled.code();
    job.error_message = AiError::Cancelled.reason();
    let error_code = job.error_code;
    record_event(
        kernel,
        &mut job,
        AiHook::JobCancelled,
        PhaseState::Phi0,
        "",
        "cancelled",
        error_code,
    );
    kernel.ai.jobs.push(job);
    Ok(format!("AI job {job_id} cancelled"))
}

pub fn jobs_table(kernel: &mut Kernel) -> String {
    poll_completions(kernel);
    let mut out = String::from("JOB   PID   PHASE   STATUS      SERVER          AGE\n");
    let now = kernel.clock.snapshot().chronos_tick;
    for job in &kernel.ai.jobs {
        let age = now.saturating_sub(job.created_tick);
        out.push_str(&format!(
            "{:<5} {:<5} {:<7} {:<11} {:<15} {}ms\n",
            job.job_id,
            job.process_id,
            format_phase(job.phase),
            job.status.label(),
            job.server_address,
            age
        ));
    }
    out
}

pub fn show_job(
    kernel: &mut Kernel,
    job_id: u64,
    include_private: bool,
) -> Result<String, AiError> {
    poll_completions(kernel);
    let job = find_job(&kernel.ai, job_id).ok_or(AiError::JobNotFound)?;
    let mut out = format!(
        "AI JOB {job_id}\n\n\
Status: {}\n\
Phase: {}\n\
PID: {}\n\
Server: {}\n\
Input hash: {}\n\
Output hash: {}\n",
        job.status.label(),
        format_phase(job.phase),
        job.process_id,
        job.server_address,
        short_hash(&job.input_hash),
        short_hash(&job.output_hash),
    );
    if (include_private || job.store_content) && !job.response.is_empty() {
        out.push_str(&format!("Response: {}\n", job.response));
    }
    if !job.error_message.is_empty() {
        out.push_str(&format!("Error: {}\n", job.error_message));
    }
    Ok(out)
}

pub fn trace_job(kernel: &mut Kernel, job_id: u64) -> Result<String, AiError> {
    poll_completions(kernel);
    let job = find_job(&kernel.ai, job_id).ok_or(AiError::JobNotFound)?;
    Ok(format_trace(job))
}

pub fn tools(kernel: &Kernel) -> String {
    format_tools_table(&kernel.ai)
}

pub fn allow(kernel: &mut Kernel, tool: &str) -> Result<String, AiError> {
    if kernel.authority.is_anom_protected() {
        return Err(AiError::NotPrivileged);
    }
    let msg = allow_tool(&mut kernel.ai, tool)?;
    record_permission_change(kernel, tool, "allowed")?;
    Ok(msg)
}

pub fn deny(kernel: &mut Kernel, tool: &str) -> Result<String, AiError> {
    if kernel.authority.is_anom_protected() {
        return Err(AiError::NotPrivileged);
    }
    let msg = deny_tool(&mut kernel.ai, tool)?;
    record_permission_change(kernel, tool, "denied")?;
    Ok(msg)
}

fn record_permission_change(
    kernel: &mut Kernel,
    tool: &str,
    decision: &str,
) -> Result<(), AiError> {
    let _ = kernel.statechain.record_transition(
        crate::kernel::state_proof::ObjectKind::Driver,
        crate::kernel::ai::types::AI_STATECHAIN_OBJECT_ID,
        crate::kernel::state_root::SubsystemKind::Driver,
        PhaseState::Phi2,
        PhaseState::Phi3,
        "ai_permission_change",
        None,
        format!("{tool}:{decision}").as_bytes(),
    );
    Ok(())
}

fn build_request_message(_ai: &AiBridgeController, job: &AiJob) -> AiRequestMessage {
    AiRequestMessage {
        request_id: job.job_id,
        process_id: job.process_id,
        model: job.model_name.clone(),
        input: job.prompt.clone(),
        input_hash: short_hash(&job.input_hash),
        allowed_tools: job.approved_tools.clone(),
        deadline: DEFAULT_DEADLINE_MS,
        current_phase: format_phase(job.phase).into(),
        parent_chain_hash: short_hash(&job.parent_chain_hash),
    }
}

fn job_counts(ai: &AiBridgeController) -> (usize, usize, usize, usize, usize) {
    let mut active = 0;
    let mut queued = 0;
    let mut completed = 0;
    let mut failed = 0;
    let mut cancelled = 0;
    for job in &ai.jobs {
        match job.status {
            AiJobStatus::Running => active += 1,
            AiJobStatus::Queued => queued += 1,
            AiJobStatus::Completed => completed += 1,
            AiJobStatus::Failed => failed += 1,
            AiJobStatus::Cancelled => cancelled += 1,
        }
    }
    (active, queued, completed, failed, cancelled)
}

fn find_job<'a>(ai: &'a AiBridgeController, job_id: u64) -> Option<&'a AiJob> {
    ai.jobs.iter().find(|j| j.job_id == job_id)
}

fn find_job_mut<'a>(ai: &'a mut AiBridgeController, job_id: u64) -> Option<&'a mut AiJob> {
    ai.jobs.iter_mut().find(|j| j.job_id == job_id)
}

fn remove_job(ai: &mut AiBridgeController, job_id: u64) -> Option<AiJob> {
    let idx = ai.jobs.iter().position(|j| j.job_id == job_id)?;
    Some(ai.jobs.remove(idx))
}

fn update_job_status(
    ai: &mut AiBridgeController,
    job_id: u64,
    status: AiJobStatus,
    phase: PhaseState,
) {
    if let Some(job) = find_job_mut(ai, job_id) {
        job.status = status;
        job.phase = phase;
        if status == AiJobStatus::Running {
            job.started_tick = Some(job.created_tick);
        }
    }
}

fn format_phase(phase: PhaseState) -> &'static str {
    match phase {
        PhaseState::Phi0 => "Phi0",
        PhaseState::Phi1 => "Phi1",
        PhaseState::Phi2 => "Phi2",
        PhaseState::Phi3 => "Phi3",
    }
}
