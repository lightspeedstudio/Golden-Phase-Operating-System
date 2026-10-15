//! Shell output abstraction for local, SSH, and test transports.

use crate::kernel::syscall::IoBuffer;
use crate::platform::String;

pub trait ShellOutput {
    fn write_str(&mut self, text: &str);
    fn write_line(&mut self, text: &str);
}

impl ShellOutput for IoBuffer {
    fn write_str(&mut self, text: &str) {
        if let Some(last) = self.output.last_mut() {
            last.push_str(text);
        } else {
            self.output.push(text.into());
        }
    }

    fn write_line(&mut self, text: &str) {
        IoBuffer::write_line(self, text);
    }
}

#[derive(Debug, Default, Clone)]
pub struct TestBufferOutput {
    pub lines: Vec<String>,
}

impl TestBufferOutput {
    pub fn joined(&self) -> String {
        self.lines.join("")
    }
}

impl ShellOutput for TestBufferOutput {
    fn write_str(&mut self, text: &str) {
        if let Some(last) = self.lines.last_mut() {
            last.push_str(text);
        } else {
            self.lines.push(text.into());
        }
    }

    fn write_line(&mut self, text: &str) {
        self.lines.push(text.into());
    }
}

#[derive(Debug, Default)]
pub struct ConsoleOutput;

impl ShellOutput for ConsoleOutput {
    fn write_str(&mut self, text: &str) {
        #[cfg(feature = "std")]
        {
            use std::io::Write;
            let _ = std::io::stdout().write_all(text.as_bytes());
        }
        let _ = text;
    }

    fn write_line(&mut self, text: &str) {
        self.write_str(text);
        self.write_str("\n");
    }
}

#[cfg(feature = "ssh")]
#[derive(Debug, Default)]
pub struct SshChannelOutput {
    pub buffer: String,
}

#[cfg(feature = "ssh")]
impl SshChannelOutput {
    pub fn take(&mut self) -> String {
        let out = self.buffer.clone();
        self.buffer.clear();
        out
    }
}

#[cfg(feature = "ssh")]
impl ShellOutput for SshChannelOutput {
    fn write_str(&mut self, text: &str) {
        self.buffer.push_str(text);
    }

    fn write_line(&mut self, text: &str) {
        self.buffer.push_str(text);
        self.buffer.push('\n');
    }
}
