//! Simulated timer for preemptive scheduling in the hosted runtime.

#[derive(Debug, Clone)]
pub struct Timer {
    pub ticks: u64,
    pub interval: u64,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new(4)
    }
}

impl Timer {
    pub fn new(interval: u64) -> Self {
        Self {
            ticks: 0,
            interval: interval.max(1),
        }
    }

    /// Advance one tick. Returns true when a preemption boundary is reached.
    pub fn tick(&mut self) -> bool {
        self.ticks += 1;
        self.ticks.is_multiple_of(self.interval)
    }

    pub fn reset(&mut self) {
        self.ticks = 0;
    }
}
