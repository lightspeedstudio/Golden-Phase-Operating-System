//! Avenue 8 — causal-curvature watchdog: null-distribution measurement
//! harness (research tool, not part of the shipped OS).
//!
//! Drives the real hosted `Runtime` (`src/runtime/host.rs`) through four
//! workloads (idle, spawn-storm, VM-program loops, filesystem churn) and
//! records the per-tick reflection-defect delta
//! (`kernel.curvature.latest()`, wired up in `src/kernel/curvature.rs`,
//! sampled once per `Kernel::advance_chronos` boundary) for each tick.
//! Prints mean/variance/histogram per workload plus an injected-anomaly
//! demo and a false-positive-rate sweep over the clean runs, all as plain
//! text consumed by `docs/reports/AVENUE_08_curvature_watchdog.md`.
//!
//! Run: `cargo run --release --bin avenue08_curvature -- <ticks_per_workload>`
//! (defaults to 20000 if omitted).

use kairos::userland::run::run_program;
use kairos::userland::syscall;
use kairos::Runtime;
use std::env;
use std::time::Instant;

const Z_ALERT_THRESHOLD: f64 = 3.29;

fn mean_var(samples: &[u32]) -> (f64, f64) {
    let n = samples.len() as f64;
    if n == 0.0 {
        return (0.0, 0.0);
    }
    let sum: f64 = samples.iter().map(|&v| v as f64).sum();
    let mean = sum / n;
    let var = samples
        .iter()
        .map(|&v| {
            let d = v as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / n;
    (mean, var)
}

fn histogram(samples: &[u32]) -> Vec<(u32, usize)> {
    let max = samples.iter().copied().max().unwrap_or(0);
    let mut counts = vec![0usize; (max as usize) + 1];
    for &v in samples {
        counts[v as usize] += 1;
    }
    counts
        .into_iter()
        .enumerate()
        .map(|(k, c)| (k as u32, c))
        .filter(|(_, c)| *c > 0)
        .collect()
}

fn print_workload_report(name: &str, samples: &[u32], elapsed_secs: f64) {
    let (mean, var) = mean_var(samples);
    let std = var.sqrt();
    let n = samples.len();
    let zero_frac = samples.iter().filter(|&&v| v == 0).count() as f64 / n as f64;
    println!("### workload: {name}");
    println!(
        "ticks: {n}  wall_time_s: {elapsed_secs:.2}  ticks_per_s: {:.1}",
        n as f64 / elapsed_secs.max(1e-9)
    );
    println!("mean: {mean:.6}  variance: {var:.6}  std: {std:.6}  frac_zero: {zero_frac:.4}");
    println!("histogram (defects_per_tick -> count):");
    for (k, c) in histogram(samples) {
        let bar_len = ((c as f64 / n as f64) * 100.0).round() as usize;
        let bar: String = std::iter::repeat('#').take(bar_len.min(80)).collect();
        println!(
            "  {k:>3} | {c:>8} ({:>6.2}%) {bar}",
            100.0 * c as f64 / n as f64
        );
    }
    println!();
}

/// Leave-one-out rolling z-score over a fixed-size trailing window, mirroring
/// `kernel::curvature::CurvatureLog::rolling_z` exactly (re-implemented here
/// so this harness can sweep the full offline sample, not just the live
/// 64-sample window kept in the running kernel).
fn rolling_z_series(samples: &[u32], window: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(samples.len());
    for i in 0..samples.len() {
        if i + 1 < window {
            continue;
        }
        let start = i + 1 - window;
        let slice = &samples[start..=i];
        let latest = *slice.last().unwrap() as f64;
        let rest = &slice[..slice.len() - 1];
        let n = rest.len() as f64;
        let mean = rest.iter().map(|&v| v as f64).sum::<f64>() / n;
        let var = rest
            .iter()
            .map(|&v| {
                let d = v as f64 - mean;
                d * d
            })
            .sum::<f64>()
            / n;
        let std = var.sqrt();
        let z = if std < 1e-9 {
            if (latest - mean).abs() < 1e-9 {
                0.0
            } else if latest > mean {
                1.0e6
            } else {
                -1.0e6
            }
        } else {
            (latest - mean) / std
        };
        out.push(z);
    }
    out
}

fn false_positive_rate(samples: &[u32], window: usize, threshold: f64) -> (usize, usize, f64) {
    let zs = rolling_z_series(samples, window);
    let flagged = zs.iter().filter(|z| z.abs() >= threshold).count();
    let total = zs.len();
    (flagged, total, flagged as f64 / total.max(1) as f64)
}

/// Workload 1: idle. No shell activity at all -- just timer-driven
/// preemption of whatever is running (in practice, just `init`).
fn run_idle(ticks: usize) -> Vec<u32> {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let mut out = Vec::with_capacity(ticks);
    for _ in 0..ticks {
        rt.force_timer_tick().unwrap();
        out.push(rt.hosted.kernel.curvature.latest().unwrap_or(0));
    }
    out
}

/// Workload 2: spawn-storm. Every tick spawns a new background process;
/// once the live population exceeds a small cap, every tick also schedules
/// and kills the oldest still-alive non-init process (a full
/// create -> run -> teardown cycle). The cap keeps the process table (and
/// therefore per-tick cost) roughly constant instead of growing without
/// bound over a long run.
fn run_spawn_storm(ticks: usize) -> Vec<u32> {
    const POPULATION_CAP: usize = 8;
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let init = rt.hosted.init_pid.unwrap();
    let mut alive: Vec<u32> = Vec::new();
    let mut out = Vec::with_capacity(ticks);
    for _ in 0..ticks {
        if let Ok(child) = rt.spawn_background() {
            alive.push(child);
        }
        if alive.len() > POPULATION_CAP {
            let target = alive.remove(0);
            let _ = rt.hosted.kernel.schedule(target);
            let _ = rt.hosted.kernel.kill_process(target);
        }
        // one advance_chronos boundary per tick, same cadence as the other
        // shell-driven workloads (run_command calls advance_chronos once).
        rt.hosted.kernel.advance_chronos();
        out.push(rt.hosted.kernel.curvature.latest().unwrap_or(0));
        let _ = init; // keep init pid alive/referenced for clarity
    }
    out
}

/// Workload 3: VM-program loops. Repeatedly loads and runs the `hello`
/// program (spawn -> run VM -> syscall Print -> syscall Exit) via the same
/// `run_program` path the `run hello` shell command uses.
fn run_vm_loops(ticks: usize) -> Vec<u32> {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let init = rt.hosted.init_pid.unwrap();
    let mut out = Vec::with_capacity(ticks);
    for _ in 0..ticks {
        syscall::ensure_running(&mut rt.hosted.kernel, init).unwrap();
        let _ = run_program(
            &mut rt.hosted.kernel,
            init,
            &mut rt.hosted.io,
            &rt.hosted.programs,
            "hello",
            &mut rt.hosted.vms,
        );
        rt.hosted.kernel.advance_chronos();
        out.push(rt.hosted.kernel.curvature.latest().unwrap_or(0));
    }
    out
}

/// Workload 4: filesystem churn. Cycles write / cat / rm on a fixed path
/// via the real shell command path (`run_command`), which is what the
/// `fs_cmd` module (and therefore each syscall's enter/leave-kernel phase
/// transition) actually runs in the product.
fn run_fs_churn(ticks: usize) -> Vec<u32> {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let mut out = Vec::with_capacity(ticks);
    for i in 0..ticks {
        let cmd = match i % 3 {
            0 => "write /tmp/churn.txt payload".to_string(),
            1 => "cat /tmp/churn.txt".to_string(),
            _ => "rm /tmp/churn.txt".to_string(),
        };
        let _ = rt.run_command(&cmd);
        out.push(rt.hosted.kernel.curvature.latest().unwrap_or(0));
    }
    out
}

/// Injected-anomaly demo: run a short clean baseline on a fresh runtime,
/// then force a genuine burst of reflection defects via the same mechanism
/// as the `curvature inject` shell command (real schedule/syscall/kreturn
/// cycles, not a fabricated counter bump), and show the rolling z-score
/// catching it.
fn run_injected_anomaly_demo() {
    println!("### injected anomaly demo (idle baseline + forced burst)");
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let mut samples: Vec<u32> = Vec::new();
    for _ in 0..80 {
        rt.force_timer_tick().unwrap();
        samples.push(rt.hosted.kernel.curvature.latest().unwrap_or(0));
    }
    let (mean, var) = mean_var(&samples);
    println!(
        "baseline (80 idle ticks): mean={mean:.4} std={:.4}",
        var.sqrt()
    );

    // Force the burst: same mechanism as `curvature inject` (real
    // schedule -> syscall -> kreturn cycles on a scratch process).
    let pid = rt.hosted.kernel.spawn_process("anomaly-inject");
    let before = rt.hosted.kernel.scheduler.reflection_count();
    let _ = rt.hosted.kernel.wake(pid);
    for _ in 0..20 {
        let _ = rt.hosted.kernel.schedule(pid);
        let _ = rt.hosted.kernel.syscall(pid);
        let _ = rt.hosted.kernel.kreturn(pid);
    }
    let _ = rt.hosted.kernel.exit(pid);
    let after = rt.hosted.kernel.scheduler.reflection_count();
    rt.hosted.kernel.advance_chronos();
    let spike = rt.hosted.kernel.curvature.latest().unwrap_or(0);
    samples.push(spike);
    println!(
        "injected burst: pid={pid} reps=20 reflection_defects_generated={}",
        after - before
    );
    println!("tick after burst: defects={spike}");

    let zs = rolling_z_series(&samples, 64);
    let last_z = *zs.last().unwrap();
    println!(
        "rolling z-score on burst tick: {last_z:.3} (threshold |z|>={Z_ALERT_THRESHOLD:.2}) -> {}",
        if last_z.abs() >= Z_ALERT_THRESHOLD {
            "CAUGHT"
        } else {
            "MISSED"
        }
    );
    println!();
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let ticks: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20_000);

    println!("# Avenue 8 curvature watchdog -- raw measurement dump");
    println!("ticks_per_workload = {ticks}");
    println!();

    let workloads: [(&str, fn(usize) -> Vec<u32>); 4] = [
        ("idle", run_idle),
        ("spawn_storm", run_spawn_storm),
        ("vm_program_loops", run_vm_loops),
        ("filesystem_churn", run_fs_churn),
    ];

    let mut all_samples: Vec<(&str, Vec<u32>)> = Vec::new();
    for (name, f) in workloads {
        let start = Instant::now();
        let samples = f(ticks);
        let elapsed = start.elapsed().as_secs_f64();
        print_workload_report(name, &samples, elapsed);
        all_samples.push((name, samples));
    }

    run_injected_anomaly_demo();

    println!("### false-positive rate on clean workloads (rolling window=64, |z|>={Z_ALERT_THRESHOLD:.2})");
    for (name, samples) in &all_samples {
        let (flagged, total, rate) = false_positive_rate(samples, 64, Z_ALERT_THRESHOLD);
        println!("  {name:<18} flagged={flagged:>6} / total={total:>6}  fpr={rate:.6}");
    }
}
