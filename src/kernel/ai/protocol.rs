//! Newline-delimited JSON protocol for the AI bridge (minimal hand-rolled codec).

use crate::kernel::ai::error::AiError;
use crate::platform::{String, Vec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRequestMessage {
    pub request_id: u64,
    pub process_id: u32,
    pub model: String,
    pub input: String,
    pub input_hash: String,
    pub allowed_tools: Vec<String>,
    pub deadline: u64,
    pub current_phase: String,
    pub parent_chain_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiToolRequest {
    pub tool: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiResponseMessage {
    pub request_id: u64,
    pub status: String,
    pub output: String,
    pub output_hash: String,
    pub runtime_ms: u64,
    pub error: String,
    pub tool_requests: Vec<AiToolRequest>,
}

pub fn encode_request(msg: &AiRequestMessage) -> String {
    let tools = msg
        .allowed_tools
        .iter()
        .map(|t| format!("\"{}\"", json_escape(t)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"request_id\":{},\"process_id\":{},\"model\":\"{}\",\"input\":\"{}\",\"input_hash\":\"{}\",\"allowed_tools\":[{}],\"deadline\":{},\"current_phase\":\"{}\",\"parent_chain_hash\":\"{}\"}}\n",
        msg.request_id,
        msg.process_id,
        json_escape(&msg.model),
        json_escape(&msg.input),
        json_escape(&msg.input_hash),
        tools,
        msg.deadline,
        json_escape(&msg.current_phase),
        json_escape(&msg.parent_chain_hash),
    )
}

pub fn encode_tool_result(request_id: u64, tool: &str, result: &str) -> String {
    format!(
        "{{\"request_id\":{},\"tool_result\":\"{}\",\"tool\":\"{}\",\"output\":\"{}\"}}\n",
        request_id,
        json_escape(tool),
        json_escape(tool),
        json_escape(result),
    )
}

pub fn decode_response(line: &str) -> Result<AiResponseMessage, AiError> {
    let line = line.trim();
    if !line.starts_with('{') || !line.ends_with('}') {
        return Err(AiError::MalformedResponse);
    }
    let request_id = parse_u64_field(line, "request_id").ok_or(AiError::MalformedResponse)?;
    let status = parse_string_field(line, "status").unwrap_or_else(|| "ok".into());
    let output = parse_string_field(line, "output").unwrap_or_default();
    let output_hash = parse_string_field(line, "output_hash").unwrap_or_default();
    let runtime_ms = parse_u64_field(line, "runtime").unwrap_or(0);
    let error = parse_string_field(line, "error").unwrap_or_default();
    let tool_requests = parse_tool_requests(line);
    Ok(AiResponseMessage {
        request_id,
        status,
        output,
        output_hash,
        runtime_ms,
        error,
        tool_requests,
    })
}

fn parse_tool_requests(line: &str) -> Vec<AiToolRequest> {
    let mut out = Vec::new();
    let Some(start) = line.find("\"tool_requests\"") else {
        return out;
    };
    let rest = &line[start..];
    let Some(arr_start) = rest.find('[') else {
        return out;
    };
    let Some(arr_end) = rest[arr_start..].find(']') else {
        return out;
    };
    let inner = &rest[arr_start + 1..arr_start + arr_end];
    for part in inner.split("},") {
        let tool = parse_string_field(part, "tool").unwrap_or_default();
        if tool.is_empty() {
            continue;
        }
        let arguments = parse_string_field(part, "arguments").unwrap_or_else(|| "none".into());
        out.push(AiToolRequest { tool, arguments });
    }
    out
}

fn parse_u64_field(json: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\":");
    let idx = json.find(&needle)?;
    let rest = json[idx + needle.len()..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn parse_string_field(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    let idx = json.find(&needle)?;
    let rest = &json[idx + needle.len()..];
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => out.push(other),
                None => break,
            }
        } else if ch == '"' {
            break;
        } else {
            out.push(ch);
        }
    }
    Some(out)
}

fn json_escape(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push('?'),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_request_encode() {
        let msg = AiRequestMessage {
            request_id: 42,
            process_id: 7,
            model: "default".into(),
            input: "hello".into(),
            input_hash: "abc".into(),
            allowed_tools: vec!["system.state".into()],
            deadline: 5000,
            current_phase: "Phi1".into(),
            parent_chain_hash: "00".into(),
        };
        let encoded = encode_request(&msg);
        assert!(encoded.contains("\"request_id\":42"));
    }

    #[test]
    fn decode_response_basic() {
        let line = r#"{"request_id":42,"status":"ok","output":"done","output_hash":"91ac","runtime":12,"error":"","tool_requests":[]}"#;
        let resp = decode_response(line).unwrap();
        assert_eq!(resp.request_id, 42);
        assert_eq!(resp.output, "done");
    }
}
