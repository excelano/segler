# CLAUDE.md

Guidance for Claude Code working in `segler`. It is short because `DESIGN.md`
is where the reasoning lives; read that before touching anything.

---

## What this is

A review-and-correction tool for DocLang documents, and the lossless Rust
library underneath it. Three crates in one workspace:

- `crates/segler-core` — the library. Document model, parser, serializer,
  validation, `.dclx` archives, edit commands, undo. `#![forbid(unsafe_code)]`.
- `crates/segler` — the CLI, binary `segler`.
- `crates/segler-desktop` — the eframe application, presented as **Segler**.

**Three documents, three authorities.** `spec.md` in `doclang-project/doclang`
is the authority on the format; this repository neither restates nor amends it.
`DESIGN.md` here is the authority on this application. `git log` is the record
of why everything is the way it is, and it is written to be read.

Clones of the spec, the official viewer and docling.rs are at `~/clones/` on
David's machine. The spec's `examples/` and `tests/` are the conformance corpus.

---

## Commands

    cargo build --workspace
    cargo test --workspace
    cargo clippy --workspace --all-targets -- -D warnings   # must be silent
    cargo fmt --check
    cargo run -p segler -- inspect FILE
    cargo run -p segler-desktop -- [FILE]

**Seeing the window from here.** Launch it under XWayland and capture its own
window: `env -u WAYLAND_DISPLAY DISPLAY=:0 setsid target/debug/segler-desktop
FILE &`, find the client window with `xwininfo -root -tree | grep
'"segler-desktop"'`, then `xwd -id ID | convert xwd:- shot.png`. Drive it with
`xdotool`; synthetic typing needs `--delay 100` or more, since faster
keystrokes outrun the window and characters go missing. That proves a code
path draws; it does not stand in for David's keyboard walkthrough, which every
slice that touches the window gets.

**The conformance corpus is a command and never a test.** It needs a checkout
of `doclang-project/doclang`, which `cargo test` does not imply. When the runner
exists it is `cargo run -p segler -- corpus /path/to/doclang`; until then the
files under that checkout's `examples/` are what to open by hand.

---

## Rules with no exceptions

**The document model is lossless.** Every element, attribute, and text run in a
file survives a parse and a serialize. If the model cannot represent something
the spec allows, the model is wrong, not the file. This is the one property that
separates an editor from a converter and it does not bend for convenience.

**docling.rs is an import engine, never the model.** The `docling` crate reads
DocLang into a flat, lossy node list by design. It may appear behind a feature
flag for "Import PDF". It never touches the document model.

**Nothing compiles C.** A crate that links a system library is fine; one that
builds C is not. The check is the artefact, not the manifest: after a build,
`find target -name '*.o' -o -name '*.a'` under the build directories is empty.
`cargo tree -i cc` is *not* empty and never will be, because `wayland-backend`
declares `cc` as a build dependency and only uses it under its `client_system`
feature, which nothing here turns on. The `zip` dependency names `deflate`
alone for this reason. slipcase-desktop has the same rule and the Store
rejection that taught it.

**The UI is a renderer.** Selection, edits, undo, validation and save live in
the core behind a view-model and command boundary, so that a native front-end
could replace the egui one without touching the core. Logic that appears in
`segler-desktop` and would be needed by any other front-end is in the wrong
crate.

**Unsafe has one home per platform arm, if it ever needs one.** `segler-core`
is `forbid`; `segler-desktop` is `deny`, which can be lifted for exactly one
module when a platform demands it, the way slipcase-desktop's document-open
handler on macOS does. Adding one is a decision to take with David.

---

## Conventions

Every source file carries `Author: David M. Anderson` and `Built with AI
assistance (Claude, Anthropic)` in its header comment. Commits carry a
`Co-Authored-By` trailer for the Claude model in use and a `Signed-off-by`
trailer for David (DCO, as an LF AI & Data project would expect), and no
session URL.

CI is the fleet's `excelano/.github` Rust workflow; change policy there, not
here. Packaging, when it arrives, is cloned from `excelano/slipcase-desktop`,
one directory per platform.
