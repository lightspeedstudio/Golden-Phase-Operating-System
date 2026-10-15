//! version — build and feature information.

use crate::brand;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(all(feature = "std", not(feature = "baremetal")))]
    let hosted = true;
    #[cfg(not(all(feature = "std", not(feature = "baremetal"))))]
    let hosted = false;
    let target = brand::build_target_label(hosted);

    let text = format!(
        "{} {} ({target})\n{}\nKairos States kernel · research OS\n",
        brand::OS_NAME,
        env!("CARGO_PKG_VERSION"),
        brand::OS_STYLIZED,
    );
    ctx.writeln(&text)?;
    Ok(())
}
