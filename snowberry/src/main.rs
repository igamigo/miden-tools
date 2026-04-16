mod cli;
mod errors;
mod package_wrapper;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let options = cli::Options {
        verbose: cli.verbose,
    };
    let package = package_wrapper::PackageWrapper::from_file(&cli.file, &options)?;
    let fields = package.info()?;
    cli::display_fields(&fields);
    Ok(())
}
