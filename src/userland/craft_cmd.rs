//! Shell commands for Avenue 10 craft fixture (`--features craft`).

use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;
use core::cell::RefCell;

#[cfg(feature = "craft")]
use crate::kernel::craft::{
    prop_stuck_over_glider_scenario, prop_stuck_scenario, CraftCommand, CraftController, CraftKind,
    FdirController,
};

#[cfg(feature = "craft")]
thread_local! {
    static CRAFT: RefCell<Option<CraftController>> = RefCell::new(None);
}

#[cfg(feature = "craft")]
fn with_craft<F, R>(f: F) -> Result<R, ShellError>
where
    F: FnOnce(&mut CraftController) -> Result<R, ShellError>,
{
    CRAFT.with(|cell| {
        let mut slot = cell.borrow_mut();
        let c = slot
            .as_mut()
            .ok_or_else(|| ShellError::Unknown("craft not initialized — run: craft init".into()))?;
        f(c)
    })
}

#[cfg(not(feature = "craft"))]
fn need_feature() -> ShellError {
    ShellError::Unknown("craft commands require --features craft".into())
}

pub fn run_init(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(feature = "craft")]
    {
        let mut c = CraftController::new();
        c.init_craft()
            .map_err(|e| ShellError::Unknown(e.as_str()))?;
        CRAFT.with(|cell| *cell.borrow_mut() = Some(c));
        ctx.writeln("CRAFT INIT\n")?;
        Ok(())
    }
    #[cfg(not(feature = "craft"))]
    {
        let _ = ctx;
        Err(need_feature())
    }
}

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(feature = "craft")]
    {
        with_craft(|c| {
            ctx.writeln(&c.status_report())?;
            Ok(())
        })
    }
    #[cfg(not(feature = "craft"))]
    {
        let _ = ctx;
        Err(need_feature())
    }
}

pub fn run_tick(ctx: &mut ShellContext<'_>, n: u64) -> Result<(), ShellError> {
    #[cfg(feature = "craft")]
    {
        with_craft(|c| {
            let notes = c.tick(n).map_err(|e| ShellError::Unknown(e.as_str()))?;
            for n in notes {
                ctx.writeln(&format!("{n}\n"))?;
            }
            Ok(())
        })
    }
    #[cfg(not(feature = "craft"))]
    {
        let _ = (ctx, n);
        Err(need_feature())
    }
}

pub fn run_command(
    ctx: &mut ShellContext<'_>,
    subsystem: &str,
    operation: &str,
) -> Result<(), ShellError> {
    #[cfg(feature = "craft")]
    {
        with_craft(|c| {
            let next = c
                .command_str(subsystem, operation)
                .map_err(|e| ShellError::Unknown(e.as_str()))?;
            ctx.writeln(&format!(
                "OK {subsystem} {operation} -> Φ{}\n",
                next.as_u8()
            ))?;
            Ok(())
        })
    }
    #[cfg(not(feature = "craft"))]
    {
        let _ = (ctx, subsystem, operation);
        Err(need_feature())
    }
}

pub fn run_fault_inject(ctx: &mut ShellContext<'_>, reason: &str) -> Result<(), ShellError> {
    #[cfg(feature = "craft")]
    {
        with_craft(|c| {
            c.fault_inject_prop_stuck(reason)
                .map_err(|e| ShellError::Unknown(e.as_str()))?;
            ctx.writeln(&format!("FAULT INJECT prop-stuck reason={reason}\n"))?;
            Ok(())
        })
    }
    #[cfg(not(feature = "craft"))]
    {
        let _ = (ctx, reason);
        Err(need_feature())
    }
}

pub fn run_quarantine_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(feature = "craft")]
    {
        with_craft(|c| {
            ctx.writeln(&c.quarantine_status())?;
            Ok(())
        })
    }
    #[cfg(not(feature = "craft"))]
    {
        let _ = ctx;
        Err(need_feature())
    }
}

pub fn run_episode(ctx: &mut ShellContext<'_>, episode_id: u64) -> Result<(), ShellError> {
    #[cfg(feature = "craft")]
    {
        with_craft(|c| {
            let ep = c
                .episodes
                .iter()
                .find(|e| e.id == episode_id)
                .ok_or_else(|| ShellError::Unknown("episode not found".into()))?;
            ctx.writeln(&format!(
                "episode={} object={} reason={} dwell={} thr={} z={:.3} tick={}\n",
                ep.id,
                ep.object_id,
                ep.reason,
                ep.dwell,
                ep.threshold,
                ep.curvature_z,
                ep.detected_tick
            ))?;
            Ok(())
        })
    }
    #[cfg(not(feature = "craft"))]
    {
        let _ = (ctx, episode_id);
        Err(need_feature())
    }
}

pub fn run_replay(ctx: &mut ShellContext<'_>, episode_id: u64) -> Result<(), ShellError> {
    #[cfg(feature = "craft")]
    {
        with_craft(|c| {
            let snap = c
                .replay_episode(episode_id)
                .map_err(|e| ShellError::Unknown(e.as_str()))?;
            ctx.writeln(&format!("REPLAY OK {snap:?}\n"))?;
            Ok(())
        })
    }
    #[cfg(not(feature = "craft"))]
    {
        let _ = (ctx, episode_id);
        Err(need_feature())
    }
}

pub fn run_scenario_prop_stuck(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(feature = "craft")]
    {
        let mut c = CraftController::new();
        let report = prop_stuck_scenario(&mut c).map_err(|e| ShellError::Unknown(e.as_str()))?;
        CRAFT.with(|cell| *cell.borrow_mut() = Some(c));
        ctx.writeln(&report)?;
        Ok(())
    }
    #[cfg(not(feature = "craft"))]
    {
        let _ = ctx;
        Err(need_feature())
    }
}

pub fn run_scenario_prop_stuck_glider(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(feature = "craft")]
    {
        let mut c = CraftController::new_over_glider().map_err(ShellError::Unknown)?;
        let report =
            prop_stuck_over_glider_scenario(&mut c).map_err(|e| ShellError::Unknown(e.as_str()))?;
        CRAFT.with(|cell| *cell.borrow_mut() = Some(c));
        ctx.writeln(&report)?;
        Ok(())
    }
    #[cfg(not(feature = "craft"))]
    {
        let _ = ctx;
        Err(need_feature())
    }
}

#[cfg(feature = "craft")]
#[allow(dead_code)]
fn _keep_imports() {
    let _ = (
        CraftCommand::PropArm,
        CraftKind::Propulsion,
        FdirController::status_line,
    );
}
