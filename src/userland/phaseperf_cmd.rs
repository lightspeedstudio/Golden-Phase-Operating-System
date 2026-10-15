//! Phase Math performance and verification commands.

use crate::kernel::phase_math::metrics::{render_queues, render_status};
use crate::kernel::phase_math::PhaseMathBackend;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln(&render_status(&ctx.kernel.phase_math.metrics))?;
    Ok(())
}

pub fn run_backend(ctx: &mut ShellContext<'_>, choice: &str) -> Result<(), ShellError> {
    if choice.is_empty() {
        ctx.writeln(&ctx.kernel.phase_math.backend_report())?;
        return Ok(());
    }
    let backend = match choice {
        "scalar" => PhaseMathBackend::Scalar,
        "avx2" => PhaseMathBackend::Avx2,
        "avx512" => PhaseMathBackend::Avx512,
        "neon" => PhaseMathBackend::Neon,
        "gpu" => PhaseMathBackend::Gpu,
        "fpga" => PhaseMathBackend::Fpga,
        _ => {
            ctx.writeln_line("usage: phaseperf backend [scalar|avx2|avx512|neon|gpu|fpga]")?;
            return Ok(());
        }
    };
    if backend != PhaseMathBackend::Scalar && !backend_present(backend) {
        ctx.writeln_line(&format!(
            "phaseperf backend {}: unavailable on this chip (Cortex-A9 / no AVX/GPU/FPGA engine) — active remains {}",
            backend.label(),
            ctx.kernel.phase_math.backend.label()
        ))?;
        return Ok(());
    }
    ctx.kernel.phase_math.force_backend(backend);
    ctx.writeln_line(&format!(
        "active backend: {}",
        ctx.kernel.phase_math.backend.label()
    ))?;
    Ok(())
}

fn backend_present(backend: PhaseMathBackend) -> bool {
    use crate::kernel::phase_math::backends;
    match backend {
        PhaseMathBackend::Scalar => true,
        PhaseMathBackend::Avx2 => backends::avx2::available(),
        PhaseMathBackend::Avx512 => backends::avx512::available(),
        PhaseMathBackend::Neon => backends::neon::available(),
        PhaseMathBackend::Gpu => backends::gpu::available(),
        PhaseMathBackend::Fpga => backends::fpga::available(),
    }
}

pub fn run_queues(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln(&render_queues(&ctx.kernel.phase_math.metrics))?;
    Ok(())
}

pub fn run_benchmark(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let out = ctx.kernel.phase_math.benchmark();
    ctx.writeln_line(&format!("phase results: {}", out.phase_results.len()))?;
    ctx.writeln_line(&format!("topology results: {}", out.topology_results.len()))?;
    ctx.writeln_line("benchmark: OK")?;
    Ok(())
}

pub fn run_verify(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let ok = ctx.kernel.phase_math.verify_scalar_equivalence();
    ctx.writeln_line(&format!(
        "phaseperf verify: backend=scalar {}",
        if ok { "MATCH" } else { "FAIL" }
    ))?;
    Ok(())
}
