//! about — print The Zen of Golden Phase Operating System.

use crate::platform::String;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;
use crate::userland::splash;

pub fn zen_text() -> String {
    format!(
        "\
The Zen of Golden Phase Operating System
gφlden phase φpertating system

State is better than status.
Explicit Kairos State is better than implicit mode.
Simple transitions are better than complex policy.
Traceable is better than trustworthy.
Transparency is better than obscuration.
Unless privacy is involved.
Readability counts - in the log, not just the source.
If the path is hard to explain, it is a bad path.
One obvious way to commit: composition has law.
Errors should never pass silently - log them in the chain.
Although practicality beats purity, topology beats guesswork.
Math is the formulation of truth. Remember our truth.
{}
",
        splash::equation_line()
    )
}

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln(&format!("{}\n", zen_text()))?;
    Ok(())
}
