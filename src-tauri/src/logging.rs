//! Process-wide `tracing` setup: stdout plus a rotating on-disk log file.
//!
//! DEC-018 requires local logs to always be written to a rotating file in
//! the app data directory — a bundled desktop app launched from a menu or
//! dock has no terminal attached, so stdout-only logging is invisible to
//! the person filing a bug report.
//!
//! This runs *before* Tauri's `App` exists, so the app data directory
//! can't come from `app.path().app_data_dir()`. It's reconstructed here
//! from the same two ingredients Tauri uses (`dirs::data_dir()` joined
//! with the bundle identifier from `tauri.conf.json`), so the log lands
//! next to `polyvocal.db` rather than in a second, divergent location.
//! Doing it this way — rather than deferring setup into `.setup()` —
//! keeps the very first startup line in the file.
//!
//! Privacy (DEC-018): nothing written here is transcript content, audio
//! data, or a file path; that constraint is on the call sites, not on
//! this module, but the log file is persistent on-disk state, so it's
//! worth re-stating where the file is created.

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use tracing::warn;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

/// Log files are `polyvocal.<YYYY-MM-DD>.log` in `<app data>/logs`.
const FILENAME_PREFIX: &str = "polyvocal";
const FILENAME_SUFFIX: &str = "log";

/// A week of daily files — enough history for a bug report to still
/// contain the failure, bounded so an idle install can't grow unbounded.
const MAX_LOG_FILES: usize = 7;

/// Default filter, overridable via `RUST_LOG`.
const DEFAULT_FILTER: &str = "polyvocal=debug";

/// Directory the rotating log files live in, for a given bundle identifier.
///
/// Mirrors `tauri::path::PathResolver::app_data_dir()` (`dirs::data_dir()`
/// joined with the identifier) plus a `logs` subdirectory, so the files sit
/// alongside the database instead of cluttering the app data root.
pub fn log_dir(identifier: &str) -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join(identifier).join("logs"))
}

/// Initialise the global subscriber: stdout always, plus a daily-rotating
/// file in the app data directory when one can be opened.
///
/// Returns the [`WorkerGuard`] for the file writer's background thread —
/// the caller must hold it for the lifetime of the process, or buffered
/// lines are lost on exit. `None` means the file writer couldn't be set
/// up (a warning is logged to stdout); logging still works otherwise, so
/// this is never fatal.
#[must_use = "dropping the guard stops the log file from being written"]
pub fn init(identifier: &str) -> Option<WorkerGuard> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    // Deliberately not reported until after `.init()` below — there is no
    // subscriber to `warn!` into yet, and this crate never uses `println!`.
    let (writer, guard, error) = match file_writer(identifier) {
        Ok((writer, guard)) => (Some(writer), Some(guard), None),
        Err(e) => (None, None, Some(e)),
    };

    // The `EnvFilter` sits on the registry, so `RUST_LOG` governs both
    // writers identically; the file layer drops ANSI colour codes, which
    // are noise in a pasted log.
    let file_layer = writer.map(|writer| fmt::layer().with_ansi(false).with_writer(writer));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(file_layer)
        .init();

    if let Some(e) = error {
        warn!("file logging disabled — could not open the log directory: {e}");
    }

    guard
}

/// Open (creating the directory if needed) the rotating log file.
fn file_writer(identifier: &str) -> Result<(NonBlocking, WorkerGuard)> {
    let dir = log_dir(identifier).ok_or_else(|| anyhow!("no platform data directory"))?;
    std::fs::create_dir_all(&dir)?;

    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(FILENAME_PREFIX)
        .filename_suffix(FILENAME_SUFFIX)
        .max_log_files(MAX_LOG_FILES)
        .build(&dir)?;

    Ok(tracing_appender::non_blocking(appender))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn log_dir_sits_under_the_bundle_identifier() {
        let Some(dir) = log_dir("com.polyvocal.app") else {
            return; // headless environment with no data dir — nothing to assert
        };

        assert!(dir.ends_with("com.polyvocal.app/logs"), "{}", dir.display());
        assert_eq!(
            dir.parent(),
            dirs::data_dir()
                .map(|d| d.join("com.polyvocal.app"))
                .as_deref(),
            "log dir must live inside the same app data dir Tauri resolves"
        );
    }

    /// The name the bug-report template tells people to look for has to be
    /// the name that actually appears on disk.
    #[test]
    fn appender_writes_a_dated_polyvocal_log_file() {
        let dir = std::env::temp_dir().join(format!("polyvocal-log-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix(FILENAME_PREFIX)
            .filename_suffix(FILENAME_SUFFIX)
            .max_log_files(MAX_LOG_FILES)
            .build(&dir)
            .unwrap();
        writeln!(appender, "hello").unwrap();
        appender.flush().unwrap();

        let names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(names[0].starts_with("polyvocal."), "{names:?}");
        assert!(names[0].ends_with(".log"), "{names:?}");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
