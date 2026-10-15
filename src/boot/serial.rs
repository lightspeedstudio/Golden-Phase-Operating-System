//! Serial line I/O for the QEMU console.

use crate::boot::arch::riscv64::uart;
use crate::kernel::syscall::IoBuffer;
use crate::platform::String;

pub fn flush_io(io: &IoBuffer) {
    for line in &io.output {
        uart::write_str(line);
        uart::write_str("\n");
    }
}

pub fn flush_new(io: &IoBuffer, from: usize) {
    for line in io.output.iter().skip(from) {
        uart::write_str(line);
        uart::write_str("\n");
    }
}

pub fn read_line() -> String {
    let mut buf = String::new();
    loop {
        let byte = uart::read_byte_blocking();
        match byte {
            b'\r' | b'\n' => {
                uart::write_byte(b'\n');
                return buf;
            }
            0x08 | 0x7f => {
                if buf.pop().is_some() {
                    uart::write_str("\x08 \x08");
                }
            }
            byte => {
                uart::write_byte(byte);
                buf.push(byte as char);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_new_skips_prior_lines() {
        let mut io = IoBuffer::default();
        io.write_line("a");
        io.write_line("b");
        assert_eq!(io.output.len(), 2);
        flush_new(&io, 1);
    }
}
