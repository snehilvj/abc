//! abc: the terminal client and CLI.

use std::process::ExitCode;

use abc::cli;
use abc_core::paths::{Paths, Profile};
use anyhow::Context as _;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "abc",
    version,
    about = "A terminal workspace for many repos and git worktrees"
)]
#[command(arg_required_else_help = true)]
struct Cli {
    /// The profile to use, else ABC_PROFILE, else stable. Profiles share nothing.
    #[arg(long, global = true)]
    profile: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// The background server, abcd.
    #[command(subcommand)]
    Server(cli::Server),
    /// Show abcd's log, or abc's.
    Logs(cli::Logs),
    /// Open a screen with fake data. No server needed.
    Gallery(abc::gallery::Args),
}

fn main() -> anyhow::Result<ExitCode> {
    let Cli { profile, command } = Cli::parse();
    let paths = || -> anyhow::Result<Paths> {
        let profile = Profile::choose(profile.as_deref(), |var| std::env::var_os(var))?;
        Ok(Paths::from_env(profile)?)
    };
    match command {
        Command::Gallery(gallery) => abc::gallery::run(gallery).map(|()| ExitCode::SUCCESS),
        Command::Logs(logs) => cli::logs(&logs, &paths()?),
        Command::Server(server) => tokio::runtime::Runtime::new()
            .context("starting the runtime")?
            .block_on(cli::server(server, &paths()?)),
    }
}
