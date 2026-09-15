# CLAUDE.md

An editor for DocLang documents and the lossless Rust library underneath it. The main pane
is the document rendered from the tree and edited in place; the page scans an archive may
carry are a reference pane, not the document. `crates/segler-core` is the library — model,
parser, serializer, validation, `.dclx` archives, edit commands, undo, `forbid(unsafe_code)`;
`crates/segler` is the CLI; `crates/segler-desktop` is the eframe window, **Segler**.
`spec.md` in `doclang-project/doclang` is the authority on the format and this repository
neither restates nor amends it. `DESIGN.md` is the authority on the application.

## Commands

    cargo build --workspace
    cargo test --workspace
    cargo clippy --workspace --all-targets -- -D warnings   # must be silent
    cargo fmt --check
    cargo run -p segler -- inspect FILE
    cargo run -p segler -- corpus /path/to/doclang   # needs the spec checkout; never a test
    cargo run -p segler-desktop -- [FILE]
    ./crates/segler-desktop/po/update-po.sh   # after changing any sentence a person reads
    ./crates/segler-desktop/po/pseudo.sh      # then a debug build with POTEXT_LANG=en-x-pseudo

Seeing the window from here: launch it under XWayland and capture its own window with
`env -u WAYLAND_DISPLAY DISPLAY=:0 setsid target/debug/segler-desktop FILE &`, find the
client window with `xwininfo -root -tree | grep '"segler-desktop"'`, then `xwd -id ID |
convert xwd:- shot.png`. Drive it with `xdotool`, whose synthetic typing needs `--delay 100`
or more. A one-frame defect is invisible to a single capture: take a burst and compare mean
brightness across frames. Run at `WINIT_X11_SCALE_FACTOR=1.25` as well as 1x, since David's
desktop is at a fractional scale. Releases: run `ship segler`. There is no release document.

## Rules

The document model is lossless: every element, attribute and text run in a file survives a
parse and a serialize, and a model that cannot represent something the spec allows is the
thing that is wrong. docling.rs is an import engine behind a feature flag and never touches
the model. Nothing compiles C — the check is the artefact, so `find target -name '*.o' -o
-name '*.a'` is empty; `cargo tree -i cc` is not and never will be, because
`wayland-backend` declares `cc` for a feature nothing here enables
(`~/notes/pure_rust_preference.md`). The UI is a renderer: selection, edits, undo,
validation and save live in the core behind the view-model and command boundary, and logic
in `segler-desktop` that another front end would need is in the wrong crate. The window is
translated and the model is not: an element's name and every attribute value a person picks
from a list stay English, because a translated one would be a different document.
`segler-core` is `forbid(unsafe_code)`; `segler-desktop` is `deny`, lifted for
`opened_document.rs` alone, where a macOS Apple Event needs an Objective-C method. A second
`allow` is David's decision. Every source file header carries `Author: David M. Anderson`
and `Built with AI assistance (Claude, Anthropic)`; commits carry a `Co-Authored-By` for the
model and a `Signed-off-by` for David (DCO), and no session URL.
