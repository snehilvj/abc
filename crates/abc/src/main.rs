//! abc: the terminal client and CLI.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "abc",
    version,
    about = "A terminal workspace for many repos and git worktrees"
)]
#[command(arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Open a screen with fake data. No server needed.
    Gallery(abc::gallery::Args),
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Gallery(args) => abc::gallery::run(args),
    }
}
