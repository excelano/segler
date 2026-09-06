# Segler

Segler is an editor for [DocLang](https://doclang.ai) documents, and the Rust
library underneath it.

DocLang is the AI-native markup format from the LF AI & Data Foundation: XML
that a language model can read and write token for token, carrying the
structure, semantics, layout and reading order of a document in one place.
Most DocLang is produced by models, from PDFs and scans, and a model is
sometimes wrong. Segler shows the document as a reader would see it and lets
a person edit it there: retype a misread line, change a heading's level, fix
a table cell, remove what should not be there. When an archive carries the page scans the
model read, they can be opened beside the document to check a doubtful line
against the page. What it saves is a valid DocLang document or archive.

The name is German for sailor.

## The pieces

`segler-core` is the library: the DocLang document model, its parser and
serializer, validation against the specification, and the `.dclx` archive
format. It loses nothing on a round trip, which is what distinguishes an
editor's model from a converter's. Nothing in it draws a window.

`segler` is the command-line tool over that library. `segler inspect FILE`
reports what a document or archive contains.

`segler-desktop` is the application. Presented as **Segler**; the binary is
named in full so it never collides with the command-line tool on `PATH`.

## Install

On Debian and Ubuntu, amd64, from the Excelano apt repository:

```
curl -fsSL https://excelano.com/apt/setup.sh | sudo sh && sudo apt install segler
```

That installs the application, the `segler` command-line tool, and the desktop
integration that opens a `.dclx` or `.dclg` on a double-click. Windows and
macOS builds are on their way to the Microsoft Store and the Mac App Store.

## Build

```
cargo build --release --workspace
```

A Rust toolchain is all it needs. Nothing in the dependency tree compiles C.
`packaging/linux/install.sh` puts a built copy and the desktop integration
under `~/.local`.

## The specification is the authority

The format is defined in
[doclang-project/doclang](https://github.com/doclang-project/doclang). Segler
implements it and does not restate it; where this repository and the
specification disagree, the specification is right and this is a bug.

## License

Apache-2.0, the same as DocLang, so that any part of this can go where the
format lives.

Built with the assistance of Claude (Anthropic).
