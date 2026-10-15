//! StateClock-oriented queue items.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockWorkItem {
    pub proper_tick: u64,
    pub kairos_tick: u64,
    pub sector_id: u8,
}
