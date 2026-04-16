mod cli;
mod errors;
mod utils;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    println!("File: {}", cli.file.display());
}
