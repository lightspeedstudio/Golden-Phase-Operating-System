//! In-process mock AI server for tests.

use crate::kernel::ai::error::AiError;
use crate::kernel::ai::protocol::{decode_response, AiRequestMessage, AiResponseMessage};
use crate::kernel::hash::tagged_hash;
use crate::platform::String;

#[derive(Clone, Debug)]
pub struct MockAiServer {
    pub fail_connect: bool,
    pub malformed: bool,
    pub wrong_request_id: bool,
    pub disconnect_after_send: bool,
    pub oversized: bool,
    pub return_tool: bool,
}

impl Default for MockAiServer {
    fn default() -> Self {
        Self {
            fail_connect: false,
            malformed: false,
            wrong_request_id: false,
            disconnect_after_send: false,
            oversized: false,
            return_tool: false,
        }
    }
}

impl MockAiServer {
    pub fn handle_request(&self, request: &AiRequestMessage) -> Result<String, AiError> {
        if self.disconnect_after_send {
            return Err(AiError::DisconnectedDuringJob);
        }
        if self.malformed {
            return Ok("{not-json}\n".into());
        }
        if self.oversized {
            return Ok("x".repeat(40_000) + "\n");
        }
        let output = if request.input.contains("process phases") {
            "Phi2 has one active process and no recorded violations.".into()
        } else if request.input.contains("kernel state") {
            "Kernel state is stable with legal phase transitions.".into()
        } else {
            format!("echo: {}", request.input)
        };
        let output_hash = tagged_hash("KAIROS_AI_OUTPUT_V1", &[output.as_bytes()]);
        let hash_hex = hex32(&output_hash);
        let request_id = if self.wrong_request_id {
            request.request_id + 999
        } else {
            request.request_id
        };
        let tool_requests = if self.return_tool {
            r#","tool_requests":[{"tool":"system.state","arguments":"none"}]"#
        } else {
            r#","tool_requests":[]"#
        };
        Ok(format!(
            "{{\"request_id\":{request_id},\"status\":\"ok\",\"output\":\"{}\",\"output_hash\":\"{}\",\"runtime\":5,\"error\":\"\"{tool_requests}}}\n",
            escape_json(&output),
            hash_hex
        ))
    }

    pub fn process_line(&self, line: &str) -> Result<AiResponseMessage, AiError> {
        if !line.trim_start().starts_with('{') {
            return Err(AiError::MalformedResponse);
        }
        let request: AiRequestMessage = parse_request_line(line)?;
        let response_line = self.handle_request(&request)?;
        if self.oversized {
            return Err(AiError::ResponseTooLarge);
        }
        let first = response_line.lines().next().unwrap_or(&response_line);
        decode_response(first)
    }
}

fn parse_request_line(line: &str) -> Result<AiRequestMessage, AiError> {
    let get_str = |key: &str| -> Result<String, AiError> {
        let needle = format!("\"{key}\":\"");
        let idx = line.find(&needle).ok_or(AiError::MalformedResponse)?;
        let rest = &line[idx + needle.len()..];
        let end = rest.find('"').ok_or(AiError::MalformedResponse)?;
        Ok(rest[..end].into())
    };
    let get_u64 = |key: &str| -> Result<u64, AiError> {
        let needle = format!("\"{key}\":");
        let idx = line.find(&needle).ok_or(AiError::MalformedResponse)?;
        let rest = line[idx + needle.len()..].trim_start();
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        rest[..end].parse().map_err(|_| AiError::MalformedResponse)
    };
    Ok(AiRequestMessage {
        request_id: get_u64("request_id")?,
        process_id: get_u64("process_id")? as u32,
        model: get_str("model").unwrap_or_else(|_| "default".into()),
        input: get_str("input").unwrap_or_default(),
        input_hash: get_str("input_hash").unwrap_or_default(),
        allowed_tools: vec!["system.state".into()],
        deadline: get_u64("deadline").unwrap_or(5000),
        current_phase: get_str("current_phase").unwrap_or_else(|_| "Phi1".into()),
        parent_chain_hash: get_str("parent_chain_hash").unwrap_or_default(),
    })
}

fn hex32(hash: &[u8; 32]) -> String {
    let mut out = String::new();
    for b in hash.iter().take(4) {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(feature = "std")]
pub fn spawn_tcp_mock(server: MockAiServer) -> std::io::Result<u16> {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let server = server.clone();
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut buf = [0u8; 8192];
                if let Ok(n) = stream.read(&mut buf) {
                    if let Ok(line) = core::str::from_utf8(&buf[..n]) {
                        if let Ok(resp) = server.process_line(line.trim()) {
                            let payload = format!(
                                "{{\"request_id\":{},\"status\":\"ok\",\"output\":\"{}\",\"output_hash\":\"{}\",\"runtime\":5,\"error\":\"\",\"tool_requests\":[]}}\n",
                                resp.request_id,
                                escape_json(&resp.output),
                                resp.output_hash
                            );
                            let _ = stream.write_all(payload.as_bytes());
                        }
                    }
                }
            });
        }
    });
    Ok(port)
}
