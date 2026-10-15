//! `mmuiso` shell — mapping summary and isolation matrix.

use crate::platform::String;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run(ctx: &mut ShellContext<'_>, args: &[String]) -> Result<(), ShellError> {
    #[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
    {
        dispatch(ctx, args)
    }
    #[cfg(not(all(target_arch = "arm", feature = "mmu-app-isolation")))]
    {
        let _ = args;
        ctx.writeln_line("mmuiso unavailable (need ARM mmu-app-isolation)")?;
        Ok(())
    }
}

#[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
fn dispatch(ctx: &mut ShellContext<'_>, args: &[String]) -> Result<(), ShellError> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
    match sub {
        "status" => ctx.writeln_line(&crate::arch::armv7::isolation::status_lines())?,
        "selftest" | "matrix" => {
            let report = crate::arch::armv7::isolation::run_matrix(ctx.kernel);
            ctx.writeln_line(&report)?;
        }
        other => {
            match crate::arch::armv7::isolation::run_named(ctx.kernel, other, None) {
                Ok(true) => {
                    ctx.writeln_line(&alloc::format!("{other}: PASS"))?;
                }
                Ok(false) => {
                    ctx.writeln_line(&alloc::format!("{other}: FAIL"))?;
                }
                Err(e) => ctx.writeln_line(e)?,
            }
        }
    }
    Ok(())
}
