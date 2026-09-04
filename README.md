# Segler

Segler is a desktop application for reviewing and correcting
[DocLang](https://doclang.ai) documents, and the Rust library underneath it.

DocLang is the AI-native markup format from the LF AI & Data Foundation: XML
that a language model can read and write token for token, carrying the
structure, semantics, layout and reading order of a document in one place.
Most DocLang is produced by models, from PDFs and scans, and a model is
sometimes wrong. Segler shows the document as a reader would see it and lets
a person edit it there: retype a misread line, change a heading's level, fix
a table cell, move a paragraph. When an archive carries the page scans the
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

## Build

```
cargo build --release
```

A Rust toolchain is all it needs. Nothing in the dependency tree compiles C.

## The specification is the authority

The format is defined in
[doclang-project/doclang](https://github.com/doclang-project/doclang). Segler
implements it and does not restate it; where this repository and the
specification disagree, the specification is right and this is a bug.

## License

Apache-2.0, the same as DocLang, so that any part of this can go where the
format lives.

Built with the assistance of Claude (Anthropic).
