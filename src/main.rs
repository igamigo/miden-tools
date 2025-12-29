use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;

mod cli;
mod commands;
mod render;
mod store;
mod util;

fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.execute()
}
