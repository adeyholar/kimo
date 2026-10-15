//! Spectator telemetry hook, engine side.
//!
//! A host (kimo-mcp) may install one progress sink at startup; long-running tools call `progress` as they
//! advance. With no sink installed `progress` is a single `OnceLock` load and returns. Tools never learn
//! whether anyone is listening, and nothing here can change a tool's result.
use std::sync::OnceLock;

/// Receives (fraction 0..=1, short stage label) from the tool currently running.
pub type ProgressFn = Box<dyn Fn(f64, &str) + Send + Sync>;

static HOOK: OnceLock<ProgressFn> = OnceLock::new();

/// Install the process-wide progress sink. Returns false when one is already installed.
pub fn set_progress_hook(f: ProgressFn) -> bool { HOOK.set(f).is_ok() }

/// True once a sink is installed.
pub fn enabled() -> bool { HOOK.get().is_some() }

/// Report progress of the running tool. Free when no sink is installed.
#[inline]
pub fn progress(frac: f64, stage: &str) {
    if let Some(h) = HOOK.get() {
        let f = if frac.is_finite() { frac.clamp(0.0, 1.0) } else { 0.0 };
        h(f, stage)
    }
}
