//! Machine timer — software stub until UART shell smoke passes (no CLINT IRQ yet).

use core::sync::atomic::{AtomicBool, Ordering};

static PENDING: AtomicBool = AtomicBool::new(false);

pub fn init() {}

pub fn poll() {}

pub fn take_pending() -> bool {
    PENDING.swap(false, Ordering::SeqCst)
}

pub fn irq_fire() {
    PENDING.store(true, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_pending_round_trip() {
        assert!(!take_pending());
        irq_fire();
        assert!(take_pending());
        assert!(!take_pending());
    }
}
