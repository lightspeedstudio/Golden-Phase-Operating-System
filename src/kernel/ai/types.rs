//! AI control layer types.

use crate::core::gphase_core::PhaseState;
#[cfg(feature = "std")]
use crate::kernel::ai::mock::MockAiServer;
use crate::kernel::hash::StateHash;
use crate::platform::{String, Vec};

pub const AI_STATECHAIN_OBJECT_ID: u32 = 9;
pub const DEFAULT_MODEL: &str = "default";
pub const DEFAULT_DEADLINE_MS: u64 = 5_000;
pub const CONNECT_TIMEOUT_MS: u64 = 3_000;

pub const MAX_ACTIVE_JOBS: usize = 16;
pub const MAX_QUEUED_JOBS: usize = 32;
pub const MAX_PROMPT_BYTES: usize = 8_192;
pub const MAX_RESPONSE_BYTES: usize = 32_768;
pub const MAX_TOOL_REQUESTS_PER_JOB: usize = 8;
pub const MAX_TRACE_RECORDS_PER_JOB: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl AiJobStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiHook {
    JobCreated,
    BeforeRequest,
    RequestSent,
    ResponseReceived,
    BeforeTool,
    ToolAllowed,
    ToolDenied,
    AfterTool,
    JobCompleted,
    JobFailed,
    JobCancelled,
    ConnectionError,
}

impl AiHook {
    pub fn label(self) -> &'static str {
        match self {
            Self::JobCreated => "AI_JOB_CREATED",
            Self::BeforeRequest => "AI_BEFORE_REQUEST",
            Self::RequestSent => "AI_REQUEST_SENT",
            Self::ResponseReceived => "AI_RESPONSE_RECEIVED",
            Self::BeforeTool => "AI_BEFORE_TOOL",
            Self::ToolAllowed => "AI_TOOL_ALLOWED",
            Self::ToolDenied => "AI_TOOL_DENIED",
            Self::AfterTool => "AI_AFTER_TOOL",
            Self::JobCompleted => "AI_JOB_COMPLETED",
            Self::JobFailed => "AI_JOB_FAILED",
            Self::JobCancelled => "AI_JOB_CANCELLED",
            Self::ConnectionError => "AI_CONNECTION_ERROR",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPermission {
    Allowed,
    Denied,
}

#[derive(Debug, Clone)]
pub struct AiTraceRecord {
    pub tick: u64,
    pub job_id: u64,
    pub process_id: u32,
    pub phase_before: PhaseState,
    pub phase_after: PhaseState,
    pub event: String,
    pub server: String,
    pub input_hash: StateHash,
    pub output_hash: StateHash,
    pub tool_name: String,
    pub authorization_result: String,
    pub error_code: u16,
    pub parent_chain_hash: StateHash,
    pub record_hash: StateHash,
}

#[derive(Debug, Clone)]
pub struct AiJob {
    pub job_id: u64,
    pub process_id: u32,
    pub parent_job_id: Option<u64>,
    pub server_address: String,
    pub model_name: String,
    pub status: AiJobStatus,
    pub phase: PhaseState,
    pub created_tick: u64,
    pub started_tick: Option<u64>,
    pub completed_tick: Option<u64>,
    pub deadline_tick: u64,
    pub input_hash: StateHash,
    pub output_hash: StateHash,
    pub error_code: u16,
    pub error_message: String,
    pub requested_tools: Vec<String>,
    pub approved_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub cancel_requested: bool,
    pub prompt: String,
    pub response: String,
    pub store_content: bool,
    pub trace: Vec<AiTraceRecord>,
    pub parent_chain_hash: StateHash,
}

#[derive(Debug, Default)]
pub struct AiBridgeController {
    pub connected: bool,
    pub host: String,
    pub port: u16,
    pub last_error: Option<String>,
    pub jobs: Vec<AiJob>,
    pub next_job_id: u64,
    pub tool_permissions: Vec<(String, ToolPermission)>,
    pub use_mock_transport: bool,
    pub store_prompts: bool,
    #[cfg(feature = "std")]
    pub mock_server: MockAiServer,
    #[cfg(feature = "std")]
    pub completion_tx: Option<std::sync::mpsc::Sender<AiJobCompletion>>,
    #[cfg(feature = "std")]
    pub completion_rx: Option<std::sync::mpsc::Receiver<AiJobCompletion>>,
}

#[cfg(feature = "std")]
#[derive(Debug)]
pub struct AiJobCompletion {
    pub job_id: u64,
    pub result:
        Result<crate::kernel::ai::protocol::AiResponseMessage, crate::kernel::ai::error::AiError>,
}

#[cfg(feature = "std")]
pub struct CompletionChannel {
    pub tx: std::sync::mpsc::Sender<AiJobCompletion>,
    pub rx: std::sync::mpsc::Receiver<AiJobCompletion>,
}

#[cfg(feature = "std")]
impl CompletionChannel {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self { tx, rx }
    }
}

impl AiBridgeController {
    pub fn new() -> Self {
        let mut controller = Self {
            next_job_id: 1,
            store_prompts: true,
            ..Self::default()
        };
        controller.init_default_tools();
        controller
    }

    #[cfg(feature = "std")]
    pub fn set_completion_channel(&mut self, ch: CompletionChannel) {
        self.completion_tx = Some(ch.tx);
        self.completion_rx = Some(ch.rx);
    }

    fn init_default_tools(&mut self) {
        let allowed = [
            "system.state",
            "system.version",
            "process.list",
            "process.show",
            "phase.counts",
            "trace.read",
            "topology.read",
            "memory.summary",
        ];
        let denied = [
            "shell.execute",
            "kernel.write",
            "memory.write",
            "process.kill",
            "system.reboot",
            "system.shutdown",
            "log.delete",
            "checkpoint.delete",
            "motor.control",
            "actuator.control",
        ];
        for name in allowed {
            self.tool_permissions
                .push((name.into(), ToolPermission::Allowed));
        }
        for name in denied {
            self.tool_permissions
                .push((name.into(), ToolPermission::Denied));
        }
    }

    pub fn allowed_tool_names(&self) -> Vec<String> {
        self.tool_permissions
            .iter()
            .filter(|(_, p)| *p == ToolPermission::Allowed)
            .map(|(n, _)| n.clone())
            .collect()
    }
}
