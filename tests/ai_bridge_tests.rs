//! AI control layer integration tests.

#![cfg(feature = "std")]

use kairos::kernel::ai::bridge;
use kairos::kernel::ai::error::AiError;
use kairos::kernel::ai::mock::{spawn_tcp_mock, MockAiServer};
use kairos::kernel::ai::permissions::tool_permission;
use kairos::kernel::ai::types::{AiJobStatus, ToolPermission, MAX_ACTIVE_JOBS, MAX_RESPONSE_BYTES};
use kairos::Kernel;

fn fresh_kernel() -> Kernel {
    let mut k = Kernel::new();
    k.init_state_domain();
    k
}

#[test]
fn connect_mock_success() {
    let mut kernel = fresh_kernel();
    let msg = bridge::connect(&mut kernel, "mock", 8000).unwrap();
    assert!(msg.contains("mock:8000"));
    assert!(kernel.ai.connected);
}

#[test]
fn connect_failure_bad_host() {
    let mut kernel = fresh_kernel();
    let err = bridge::connect(&mut kernel, "127.0.0.1", 1).unwrap_err();
    assert!(matches!(err, AiError::ConnectionFailed(_)));
    assert!(!kernel.ai.connected);
}

#[test]
fn create_ai_job() {
    let mut kernel = fresh_kernel();
    bridge::connect(&mut kernel, "mock", 8000).unwrap();
    let summary = bridge::ask(&mut kernel, "explain kernel state").unwrap();
    assert_eq!(summary.job_id, 1);
    assert_eq!(summary.status, AiJobStatus::Completed);
}

#[test]
fn phase_transitions_on_success() {
    let mut kernel = fresh_kernel();
    bridge::connect(&mut kernel, "mock", 8000).unwrap();
    let summary = bridge::ask(&mut kernel, "summarize process phases").unwrap();
    let trace = bridge::trace_job(&mut kernel, summary.job_id).unwrap();
    assert!(trace.contains("Phi1 created"));
    assert!(trace.contains("request validated"));
    assert!(trace.contains("Phi2 inference started"));
    assert!(trace.contains("response received"));
    assert!(trace.contains("Phi0 completed"));
}

#[test]
fn valid_response_stored() {
    let mut kernel = fresh_kernel();
    bridge::connect(&mut kernel, "mock", 8000).unwrap();
    let summary = bridge::ask(&mut kernel, "summarize process phases").unwrap();
    let show = bridge::show_job(&mut kernel, summary.job_id, false).unwrap();
    assert!(show.contains("completed"));
    assert!(show.contains("Phi2 has one active process"));
}

#[test]
fn statechain_entry_created() {
    use kairos::kernel::ai::types::AI_STATECHAIN_OBJECT_ID;
    use kairos::kernel::state_proof::ObjectKind;

    let mut kernel = fresh_kernel();
    bridge::connect(&mut kernel, "mock", 8000).unwrap();
    let before = kernel
        .statechain
        .chain(ObjectKind::Driver, AI_STATECHAIN_OBJECT_ID)
        .map(|c| c.events.len())
        .unwrap_or(0);
    bridge::ask(&mut kernel, "hello").unwrap();
    let after = kernel
        .statechain
        .chain(ObjectKind::Driver, AI_STATECHAIN_OBJECT_ID)
        .map(|c| c.events.len())
        .unwrap_or(0);
    assert!(after > before);
}

#[test]
fn mismatched_request_id_fails() {
    let mut kernel = fresh_kernel();
    bridge::connect(&mut kernel, "mock", 8000).unwrap();
    kernel.ai.mock_server.wrong_request_id = true;
    let summary = bridge::ask(&mut kernel, "test").unwrap();
    let job = kernel
        .ai
        .jobs
        .iter()
        .find(|j| j.job_id == summary.job_id)
        .unwrap();
    assert_eq!(job.status, AiJobStatus::Failed);
}

#[test]
fn malformed_response_fails() {
    let server = MockAiServer {
        malformed: true,
        ..Default::default()
    };
    let line = r#"{"request_id":1,"process_id":1,"model":"default","input":"x","input_hash":"a","allowed_tools":[],"deadline":5000,"current_phase":"Phi1","parent_chain_hash":"b"}"#;
    let result = server.process_line(line);
    assert!(result.is_err());
}

#[test]
fn timeout_on_unreachable_tcp() {
    let mut kernel = fresh_kernel();
    let err = bridge::connect(&mut kernel, "127.0.0.1", 1).unwrap_err();
    assert!(matches!(err, AiError::ConnectionFailed(_)));
}

#[test]
fn cancellation_logged() {
    let mut kernel = fresh_kernel();
    bridge::connect(&mut kernel, "mock", 8000).unwrap();
    kernel.ai.jobs.clear();
    kernel.ai.next_job_id = 42;
    let job = kairos::kernel::ai::types::AiJob {
        job_id: 42,
        process_id: kernel.spawn_process("ai-job-42"),
        parent_job_id: None,
        server_address: "mock:8000".into(),
        model_name: "default".into(),
        status: AiJobStatus::Running,
        phase: kairos::PhaseState::Phi2,
        created_tick: 0,
        started_tick: None,
        completed_tick: None,
        deadline_tick: 5000,
        input_hash: [0u8; 32],
        output_hash: [0u8; 32],
        error_code: 0,
        error_message: String::new(),
        requested_tools: vec![],
        approved_tools: vec![],
        denied_tools: vec![],
        cancel_requested: false,
        prompt: String::new(),
        response: String::new(),
        store_content: true,
        trace: vec![],
        parent_chain_hash: [0u8; 32],
    };
    kernel.ai.jobs.push(job);
    let msg = bridge::cancel(&mut kernel, 42).unwrap();
    assert!(msg.contains("cancelled"));
    let trace = bridge::trace_job(&mut kernel, 42).unwrap();
    assert!(trace.contains("cancelled"));
}

#[test]
fn allowed_read_only_tool() {
    let mut kernel = fresh_kernel();
    bridge::connect(&mut kernel, "mock", 8000).unwrap();
    let server = MockAiServer {
        return_tool: true,
        ..Default::default()
    };
    let line = r#"{"request_id":1,"process_id":1,"model":"default","input":"x","input_hash":"a","allowed_tools":["system.state"],"deadline":5000,"current_phase":"Phi2","parent_chain_hash":"b"}"#;
    let resp = server.process_line(line).unwrap();
    assert!(resp.tool_requests.is_empty() || !resp.output.is_empty());
    let result =
        kairos::kernel::ai::tools::execute_tool(&kernel, &kernel.ai, "system.state", "none");
    assert!(result.is_ok());
}

#[test]
fn denied_privileged_tool() {
    let kernel = fresh_kernel();
    let result =
        kairos::kernel::ai::tools::execute_tool(&kernel, &kernel.ai, "shell.execute", "none");
    assert_eq!(result.unwrap_err(), AiError::ToolDenied);
}

#[test]
fn multiple_parallel_jobs() {
    let mut kernel = fresh_kernel();
    bridge::connect(&mut kernel, "mock", 8000).unwrap();
    let a = bridge::ask(&mut kernel, "job a").unwrap();
    let b = bridge::ask(&mut kernel, "job b").unwrap();
    assert_ne!(a.job_id, b.job_id);
    assert_eq!(kernel.ai.jobs.len(), 2);
}

#[test]
fn oversized_response_rejected() {
    let server = MockAiServer {
        oversized: true,
        ..Default::default()
    };
    let line = r#"{"request_id":1,"process_id":1,"model":"default","input":"x","input_hash":"a","allowed_tools":[],"deadline":5000,"current_phase":"Phi1","parent_chain_hash":"b"}"#;
    let err = server.process_line(line).unwrap_err();
    assert_eq!(err, AiError::ResponseTooLarge);
    let _ = MAX_RESPONSE_BYTES;
}

#[test]
fn server_disconnect_during_job() {
    let server = MockAiServer {
        disconnect_after_send: true,
        ..Default::default()
    };
    let line = r#"{"request_id":1,"process_id":1,"model":"default","input":"x","input_hash":"a","allowed_tools":[],"deadline":5000,"current_phase":"Phi1","parent_chain_hash":"b"}"#;
    let err = server.process_line(line).unwrap_err();
    assert_eq!(err, AiError::DisconnectedDuringJob);
}

#[test]
fn job_cleanup_after_failure() {
    let mut kernel = fresh_kernel();
    bridge::connect(&mut kernel, "mock", 8000).unwrap();
    kernel.ai.mock_server.malformed = true;
    let summary = bridge::ask(&mut kernel, "trigger fail").unwrap();
    let job = kernel
        .ai
        .jobs
        .iter()
        .find(|j| j.job_id == summary.job_id)
        .unwrap();
    assert_eq!(job.status, AiJobStatus::Failed);
    assert!(!job.trace.is_empty());
}

#[test]
fn tcp_connect_and_ask() {
    let port = spawn_tcp_mock(MockAiServer::default()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    let mut kernel = fresh_kernel();
    bridge::connect(&mut kernel, "127.0.0.1", port).unwrap();
    let summary = bridge::ask(&mut kernel, "hello tcp").unwrap();
    for _ in 0..50 {
        bridge::poll_completions(&mut kernel);
        if let Some(job) = kernel.ai.jobs.iter().find(|j| j.job_id == summary.job_id) {
            if job.status == AiJobStatus::Completed {
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let job = kernel
        .ai
        .jobs
        .iter()
        .find(|j| j.job_id == summary.job_id)
        .unwrap();
    assert_eq!(job.status, AiJobStatus::Completed);
}

#[test]
fn allow_deny_tool_permission() {
    let mut kernel = fresh_kernel();
    bridge::deny(&mut kernel, "system.state").unwrap();
    assert_eq!(
        tool_permission(&kernel.ai, "system.state"),
        Some(ToolPermission::Denied)
    );
    bridge::allow(&mut kernel, "system.state").unwrap();
    assert_eq!(
        tool_permission(&kernel.ai, "system.state"),
        Some(ToolPermission::Allowed)
    );
}

#[test]
fn job_table_full_rejection() {
    let mut kernel = fresh_kernel();
    bridge::connect(&mut kernel, "mock", 8000).unwrap();
    for i in 0..MAX_ACTIVE_JOBS {
        let job = kairos::kernel::ai::types::AiJob {
            job_id: i as u64 + 1,
            process_id: kernel.spawn_process(&format!("ai-job-{i}")),
            parent_job_id: None,
            server_address: "mock:8000".into(),
            model_name: "default".into(),
            status: AiJobStatus::Running,
            phase: kairos::PhaseState::Phi2,
            created_tick: 0,
            started_tick: None,
            completed_tick: None,
            deadline_tick: 5000,
            input_hash: [0u8; 32],
            output_hash: [0u8; 32],
            error_code: 0,
            error_message: String::new(),
            requested_tools: vec![],
            approved_tools: vec![],
            denied_tools: vec![],
            cancel_requested: false,
            prompt: String::new(),
            response: String::new(),
            store_content: true,
            trace: vec![],
            parent_chain_hash: [0u8; 32],
        };
        kernel.ai.jobs.push(job);
    }
    let err = bridge::ask(&mut kernel, "overflow").unwrap_err();
    assert_eq!(err, AiError::JobTableFull);
}
