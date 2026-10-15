//! TCP client for external AI inference server (hosted std only).

use crate::kernel::ai::error::AiError;
use crate::kernel::ai::protocol::{
    decode_response, encode_request, AiRequestMessage, AiResponseMessage,
};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

pub fn connect_tcp(host: &str, port: u16, timeout_ms: u64) -> Result<TcpStream, AiError> {
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e: std::net::AddrParseError| AiError::ConnectionFailed(e.to_string()))?;
    let stream = TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms))
        .map_err(|e| AiError::ConnectionFailed(e.to_string()))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .map_err(|e| AiError::ConnectionFailed(e.to_string()))?;
    stream
        .set_write_timeout(Some(Duration::from_millis(timeout_ms)))
        .map_err(|e| AiError::ConnectionFailed(e.to_string()))?;
    Ok(stream)
}

pub fn send_request(
    host: &str,
    port: u16,
    request: &AiRequestMessage,
    timeout_ms: u64,
    max_response_bytes: usize,
) -> Result<AiResponseMessage, AiError> {
    let mut stream = connect_tcp(host, port, timeout_ms)?;
    let payload = encode_request(request);
    stream
        .write_all(payload.as_bytes())
        .map_err(|e| AiError::ConnectionFailed(e.to_string()))?;
    let mut buf = vec![0u8; max_response_bytes + 1];
    let n = stream.read(&mut buf).map_err(|_| AiError::RequestTimeout)?;
    if n > max_response_bytes {
        return Err(AiError::ResponseTooLarge);
    }
    let line = core::str::from_utf8(&buf[..n]).map_err(|_| AiError::MalformedResponse)?;
    let first_line = line.lines().next().unwrap_or(line);
    decode_response(first_line)
}

pub fn verify_connection(host: &str, port: u16, timeout_ms: u64) -> Result<(), AiError> {
    let stream = connect_tcp(host, port, timeout_ms)?;
    drop(stream);
    Ok(())
}
