//! The `segler` command-line tool.
//!
//!   segler inspect FILE          what a document or archive contains
//!   segler validate FILE         every finding against the specification
//!   segler corpus DIR [--toolkit]
//!                                parse and write back every document under
//!                                DIR, validate each against what its
//!                                directory says it should be, and with
//!                                --toolkit compare verdicts with the
//!                                reference toolkit's `doclang validate`
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, ExitCode};

use clap::{Parser, Subcommand};
use segler_core::{
    archive, doclang,
    summary::Summary,
    tree::Document,
    validate::{self, Layer},
};

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
    /// Check a document or archive against the specification
    Validate {
        /// A `.dclg` document or `.dclx` archive
        file: PathBuf,
    },
    /// Round-trip and validate every document under a directory. Point it
    /// at a checkout of doclang-project/doclang: files under a `valid`
    /// directory must produce no findings and files under `invalid` must
    /// produce at least one.
    Corpus {
        /// A directory to walk for `.dclg`, `.xml` and `.dclx` files
        dir: PathBuf,
        /// Also run the reference toolkit's `doclang validate` on each file
        /// and report any file where the two disagree on either layer
        #[arg(long)]
        toolkit: bool,
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
        Command::Validate { file } => validate_file(&file),
        Command::Corpus { dir, toolkit } => corpus(&dir, toolkit),
    }
}

fn load(file: &Path) -> Result<String, String> {
    archive::load(file)
        .map(|loaded| loaded.markup)
        .map_err(|e| format!("{}: {e}", file.display()))
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

fn validate_file(file: &Path) -> Result<ExitCode, String> {
    let markup = load(file)?;
    let doc = Document::parse(&markup).map_err(|e| format!("{}: {e}", file.display()))?;
    let findings = validate::validate(&doc);
    if findings.is_empty() {
        println!("{}: valid", file.display());
        return Ok(ExitCode::SUCCESS);
    }
    for f in &findings {
        println!("{}  {}  {}\n    {}", f.layer, f.rule, f.path, f.message);
    }
    println!("{}: {} findings", file.display(), findings.len());
    Ok(ExitCode::FAILURE)
}

/// What a corpus file is expected to be, from the directory it sits in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Expect {
    Valid,
    Invalid,
    Unknown,
}

fn expectation(file: &Path) -> Expect {
    let mut expect = Expect::Unknown;
    for part in file.components() {
        match part.as_os_str().to_str() {
            Some("valid") => expect = Expect::Valid,
            Some("invalid") => expect = Expect::Invalid,
            _ => {}
        }
    }
    expect
}

/// Walk `dir`, and for every document parse it, write it back, and
/// validate it. A file that comes back byte-identical and matches its
/// directory's expectation passes; anything else is printed, with the first
/// differing byte for a round-trip mismatch.
fn corpus(dir: &Path, toolkit: bool) -> Result<ExitCode, String> {
    let mut files = Vec::new();
    collect(dir, &mut files).map_err(|e| format!("{}: {e}", dir.display()))?;
    files.sort();

    let mut problems = 0usize;
    let (mut identical, mut valid, mut invalid, mut skipped, mut agreed) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    for file in &files {
        let markup = match load(file) {
            Ok(markup) => markup,
            Err(e) => {
                problems += 1;
                println!("unreadable  {e}");
                continue;
            }
        };
        let doc = match Document::parse(&markup) {
            Ok(doc) => doc,
            Err(e) => {
                problems += 1;
                println!("unparseable {}: {e}", file.display());
                continue;
            }
        };

        let back = doc.to_string();
        if back == markup {
            identical += 1;
        } else {
            problems += 1;
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

        if doclang::kind(&doc.root) != Some(doclang::Kind::Doclang) {
            skipped += 1;
            continue;
        }
        let findings = validate::validate(&doc);
        let schema = findings.iter().any(|f| f.layer == Layer::Schema);
        let rules = findings.iter().any(|f| f.layer == Layer::Rules);
        match (expectation(file), findings.is_empty()) {
            (Expect::Valid, false) => {
                problems += 1;
                println!(
                    "unexpected  {} has {} findings:",
                    file.display(),
                    findings.len()
                );
                for f in &findings {
                    println!(
                        "              {} {} {}: {}",
                        f.layer, f.rule, f.path, f.message
                    );
                }
            }
            (Expect::Invalid, true) => {
                problems += 1;
                println!(
                    "missed      {} should have findings and has none",
                    file.display()
                );
            }
            _ => {}
        }
        if findings.is_empty() {
            valid += 1;
        } else {
            invalid += 1;
        }

        if toolkit {
            match toolkit_verdict(file) {
                Ok((their_schema, their_rules)) => {
                    if (their_schema, their_rules) == (schema, rules) {
                        agreed += 1;
                    } else {
                        problems += 1;
                        println!(
                            "disagree    {}: schema ours={} toolkit={}, rules ours={} toolkit={}",
                            file.display(),
                            verdict(schema),
                            verdict(their_schema),
                            verdict(rules),
                            verdict(their_rules)
                        );
                        for f in &findings {
                            println!(
                                "              {} {} {}: {}",
                                f.layer, f.rule, f.path, f.message
                            );
                        }
                    }
                }
                Err(e) => {
                    problems += 1;
                    println!("toolkit     {}: {e}", file.display());
                }
            }
        }
    }
    println!(
        "{} files: {identical} identical after round trip; {valid} valid, {invalid} invalid, {skipped} not DocLang{}",
        files.len(),
        if toolkit { format!("; toolkit agrees on {agreed}") } else { String::new() }
    );
    Ok(if problems == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn verdict(has_findings: bool) -> &'static str {
    if has_findings {
        "invalid"
    } else {
        "valid"
    }
}

/// Run the reference toolkit on a file and return whether its XSD and
/// Schematron layers found problems. Uses `-n` so that a document without
/// the namespace is judged on its content, as this validator judges it.
fn toolkit_verdict(file: &Path) -> Result<(bool, bool), String> {
    let output = Process::new("doclang")
        .args(["validate", "--format", "json", "-n"])
        .arg(file)
        .output()
        .map_err(|e| format!("cannot run doclang: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if output.status.success() && !stdout.trim_start().starts_with('{') {
        return Ok((false, false));
    }
    let json: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("toolkit output is not JSON: {e}"))?;
    let layer = |name: &str| {
        json.get(name)
            .and_then(|l| l.get("valid"))
            .and_then(|v| v.as_bool())
            .map(|valid| !valid)
            .ok_or_else(|| format!("toolkit output has no {name}.valid"))
    };
    Ok((layer("xsd")?, layer("schematron")?))
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
