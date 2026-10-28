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
struct Cli {
    /// The profile to use, else ABC_PROFILE, else stable. Profiles share nothing.
    #[arg(long, global = true)]
    profile: Option<String>,
    /// Without one, abc opens.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// The repositories abc knows: list, add and remove them.
    #[command(subcommand)]
    Repo(cli::RepoCommand),
    /// The background server, abcd.
    #[command(subcommand)]
    Server(cli::Server),
    /// Settings: where they are, whether they're valid, and every option.
    #[command(subcommand)]
    Config(cli::Config),
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
    let Some(command) = command else {
        return abc::tui::run(paths()?).map(|()| ExitCode::SUCCESS);
    };
    match command {
        Command::Gallery(gallery) => abc::gallery::run(gallery).map(|()| ExitCode::SUCCESS),
        Command::Config(config) => Ok(cli::config(config, &paths()?)),
        Command::Logs(logs) => cli::logs(&logs, &paths()?),
        Command::Repo(repo) => tokio::runtime::Runtime::new()
            .context("starting the runtime")?
            .block_on(cli::repo(repo, &paths()?)),
        Command::Server(server) => tokio::runtime::Runtime::new()
            .context("starting the runtime")?
            .block_on(cli::server(server, &paths()?)),
    }
}
