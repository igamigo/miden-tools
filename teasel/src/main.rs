use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;

mod cli;
mod render;
mod rpc;
mod store;
mod util;

fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.execute()
}
