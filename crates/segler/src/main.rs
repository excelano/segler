//! The `segler` command-line tool.
//!
//!   segler inspect FILE     what a document or archive contains
//!   segler corpus DIR       parse and write back every document under DIR,
//!                           and report any that do not come back identical
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use segler_core::{archive, summary::Summary, tree::Document};

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
    /// Round-trip every document under a directory and report any that
    /// come back changed. Point it at a checkout of doclang-project/doclang.
    Corpus {
        /// A directory to walk for `.dclg`, `.xml` and `.dclx` files
        dir: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("segler: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.command {
        Command::Inspect { file } => inspect(&file).map(|()| ExitCode::SUCCESS),
        Command::Corpus { dir } => corpus(&dir),
    }
}

fn inspect(file: &Path) -> Result<(), String> {
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
    println!("located     {}", summary.located);
    println!("elements");
    for (name, count) in &summary.elements {
        println!("  {name:<20}{count}");
    }
    Ok(())
}

/// Walk `dir`, and for every document parse it and write it back. A file
/// that comes back byte-identical passes. One that does not is a defect in
/// the tree, and the command says where the first differing byte is.
fn corpus(dir: &Path) -> Result<ExitCode, String> {
    let mut files = Vec::new();
    collect(dir, &mut files).map_err(|e| format!("{}: {e}", dir.display()))?;
    files.sort();

    let (mut identical, mut mismatched, mut unparseable) = (0usize, 0usize, 0usize);
    for file in &files {
        let markup = match archive::load(file) {
            Ok(loaded) => loaded.markup,
            Err(e) => {
                unparseable += 1;
                println!("unreadable  {}: {e}", file.display());
                continue;
            }
        };
        let doc = match Document::parse(&markup) {
            Ok(doc) => doc,
            Err(e) => {
                unparseable += 1;
                println!("unparseable {}: {e}", file.display());
                continue;
            }
        };
        let back = doc.to_string();
        if back == markup {
            identical += 1;
        } else {
            mismatched += 1;
            let at = markup
                .bytes()
                .zip(back.bytes())
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| markup.len().min(back.len()));
            println!(
                "mismatch    {} (first difference at byte {at})",
                file.display()
            );
        }
    }
    println!(
        "{} files: {identical} identical, {mismatched} mismatched, {unparseable} unparseable",
        files.len()
    );
    Ok(if mismatched == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| matches!(e, "dclg" | "xml" | "dclx"))
        {
            out.push(path);
        }
    }
    Ok(())
}
