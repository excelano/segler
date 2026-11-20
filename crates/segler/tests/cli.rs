// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use segler_core::{COMPATIBLE_VERSIONS, SPEC_VERSION};
use tempfile::TempDir;
use zip::write::SimpleFileOptions;

const DOC: &str = "<doclang version=\"0.10\"><picture><src uri=\"assets/a.png\"/></picture><page_break/><text>b</text><table><ched/>A<ched/>B<nl/><fcel/>1<fcel/>2<nl/></table></doclang>";

const TYPES: &str = "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/><Default Extension=\"pdf\" ContentType=\"application/pdf\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/document.xml\" ContentType=\"application/vnd.doclang.document+xml\"/></Types>";

const RELS: &str = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"r1\" Type=\"http://doclang.ai/ns/package/2026/relationships/document\" Target=\"document.xml\"/><Relationship Id=\"s\" Type=\"http://doclang.ai/ns/package/2026/relationships/source\" Target=\"source.pdf\"/><Relationship Id=\"m\" Type=\"http://doclang.ai/ns/package/2026/relationships/source-metadata\" Target=\"source-metadata.xml\"/></Relationships>";

const DESCRIPTOR: &str =
    "<source xmlns=\"http://doclang.ai/ns/package/2026/source-metadata\" name=\"report.pdf\"/>";

struct Run {
    ok: bool,
    stdout: String,
    stderr: String,
}

fn segler(args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_segler"))
        .args(args)
        .output()
        .unwrap();
    Run {
        ok: output.status.success(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

fn write_archive(path: &Path, parts: &[(&str, &str)]) {
    let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, data) in parts {
        w.start_file(*name, SimpleFileOptions::default()).unwrap();
        w.write_all(data.as_bytes()).unwrap();
    }
    w.finish().unwrap();
}

fn well_formed() -> Vec<(&'static str, &'static str)> {
    vec![
        ("[Content_Types].xml", TYPES),
        ("_rels/.rels", RELS),
        ("document.xml", DOC),
        ("pages/1.png", "png"),
        ("pages/2.png", "png"),
        ("assets/a.png", "png"),
        ("source.pdf", "%PDF"),
        ("source-metadata.xml", DESCRIPTOR),
    ]
}

fn archive(dir: &TempDir, name: &str, parts: &[(&str, &str)]) -> PathBuf {
    let path = dir.path().join(name);
    write_archive(&path, parts);
    path
}

fn markup(dir: &TempDir, name: &str, text: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, text).unwrap();
    path
}

fn arg(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn inspect_reports_an_archive_and_its_source() {
    let dir = TempDir::new().unwrap();
    let file = archive(&dir, "a.dclx", &well_formed());
    let run = segler(&["inspect", arg(&file)]);
    assert!(run.ok, "{}", run.stderr);
    for line in [
        "kind        archive",
        "version     0.10",
        "pages       2",
        "page images 2",
        "assets      1",
        "source      source.pdf (application/pdf, originally report.pdf)",
    ] {
        assert!(run.stdout.contains(line), "{line:?} not in\n{}", run.stdout);
    }
}

#[test]
fn inspect_reports_bare_markup() {
    let dir = TempDir::new().unwrap();
    let file = markup(&dir, "a.dclg", "<doclang><text>x</text></doclang>");
    let run = segler(&["inspect", arg(&file)]);
    assert!(run.ok, "{}", run.stderr);
    assert!(run.stdout.contains("kind        markup"));
    assert!(run.stdout.contains("version     not declared"));
    assert!(!run.stdout.contains("page images"));
}

#[test]
fn a_well_formed_archive_validates() {
    let dir = TempDir::new().unwrap();
    let file = archive(&dir, "a.dclx", &well_formed());
    let run = segler(&["validate", arg(&file)]);
    assert!(run.ok, "{}", run.stdout);
    assert_eq!(run.stdout, format!("{}: valid\n", file.display()));
}

#[test]
fn an_archive_defect_fails_validation() {
    let dir = TempDir::new().unwrap();
    let parts: Vec<_> = well_formed()
        .into_iter()
        .filter(|(name, _)| *name != "assets/a.png")
        .collect();
    let file = archive(&dir, "a.dclx", &parts);
    let run = segler(&["validate", arg(&file)]);
    assert!(!run.ok);
    assert!(run.stdout.contains("archive-references"), "{}", run.stdout);
    assert!(run.stdout.ends_with(": 1 findings\n"), "{}", run.stdout);
}

#[test]
fn a_warning_is_printed_but_does_not_fail() {
    let dir = TempDir::new().unwrap();
    let mut parts = well_formed();
    parts.push(("notes.xml", "<notes/>"));
    let file = archive(&dir, "a.dclx", &parts);
    let run = segler(&["validate", arg(&file)]);
    assert!(run.ok, "{}", run.stdout);
    assert!(
        run.stdout.contains("archive-unreferenced  notes.xml"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.ends_with(": valid\n"));
}

#[test]
fn a_markup_defect_fails_validation() {
    let dir = TempDir::new().unwrap();
    let file = markup(
        &dir,
        "a.dclg",
        "<doclang><heading level=\"0\">x</heading></doclang>",
    );
    let run = segler(&["validate", arg(&file)]);
    assert!(!run.ok);
    assert!(run.stdout.contains("/doclang/heading[1]"), "{}", run.stdout);
}

#[test]
fn an_unreadable_file_is_an_error() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("missing.dclx");
    let run = segler(&["validate", arg(&file)]);
    assert!(!run.ok);
    assert!(run.stdout.is_empty());
    assert!(run.stderr.starts_with("segler: "), "{}", run.stderr);
}

#[test]
fn render_and_page_show_one_page() {
    let dir = TempDir::new().unwrap();
    let file = archive(&dir, "a.dclx", &well_formed());
    let run = segler(&["page", arg(&file), "2"]);
    assert!(run.ok, "{}", run.stderr);
    assert!(
        run.stdout.starts_with("page 2 of 2, image pages/2.png\n"),
        "{}",
        run.stdout
    );
    let run = segler(&["render", arg(&file), "2"]);
    assert!(run.ok, "{}", run.stderr);
    assert!(run.stdout.contains('b'), "{}", run.stdout);
    let run = segler(&["render", arg(&file), "3"]);
    assert!(!run.ok);
    assert!(
        run.stderr.contains("no page 3; the document has 2"),
        "{}",
        run.stderr
    );
}

#[test]
fn table_prints_the_grid() {
    let dir = TempDir::new().unwrap();
    let file = archive(&dir, "a.dclx", &well_formed());
    let run = segler(&["table", arg(&file), "/doclang/table[1]"]);
    assert!(run.ok, "{}", run.stderr);
    assert!(
        run.stdout.starts_with("2 rows, 2 cols, 4 cells\n"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("(1,1) Full          span 1x1  \"2\""),
        "{}",
        run.stdout
    );
    let run = segler(&["table", arg(&file), "/doclang/table[2]"]);
    assert!(!run.ok);
    assert!(run.stderr.contains("no element at /doclang/table[2]"));
}

#[test]
fn edit_writes_markup_to_stdout() {
    let dir = TempDir::new().unwrap();
    let file = markup(&dir, "a.dclg", "<doclang><text>old</text></doclang>");
    let run = segler(&[
        "edit",
        arg(&file),
        "-e",
        "text /doclang/text[1] \"new words\"",
    ]);
    assert!(run.ok, "{}", run.stderr);
    assert_eq!(run.stdout, "<doclang><text>new words</text></doclang>");
}

#[test]
fn edit_writes_an_archive_that_still_validates() {
    let dir = TempDir::new().unwrap();
    let file = archive(&dir, "a.dclx", &well_formed());
    let out = dir.path().join("b.dclx");
    let run = segler(&[
        "edit",
        arg(&file),
        "-e",
        "text /doclang/text[1] c",
        "-o",
        arg(&out),
    ]);
    assert!(run.ok, "{}", run.stderr);
    assert!(
        run.stdout
            .starts_with("1 edits applied, 0 findings, written to "),
        "{}",
        run.stdout
    );
    let run = segler(&["validate", arg(&out)]);
    assert!(run.ok, "{}", run.stdout);
    assert!(segler(&["render", arg(&out), "2"]).stdout.contains('c'));
}

#[test]
fn edit_undo_returns_to_the_original() {
    let dir = TempDir::new().unwrap();
    let file = archive(&dir, "a.dclx", &well_formed());
    let run = segler(&[
        "edit",
        arg(&file),
        "-e",
        "text /doclang/text[1] c",
        "-e",
        "remove /doclang/picture[1]",
        "--undo",
    ]);
    assert!(run.ok, "{}", run.stdout);
    assert_eq!(
        run.stdout,
        "2 edits applied and undone; document matches the original\n"
    );
}

#[test]
fn a_bad_edit_is_an_error() {
    let dir = TempDir::new().unwrap();
    let file = markup(&dir, "a.dclg", "<doclang><text>old</text></doclang>");
    let run = segler(&["edit", arg(&file), "-e", "paint /doclang/text[1] red"]);
    assert!(!run.ok);
    assert!(
        run.stderr.contains("paint is not an edit"),
        "{}",
        run.stderr
    );
}

fn spec_checkout(earlier_changes: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    let schemas = dir.path().join("doclang").join("schemas");
    let schema = |version: &str, body: &str| {
        format!("<xs:schema version=\"{version}.0\">{body}</xs:schema>")
    };
    for (version, body) in [
        (SPEC_VERSION, ""),
        (COMPATIBLE_VERSIONS[0], earlier_changes),
    ] {
        let at = schemas.join(version);
        fs::create_dir_all(&at).unwrap();
        fs::write(at.join("doclang.xsd"), schema(version, body)).unwrap();
        fs::write(at.join("doclang.sch"), schema(version, body)).unwrap();
    }
    for sub in ["valid", "invalid"] {
        fs::create_dir_all(dir.path().join("examples").join(sub)).unwrap();
    }
    fs::write(
        dir.path().join("examples/valid/a.dclg"),
        "<doclang version=\"0.10\"><text>a</text></doclang>",
    )
    .unwrap();
    fs::write(
        dir.path().join("examples/invalid/b.dclg"),
        "<doclang><heading level=\"0\">x</heading></doclang>",
    )
    .unwrap();
    write_archive(&dir.path().join("examples/valid/c.dclx"), &well_formed());
    dir
}

#[test]
fn corpus_passes_a_checkout_that_matches_its_expectations() {
    let dir = spec_checkout("");
    let run = segler(&["corpus", arg(dir.path())]);
    assert!(run.ok, "{}", run.stdout);
    assert!(
        run.stdout.starts_with(
            "3 files: 3 identical after round trip; 2 valid, 1 invalid, 0 not DocLang"
        ),
        "{}",
        run.stdout
    );
}

#[test]
fn corpus_reports_a_compatible_version_whose_schema_changed() {
    let dir = spec_checkout("<xs:element/>");
    let run = segler(&["corpus", arg(dir.path())]);
    assert!(!run.ok);
    assert!(
        run.stdout.contains(&format!(
            "compatible  doclang.xsd of DocLang {} differs from {SPEC_VERSION}'s in more than its version",
            COMPATIBLE_VERSIONS[0]
        )),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("compatible  doclang.sch"));
}

#[test]
fn corpus_reports_a_file_in_the_wrong_directory() {
    let dir = spec_checkout("");
    fs::write(
        dir.path().join("examples/valid/d.dclg"),
        "<doclang><heading level=\"0\">x</heading></doclang>",
    )
    .unwrap();
    fs::write(
        dir.path().join("examples/invalid/e.dclg"),
        "<doclang version=\"0.10\"/>",
    )
    .unwrap();
    let run = segler(&["corpus", arg(dir.path())]);
    assert!(!run.ok);
    assert!(run.stdout.contains("unexpected  "), "{}", run.stdout);
    assert!(
        run.stdout.contains("d.dclg has 1 findings:"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("missed      "), "{}", run.stdout);
    assert!(run
        .stdout
        .contains("e.dclg should have findings and has none"));
}
