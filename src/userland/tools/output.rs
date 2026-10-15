//! Tool output abstraction.

use crate::kernel::syscall::IoBuffer;
use crate::platform::String;
use crate::userland::tools::error::ToolError;

pub trait ToolOutput {
    fn write_str(&mut self, text: &str) -> Result<(), ToolError>;
    fn write_line(&mut self, text: &str) -> Result<(), ToolError> {
        self.write_str(&format!("{text}\n"))
    }
}

pub struct IoToolOutput<'a> {
    io: &'a mut IoBuffer,
    limit: usize,
    written: usize,
}

impl<'a> IoToolOutput<'a> {
    pub fn new(io: &'a mut IoBuffer, limit: usize) -> Self {
        Self {
            io,
            limit,
            written: 0,
        }
    }
}

impl ToolOutput for IoToolOutput<'_> {
    fn write_str(&mut self, text: &str) -> Result<(), ToolError> {
        self.try_write_str(text)
    }
}

impl IoToolOutput<'_> {
    pub fn try_write_str(&mut self, text: &str) -> Result<(), ToolError> {
        if self.written.saturating_add(text.len()) > self.limit {
            return Err(ToolError::OutputLimitExceeded);
        }
        if let Some(last) = self.io.output.last_mut() {
            last.push_str(text);
        } else {
            self.io.output.push(text.into());
        }
        self.written = self.written.saturating_add(text.len());
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct CaptureOutput {
    pub buffer: String,
    pub limit: usize,
}

impl CaptureOutput {
    pub fn new(limit: usize) -> Self {
        Self {
            buffer: String::new(),
            limit,
        }
    }
}

impl ToolOutput for CaptureOutput {
    fn write_str(&mut self, text: &str) -> Result<(), ToolError> {
        if self.buffer.len() + text.len() > self.limit {
            return Err(ToolError::OutputLimitExceeded);
        }
        self.buffer.push_str(text);
        Ok(())
    }
}
