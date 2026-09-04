//! The `segler` command-line tool.
//!
//!   segler inspect FILE     what a document or archive contains
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use segler_core::{archive, summary::Summary};

#[derive(Parser)]
#[command(
    name = "segler",
    version,
    about = "Tools for DocLang documents and archives"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Report what a DocLang document or archive contains
    Inspect {
        /// A `.dclg` document or `.dclx` archive
        file: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("segler: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Inspect { file } => inspect(&file),
    }
}

fn inspect(file: &std::path::Path) -> Result<(), String> {
    let loaded = archive::load(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let summary = Summary::of(&loaded.markup).map_err(|e| format!("{}: {e}", file.display()))?;

    let kind = match loaded.kind {
        archive::Kind::Markup => "markup",
        archive::Kind::Archive => "archive",
    };
    println!("kind        {kind}");
    println!("version     {}", summary.version);
    println!(
        "namespace   {}",
        if summary.namespaced {
            "declared"
        } else {
            "absent"
        }
    );
    println!("pages       {}", summary.pages);
    if loaded.kind == archive::Kind::Archive {
        println!("page images {}", loaded.pages.len());
        println!("assets      {}", loaded.assets.len());
    }
    println!("located     {}", summary.located());
    println!("elements");
    for (name, count) in &summary.elements {
        println!("  {name:<20}{count}");
    }
    Ok(())
}
