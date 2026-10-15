//! run — paste-and-run / file-run through the assembler and PhaseVM.

use crate::isa::assemble_instructions;
use crate::userland::context::ShellContext;
use crate::userland::run;
use crate::userland::shell::ShellError;

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln_line("paste Golden Phase program, end with .end")?;
    let mut source = crate::platform::String::new();
    while let Some(line) = ctx.io.read_line() {
        if line.trim() == ".end" {
            break;
        }
        source.push_str(&line);
        source.push('\n');
    }
    execute_source(ctx, "_paste", &source)
}

pub fn run_file(ctx: &mut ShellContext<'_>, filename: &str) -> Result<(), ShellError> {
    match ctx.kernel.fs.read_file(filename) {
        Ok(bytes) => {
            let source = core::str::from_utf8(&bytes).unwrap_or("");
            execute_source(ctx, filename, source)
        }
        Err(err) => {
            ctx.writeln_line(&format!("run file: {err}"))?;
            Ok(())
        }
    }
}

fn execute_source(ctx: &mut ShellContext<'_>, name: &str, source: &str) -> Result<(), ShellError> {
    let program = match assemble_instructions(source) {
        Ok(p) => p,
        Err(err) => {
            ctx.writeln_line(&format!("asm: {err}"))?;
            return Ok(());
        }
    };
    match run::run_assembled(
        ctx.kernel,
        ctx.caller,
        ctx.io,
        name,
        &program,
        ctx.vms,
    ) {
        Ok(_) => Ok(()),
        Err(err) => {
            ctx.writeln_line(&format!("{err}"))?;
            Ok(())
        }
    }
}
