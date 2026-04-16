use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct Cli {
    /// Path to the file to process
    pub file: PathBuf,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,
}
