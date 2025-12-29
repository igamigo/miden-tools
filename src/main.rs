use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;

mod account;
mod cli;
mod inspect;
mod net;
mod parse;
mod rpc_tools;
mod store_account;
mod store_inspect;
mod tx_inspect;
mod word;

fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.execute()
}
