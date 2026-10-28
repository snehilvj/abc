//! abcd: the abc server. Owns all state and serves clients over a Unix socket.
//! abc starts it when it isn't running; it runs until asked to stop.

mod log;
mod server;

use abc_core::build::BUILD_ID;
use abc_core::paths::{Paths, Profile};
use abc_core::protocol::PROTOCOL;
use anyhow::Context as _;
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "abcd",
    version,
    about = "The abc server: owns all state and serves abc over a Unix socket"
)]
struct Args {
    /// The profile to serve, else ABC_PROFILE, else stable.
    #[arg(long)]
    profile: Option<String>,
    /// Print this build's id and exit.
    #[arg(long)]
    print_build_id: bool,
    /// Say this build id instead of its own, to test what clients do about a stale server.
    #[arg(long, hide = true)]
    build_id: Option<String>,
    /// Say this protocol instead of its own, likewise.
    #[arg(long, hide = true)]
    protocol: Option<u32>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.print_build_id {
        println!("{BUILD_ID}");
        return Ok(());
    }
    let profile = Profile::choose(args.profile.as_deref(), |var| std::env::var_os(var))?;
    let paths = Paths::from_env(profile)?;
    let build = args.build_id.unwrap_or_else(|| BUILD_ID.into());
    tokio::runtime::Runtime::new()
        .context("starting the runtime")?
        .block_on(server::run(paths, build, args.protocol.unwrap_or(PROTOCOL)))
}
