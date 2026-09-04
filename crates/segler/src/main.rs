//! The `segler` command-line tool.
//!
//!   segler inspect FILE          what a document or archive contains
//!   segler validate FILE         every finding against the specification
//!   segler page FILE [N]         the boxes and tree of one page, as the
//!                                scan pane will draw them
//!   segler render FILE [N]       one page as blocks, the document pane's
//!                                input, printed as text
//!   segler table FILE PATH       a table's grid: every cell with its kind,
//!                                span and text
//!   segler edit FILE -e EDIT...  apply edits through the session and write
//!                                the result; --undo proves they undo
//!   segler corpus DIR [--toolkit]
//!                                parse and write back every document under
//!                                DIR, validate each against what its
//!                                directory says it should be, edit and undo
//!                                each to prove undo is exact, and with
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
    archive,
    blocks::{Block, Run},
    doclang::{self, Kind},
    otsl::{CellKind, Grid},
    session::{Command as Edit, Session},
    summary::Summary,
    tree::{Document, ElementId},
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
    /// Show one page as the window will draw it: its boxes and its tree
    Page {
        /// A `.dclg` document or `.dclx` archive
        file: PathBuf,
        /// The page number, from one
        #[arg(default_value_t = 1)]
        number: usize,
    },
    /// Print one page as the blocks the document pane draws
    Render {
        /// A `.dclg` document or `.dclx` archive
        file: PathBuf,
        /// The page number, from one
        #[arg(default_value_t = 1)]
        number: usize,
    },
    /// Print a table's grid: each cell with its kind, span and text
    Table {
        /// A `.dclg` document or `.dclx` archive
        file: PathBuf,
        /// The table's path, e.g. /doclang/table[1]
        path: String,
    },
    /// Apply edits through the session and write the result. Each edit is
    /// one of:
    ///   text PATH VALUE            attr PATH NAME VALUE|-
    ///   label PATH VALUE|-         layer PATH VALUE|-
    ///   bounds PATH X0 Y0 X1 Y1|-  rename PATH KIND
    ///   cell PATH ROW COL VALUE    cellkind PATH ROW COL KIND
    ///   move PATH PARENT INDEX     insert PARENT INDEX KIND
    ///   remove PATH
    /// where PATH is as `validate` prints it, e.g. /doclang/text[2].
    #[command(verbatim_doc_comment)]
    Edit {
        /// A `.dclg` document or `.dclx` archive
        file: PathBuf,
        /// An edit to apply, in order; repeatable
        #[arg(short = 'e', long = "edit", required = true)]
        edits: Vec<String>,
        /// Where to write; the markup goes to stdout without it
        #[arg(short = 'o', long)]
        out: Option<PathBuf>,
        /// After applying, undo every edit and confirm the document is
        /// byte-identical to what was opened
        #[arg(long)]
        undo: bool,
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
        Command::Page { file, number } => page(&file, number).map(|()| ExitCode::SUCCESS),
        Command::Render { file, number } => render(&file, number).map(|()| ExitCode::SUCCESS),
        Command::Table { file, path } => table(&file, &path).map(|()| ExitCode::SUCCESS),
        Command::Edit {
            file,
            edits,
            out,
            undo,
        } => edit(&file, &edits, out.as_deref(), undo),
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

fn open_session(file: &Path) -> Result<Session, String> {
    Session::open(file).map_err(|e| format!("{}: {e}", file.display()))
}

fn page(file: &Path, number: usize) -> Result<(), String> {
    let session = open_session(file)?;
    let view = session.page(number).ok_or_else(|| {
        format!(
            "{}: no page {number}; the document has {}",
            file.display(),
            session.page_count()
        )
    })?;
    println!(
        "page {} of {}{}",
        view.number,
        view.count,
        view.image
            .as_ref()
            .map(|i| format!(", image {i}"))
            .unwrap_or_default()
    );
    println!("boxes {}", view.boxes.len());
    for b in &view.boxes {
        let [x0, y0, x1, y1] = b.rect;
        println!(
            "  {:<14} {:.3} {:.3} {:.3} {:.3}",
            b.kind.name(),
            x0,
            y0,
            x1,
            y1
        );
    }
    println!("tree");
    for r in &view.rows {
        let mut tag = r.kind.name().to_owned();
        if let Some(d) = &r.detail {
            tag.push_str(&format!("[{d}]"));
        }
        let mut notes = Vec::new();
        if let Some(l) = &r.label {
            notes.push(format!("label={l}"));
        }
        if r.layer != "body" {
            notes.push(format!("layer={}", r.layer));
        }
        if r.located {
            notes.push("located".to_owned());
        }
        println!(
            "  {}{:<16} {:?}{}",
            "  ".repeat(r.depth),
            tag,
            r.excerpt,
            if notes.is_empty() {
                String::new()
            } else {
                format!("  ({})", notes.join(", "))
            }
        );
    }
    Ok(())
}

fn render(file: &Path, number: usize) -> Result<(), String> {
    let session = open_session(file)?;
    let blocks = session.blocks(number).ok_or_else(|| {
        format!(
            "{}: no page {number}; the document has {}",
            file.display(),
            session.page_count()
        )
    })?;
    print_blocks(&blocks, 0);
    Ok(())
}

fn runs_text(runs: &[Run]) -> String {
    let mut out = String::new();
    for r in runs {
        let mut t = r.text.clone();
        if r.style.bold {
            t = format!("**{t}**");
        }
        if r.style.italic {
            t = format!("_{t}_");
        }
        out.push_str(&t);
    }
    out
}

fn print_blocks(blocks: &[Block], depth: usize) {
    let pad = "  ".repeat(depth);
    for b in blocks {
        match b {
            Block::Heading { level, runs, .. } => {
                println!("{pad}{} {}", "#".repeat(*level as usize), runs_text(runs))
            }
            Block::Paragraph {
                kind, runs, blocks, ..
            } => {
                let tag = if *kind == Kind::Text {
                    String::new()
                } else {
                    format!("[{}] ", kind.name())
                };
                println!("{pad}{tag}{}", runs_text(runs));
                print_blocks(blocks, depth + 1);
            }
            Block::List { ordered, items, .. } => {
                for (i, item) in items.iter().enumerate() {
                    let marker = item.marker.clone().unwrap_or_else(|| {
                        if *ordered {
                            format!("{}.", i + 1)
                        } else {
                            "-".into()
                        }
                    });
                    println!("{pad}{marker} {}", runs_text(&item.runs));
                    print_blocks(&item.blocks, depth + 1);
                }
            }
            Block::Table {
                kind,
                caption,
                rows,
                cols,
                cells,
                ..
            } => {
                if let Some(c) = caption {
                    println!("{pad}[{}: {}]", kind.name(), runs_text(c));
                }
                for r in 0..*rows {
                    let mut line = String::new();
                    for c in 0..*cols {
                        let text = cells
                            .iter()
                            .find(|cell| cell.row == r && cell.col == c)
                            .map(|cell| {
                                let mut t = runs_text(&cell.runs);
                                if t.is_empty() && !cell.blocks.is_empty() {
                                    t = "(…)".into();
                                }
                                if cell.kind.is_header() {
                                    t = format!("*{t}*");
                                }
                                t
                            })
                            .unwrap_or_else(|| "^".into());
                        line.push_str(&format!("| {text} "));
                    }
                    println!("{pad}{line}|");
                }
            }
            Block::Picture {
                src,
                caption,
                blocks,
                ..
            } => {
                println!(
                    "{pad}[picture {}]{}",
                    src.as_deref().unwrap_or("no src"),
                    caption
                        .as_ref()
                        .map(|c| format!(" {}", runs_text(c)))
                        .unwrap_or_default()
                );
                print_blocks(blocks, depth + 1);
            }
            Block::Code { language, text, .. } => {
                println!("{pad}```{}", language.as_deref().unwrap_or(""));
                for line in text.lines() {
                    println!("{pad}{line}");
                }
                println!("{pad}```");
            }
            Block::Formula { text, .. } => println!("{pad}$ {text} $"),
            Block::Container { kind, blocks, .. } => {
                println!("{pad}[{}]", kind.name());
                print_blocks(blocks, depth + 1);
            }
            Block::PageBreak => println!("{pad}----"),
            Block::Other { name, runs, .. } => println!("{pad}<{name}> {}", runs_text(runs)),
        }
    }
}

fn table(file: &Path, path: &str) -> Result<(), String> {
    let session = open_session(file)?;
    let id = session
        .find_by_path(path)
        .ok_or_else(|| format!("no element at {path}"))?;
    let el = session
        .document()
        .find(id)
        .ok_or_else(|| format!("no element at {path}"))?;
    let grid = Grid::parse(el);
    println!(
        "{} rows, {} cols, {} cells",
        grid.rows,
        grid.cols,
        grid.cells.len()
    );
    for cell in &grid.cells {
        let start = segler_core::otsl::body_start(el, cell);
        let text: String = el.children()[start..cell.content.end]
            .iter()
            .map(|n| match n {
                segler_core::tree::Node::Text(t) => t.value().to_owned(),
                segler_core::tree::Node::CData(s) => s.clone(),
                segler_core::tree::Node::Element(e) => e.text(),
                _ => String::new(),
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "  ({},{}) {:<13} span {}x{}  {:?}",
            cell.row,
            cell.col,
            format!("{:?}", cell.kind),
            cell.rowspan,
            cell.colspan,
            text
        );
    }
    Ok(())
}

fn edit(file: &Path, edits: &[String], out: Option<&Path>, undo: bool) -> Result<ExitCode, String> {
    let mut session = open_session(file)?;
    let original = session.markup();
    let mut applied = 0;
    for line in edits {
        let command = parse_edit(&session, line)?;
        session
            .apply(command)
            .map_err(|e| format!("edit {line:?}: {e}"))?;
        applied += 1;
    }
    let findings = session.findings().len();
    if undo {
        for _ in 0..applied {
            session.undo();
        }
        let same = session.markup() == original;
        println!(
            "{applied} edits applied and undone; document {} the original",
            if same { "matches" } else { "DIFFERS FROM" }
        );
        return Ok(if same {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        });
    }
    match out {
        Some(path) => {
            session
                .save_to(path)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            println!(
                "{applied} edits applied, {findings} findings, written to {}",
                path.display()
            );
        }
        None => print!("{}", session.markup()),
    }
    Ok(ExitCode::SUCCESS)
}

/// One edit line into a command. Values may be double-quoted.
fn parse_edit(session: &Session, line: &str) -> Result<Edit, String> {
    let words = split_words(line);
    let word = |i: usize| {
        words
            .get(i)
            .map(String::as_str)
            .ok_or_else(|| format!("edit {line:?} is incomplete"))
    };
    let at = |path: &str| -> Result<ElementId, String> {
        session
            .find_by_path(path)
            .ok_or_else(|| format!("no element at {path}"))
    };
    let optional = |v: &str| (v != "-").then(|| v.to_owned());
    let kind = |name: &str| {
        Kind::from_name(name).ok_or_else(|| format!("{name} is not a DocLang element"))
    };
    let index = |v: &str| {
        v.parse::<usize>()
            .map_err(|_| format!("{v} is not an index"))
    };
    Ok(match word(0)? {
        "text" => Edit::SetText {
            id: at(word(1)?)?,
            text: word(2)?.to_owned(),
        },
        "attr" => Edit::SetAttr {
            id: at(word(1)?)?,
            name: word(2)?.to_owned(),
            value: optional(word(3)?),
        },
        "label" => Edit::SetLabel {
            id: at(word(1)?)?,
            value: optional(word(2)?),
        },
        "layer" => Edit::SetLayer {
            id: at(word(1)?)?,
            value: optional(word(2)?),
        },
        "bounds" => {
            let id = at(word(1)?)?;
            let bounds = if word(2)? == "-" {
                None
            } else {
                let mut b = [0u32; 4];
                for (i, slot) in b.iter_mut().enumerate() {
                    *slot = word(2 + i)?.parse().map_err(|_| {
                        format!("{} is not a grid value", word(2 + i).unwrap_or(""))
                    })?;
                }
                Some(b)
            };
            Edit::SetBounds { id, bounds }
        }
        "rename" => Edit::Rename {
            id: at(word(1)?)?,
            kind: kind(word(2)?)?,
        },
        "move" => Edit::Move {
            id: at(word(1)?)?,
            parent: at(word(2)?)?,
            index: index(word(3)?)?,
        },
        "insert" => Edit::Insert {
            parent: at(word(1)?)?,
            index: index(word(2)?)?,
            kind: kind(word(3)?)?,
        },
        "remove" => Edit::Remove { id: at(word(1)?)? },
        "cell" => Edit::SetCellText {
            id: at(word(1)?)?,
            row: index(word(2)?)?,
            col: index(word(3)?)?,
            text: word(4)?.to_owned(),
        },
        "cellkind" => Edit::SetCellKind {
            id: at(word(1)?)?,
            row: index(word(2)?)?,
            col: index(word(3)?)?,
            kind: CellKind::ALL
                .iter()
                .copied()
                .find(|k| k.token().name() == word(4).unwrap_or(""))
                .ok_or_else(|| format!("{} is not a cell token", word(4).unwrap_or("")))?,
        },
        other => return Err(format!("{other} is not an edit")),
    })
}

fn split_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut pending = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                pending = true;
            }
            c if c.is_whitespace() && !quoted => {
                if pending {
                    words.push(std::mem::take(&mut current));
                    pending = false;
                }
            }
            c => {
                current.push(c);
                pending = true;
            }
        }
    }
    if pending {
        words.push(current);
    }
    words
}

/// Edit every element a session offers, then undo it all: the document
/// must come back byte-identical. Returns what differed, if anything.
fn edit_undo_identity(file: &Path) -> Result<Option<String>, String> {
    let mut session = open_session(file)?;
    let original = session.markup();
    let ids: Vec<ElementId> = session.document().elements().map(|e| e.id()).collect();
    let mut applied = 0;
    for id in &ids {
        let attempts = [
            Edit::SetText {
                id: *id,
                text: "edited".into(),
            },
            Edit::SetLabel {
                id: *id,
                value: Some("edited".into()),
            },
            Edit::SetLayer {
                id: *id,
                value: Some("furniture".into()),
            },
            Edit::SetBounds {
                id: *id,
                bounds: Some([1, 2, 3, 4]),
            },
            Edit::SetAttr {
                id: *id,
                name: "class".into(),
                value: Some("edited".into()),
            },
        ];
        for command in attempts {
            if session.apply(command).is_ok() {
                applied += 1;
            }
        }
    }
    let root = session.document().root.id();
    if let Some(last) = session
        .document()
        .root
        .child_elements()
        .last()
        .map(|e| e.id())
    {
        if session
            .apply(Edit::Move {
                id: last,
                parent: root,
                index: 0,
            })
            .is_ok()
        {
            applied += 1;
        }
        if session.apply(Edit::Remove { id: last }).is_ok() {
            applied += 1;
        }
    }
    for _ in 0..applied {
        session.undo();
    }
    let back = session.markup();
    if back == original {
        return Ok(None);
    }
    let at = original
        .bytes()
        .zip(back.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(original.len().min(back.len()));
    Ok(Some(format!(
        "after {applied} edits and undos, first difference at byte {at}"
    )))
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

        match edit_undo_identity(file) {
            Ok(None) => {}
            Ok(Some(why)) => {
                problems += 1;
                println!("undo        {}: {why}", file.display());
            }
            Err(e) => {
                problems += 1;
                println!("undo        {e}");
            }
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
        "{} files: {identical} identical after round trip; {valid} valid, {invalid} invalid, {skipped} not DocLang; every DocLang file edited and undone to identity{}",
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
