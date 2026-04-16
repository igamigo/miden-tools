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

pub struct Options {
    pub verbose: bool,
}

pub fn display_fields(fields: &[(String, String)]) {
    if fields.is_empty() {
        return;
    }

    let name_width = fields.iter().map(|(n, _)| n.len()).max().unwrap();
    let value_width = fields.iter().map(|(_, v)| v.len()).max().unwrap();

    let top = format!("┌{:─<nw$}┬{:─<vw$}┐", "", "", nw = name_width + 2, vw = value_width + 2);
    let bottom = format!("└{:─<nw$}┴{:─<vw$}┘", "", "", nw = name_width + 2, vw = value_width + 2);
    let separator = format!("├{:─<nw$}┼{:─<vw$}┤", "", "", nw = name_width + 2, vw = value_width + 2);

    println!("{top}");
    for (i, (name, value)) in fields.iter().enumerate() {
        println!("│ {name:<name_width$} │ {value:<value_width$} │");
        if i < fields.len() - 1 {
            println!("{separator}");
        }
    }
    println!("{bottom}");
}
