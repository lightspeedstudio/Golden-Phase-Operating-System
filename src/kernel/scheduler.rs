//! Scheduler over phase-state processes; contention uses core compose.

use crate::core::gphase_core::PhaseState;
use crate::kernel::event_log::{ProcessId, TopologyEvent};
use crate::kernel::phase_consensus::PhaseConsensus;
use crate::kernel::process::Process;
use crate::platform::{HashMap, Vec};

pub struct Scheduler {
    processes: HashMap<ProcessId, Process>,
    next_pid: ProcessId,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            next_pid: 0,
        }
    }

    pub fn spawn(&mut self) -> ProcessId {
        self.spawn_named("")
    }

    pub fn spawn_named(&mut self, name: impl Into<crate::platform::String>) -> ProcessId {
        let pid = self.next_pid;
        self.next_pid += 1;
        self.processes
            .insert(pid, Process::with_name(pid, PhaseState::Phi0, name));
        pid
    }

    pub fn add_process(&mut self, pid: ProcessId) -> Result<&mut Process, &'static str> {
        if self.processes.contains_key(&pid) {
            return Err("process already exists");
        }
        self.processes
            .insert(pid, Process::new(pid, PhaseState::Phi0));
        if pid >= self.next_pid {
            self.next_pid = pid + 1;
        }
        Ok(self.processes.get_mut(&pid).expect("just inserted"))
    }

    pub fn get_process(&self, pid: ProcessId) -> Result<&Process, &'static str> {
        self.processes.get(&pid).ok_or("no such process")
    }

    pub fn get_process_mut(&mut self, pid: ProcessId) -> Result<&mut Process, &'static str> {
        self.processes.get_mut(&pid).ok_or("no such process")
    }

    pub fn wake(&mut self, pid: ProcessId) -> Result<bool, &'static str> {
        Ok(self.get_process_mut(pid)?.transition(PhaseState::Phi1))
    }

    pub fn schedule(&mut self, pid: ProcessId) -> Result<bool, &'static str> {
        Ok(self.get_process_mut(pid)?.transition(PhaseState::Phi2))
    }

    pub fn preempt(&mut self, pid: ProcessId) -> Result<bool, &'static str> {
        Ok(self.get_process_mut(pid)?.transition(PhaseState::Phi1))
    }

    pub fn syscall(&mut self, pid: ProcessId) -> Result<bool, &'static str> {
        Ok(self.get_process_mut(pid)?.transition(PhaseState::Phi3))
    }

    pub fn kreturn(&mut self, pid: ProcessId) -> Result<bool, &'static str> {
        Ok(self.get_process_mut(pid)?.transition(PhaseState::Phi1))
    }

    pub fn exit(&mut self, pid: ProcessId) -> Result<bool, &'static str> {
        Ok(self.get_process_mut(pid)?.transition(PhaseState::Phi0))
    }

    pub fn contention(
        &self,
        pid_a: ProcessId,
        pid_b: ProcessId,
        consensus: &PhaseConsensus,
    ) -> Result<PhaseState, &'static str> {
        let a = self.get_process(pid_a)?.state;
        let b = self.get_process(pid_b)?.state;
        match consensus.evaluate_compose(a, b) {
            Ok(ok) => Ok(ok.sw.compose),
            Err(_) => Err("phase consensus rejected"),
        }
    }

    pub fn topology_report(&self) -> Vec<TopologyEvent> {
        let mut events = Vec::new();
        let mut pids: Vec<_> = self.processes.keys().copied().collect();
        pids.sort_unstable();
        for pid in pids {
            if let Some(proc) = self.processes.get(&pid) {
                events.extend(proc.topology_events.iter().cloned());
            }
        }
        events
    }

    pub fn reflection_count(&self) -> usize {
        self.processes
            .values()
            .map(|p| p.topology_events.len())
            .sum()
    }

    pub fn process_ids(&self) -> Vec<ProcessId> {
        let mut pids: Vec<_> = self.processes.keys().copied().collect();
        pids.sort_unstable();
        pids
    }
}
