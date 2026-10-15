//! Terminal I/O for the hosted runtime (scripted or interactive input).

use crate::kernel::syscall::IoBuffer;
use crate::platform::String;

pub struct Terminal {
    io: IoBuffer,
}

impl Default for Terminal {
    fn default() -> Self {
        Self::new()
    }
}

impl Terminal {
    pub fn new() -> Self {
        Self {
            io: IoBuffer::default(),
        }
    }

    pub fn with_input<I, S>(lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            io: IoBuffer::with_input(lines),
        }
    }

    pub fn from_io(io: IoBuffer) -> Self {
        Self { io }
    }

    pub fn io_mut(&mut self) -> &mut IoBuffer {
        &mut self.io
    }

    pub fn read_line(&mut self) -> Option<String> {
        self.io.read_line()
    }

    pub fn output_lines(&self) -> &[String] {
        &self.io.output
    }
}
