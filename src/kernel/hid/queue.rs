//! Bounded InputEvent ring. No allocation on push/pop.

use super::types::{HidError, InputEvent};

pub const INPUT_QUEUE_CAP: usize = 32;

#[derive(Clone, Copy)]
pub struct InputQueue {
    buf: [InputEvent; INPUT_QUEUE_CAP],
    head: u8,
    tail: u8,
    len: u8,
}

impl InputQueue {
    pub const fn new() -> Self {
        Self {
            buf: [InputEvent::DeviceDetached {
                device_id: 0,
                interface: 0,
            }; INPUT_QUEUE_CAP],
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
        self.len = 0;
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_full(&self) -> bool {
        self.len as usize == INPUT_QUEUE_CAP
    }

    /// Push newest. If full, leave the queue unchanged and return QueueOverflow.
    pub fn push(&mut self, ev: InputEvent) -> Result<(), HidError> {
        if self.is_full() {
            return Err(HidError::QueueOverflow);
        }
        self.buf[self.tail as usize] = ev;
        self.tail = (self.tail + 1) % INPUT_QUEUE_CAP as u8;
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<InputEvent> {
        if self.is_empty() {
            return None;
        }
        let ev = self.buf[self.head as usize];
        self.head = (self.head + 1) % INPUT_QUEUE_CAP as u8;
        self.len -= 1;
        Some(ev)
    }
}

static mut GLOBAL: InputQueue = InputQueue::new();

pub fn push_global(ev: InputEvent) -> Result<(), HidError> {
    unsafe { (*core::ptr::addr_of_mut!(GLOBAL)).push(ev) }
}

pub fn poll_global() -> Option<InputEvent> {
    unsafe { (*core::ptr::addr_of_mut!(GLOBAL)).pop() }
}

pub fn clear_global() {
    unsafe {
        (*core::ptr::addr_of_mut!(GLOBAL)).clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_and_overflow() {
        let mut q = InputQueue::new();
        for i in 0..INPUT_QUEUE_CAP {
            q.push(InputEvent::MouseWheel { delta: i as i8 })
                .unwrap();
        }
        assert_eq!(q.push(InputEvent::MouseWheel { delta: 99 }), Err(HidError::QueueOverflow));
        assert_eq!(q.len(), INPUT_QUEUE_CAP);
        assert_eq!(q.pop(), Some(InputEvent::MouseWheel { delta: 0 }));
        q.push(InputEvent::MouseWheel { delta: 32 }).unwrap();
        for i in 1..INPUT_QUEUE_CAP {
            assert_eq!(q.pop(), Some(InputEvent::MouseWheel { delta: i as i8 }));
        }
        assert_eq!(q.pop(), Some(InputEvent::MouseWheel { delta: 32 }));
        assert!(q.pop().is_none());
    }
}
