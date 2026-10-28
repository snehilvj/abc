//! Logs: abcd's `abcd.log` and abc's `abc.log` in the profile's state
//! folder, each keeping the previous run's as `.1`, filtered by the
//! `server.log` setting (or `ABC_LOG`), which can change while they run.

use std::fs;
use std::io;
use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Registry, reload};

/// The log, while abcd runs: dropping it flushes what's left.
pub struct Log {
    _guard: WorkerGuard,
    filter: reload::Handle<EnvFilter, Registry>,
}

impl Log {
    /// Filters by `filter` from now on, such as "info" or "abcd=debug".
    pub fn set_filter(&self, filter: &str) {
        let _ = self.filter.reload(parse(filter));
    }
}

/// A filter as written, or `info` if it can't be read.
fn parse(filter: &str) -> EnvFilter {
    EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("info"))
}

/// Starts logging to `log`, keeping the last run's beside it.
pub fn start(log: &Path, filter: &str) -> io::Result<Log> {
    if log.exists() {
        let mut previous = log.as_os_str().to_owned();
        previous.push(".1");
        fs::rename(log, previous)?;
    }
    let file = fs::File::create(log)?;
    let (writer, guard) = tracing_appender::non_blocking(file);
    let (filter, handle) = reload::Layer::new(parse(filter));
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false),
        )
        .init();
    Ok(Log {
        _guard: guard,
        filter: handle,
    })
}
