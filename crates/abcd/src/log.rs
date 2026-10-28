//! abcd's log: `abcd.log` in the profile's state folder, the previous run's
//! kept as `abcd.log.1`. `ABC_LOG` sets the filter, `info` by default.

use std::fs;

use abc_core::paths::Paths;
use anyhow::Context as _;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// Starts logging to the profile's log file. Keep the guard: dropping it
/// flushes what's left.
pub fn start(paths: &Paths) -> anyhow::Result<WorkerGuard> {
    let log = paths.server_log();
    if log.exists() {
        let mut previous = log.clone().into_os_string();
        previous.push(".1");
        fs::rename(&log, previous).context("keeping the last log")?;
    }
    let file = fs::File::create(&log).with_context(|| format!("creating {}", log.display()))?;
    let (writer, guard) = tracing_appender::non_blocking(file);
    let filter = EnvFilter::try_from_env("ABC_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .init();
    Ok(guard)
}
