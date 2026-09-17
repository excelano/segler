# Segler — Design Document

**Implements:** DocLang 0.7, from `spec.md` in `doclang-project/doclang`.
**Section references:** a bare `§4` is this document. `spec §Tables` is the specification. `slipcase-desktop §2` is that repository's design document, which this one borrows from and names when it does.

---

## 1. What this repository is

A desktop application that opens a DocLang document or archive, renders it as a reader would see it, and lets a person edit it there and save a valid document back. Underneath it, the first Rust library that represents DocLang without losing anything.

The library is a deliverable in its own right. DocLang's reference toolkit is Python, the official viewer is JavaScript and view-only, and docling.rs, the Rust port of IBM's converter, reads DocLang into a model built for conversion rather than editing. A lossless Rust model is the missing piece, and it is licensed and organised so that the DocLang project could take it in whole.

The specification is the authority on the format, and this document neither restates nor amends it. Where a rule is cited below it is the spec's rule and the spec's wording wins.

---

## 2. What DocLang commits us to

Seven facts in the specification shape everything below, and they are named here so that nothing later has to be inferred from them.

Every semantic element has a head and a body. The head is a fixed-order sequence of property elements: label, thread, one of xref or href, layer, four locations, caption, description, summary, custom. The body is the payload. Property semantics live in leading child elements rather than attributes, on purpose, because the format is designed around a tokenizer. An editor's model has to keep that order on the way out.

Geometry is a 512-point grid. Four `location` values are `x_min, y_min, x_max, y_max` on a grid whose real size the document head's `default_resolution` declares, relative to the top left of the page. Every box the window draws goes through that normalisation, and every box a person drags comes back through it.

Tables are OTSL. A table body is a flat run of cell markers, `fcel`, `ched`, `rhed`, `lcel`, `ucel`, `xcel` and the rest, with `nl` ending a row and the rectangular rule requiring every row to carry the same number of markers. The grid is not in the XML; it is computed from the sequence, and a cell edit has to be expressed back as a change to the sequence.

Pages are `page_break` elements. There is no page element. A document with two breaks has three pages; the spec's page-alignment rule ties segment N to page image N, and images may have gaps but may not exceed the count.

An archive is an OPC package. `.dclx` is a ZIP with `[Content_Types].xml`, `_rels/.rels`, `document.xml`, optional `pages/N.png` counted from one, and optional `assets/`. OPC only stores or deflates, which is why `§5` takes `zip` with deflate alone.

Version 0.x breaks on every minor. A 0.7 document is incompatible with a 0.8 schema by the spec's own rule. Segler targets one version at a time and says which; it does not pretend to read a version it was not built against.

Validation is an XSD and a Schematron. The XSD is structural. The Schematron is thirteen patterns, eighteen assertions, in XPath 3.1 with `queryBinding="xslt3"`, which no Rust engine runs. `§6` says what that costs.

---

## 3. The product

Segler is an editor for DocLang documents. A DocLang document is the `<doclang>` tree, whichever file carries it: a `.dclg` is the tree alone, a `.dclx` is the same tree in a package with two optional extras, page scans under `pages/` that the spec itself calls raster images for review, and files under `assets/` that the markup refers to. Everything a person would call the content is in the tree, and so is everything the model said about it: labels, layers, bounding boxes, threads and cross-references. Editing a DocLang document means editing that tree, and what Segler saves is that tree, bare or repacked with the extras untouched.

An editor shows the document. The main pane renders the tree the way a reader would see it, headings as headings, paragraphs as paragraphs, lists, tables as grids, pictures from their `src`, captions under them, and edits happen there: click into a paragraph and retype it, change a heading's level and watch it change, fix a table cell in the grid. The view is drawn from the tree, so an edit shows because the thing it changed is the thing on screen, and it works identically for both file types because it needs nothing but the tree.

The page scans keep a job, a secondary one. When a paragraph reads wrong and a person wants to see what the scanner saw, or a bounding box is off, the scan belongs beside the rendered document with the boxes drawn on it. That is a reference pane, shown on request for an archive that carries scans, and absent by design for a bare file. It is not the centre of the window: a pane at the centre would be empty for a whole class of documents and would never change when the document does.

Two readings of "editor" remain out. An XML editor with DocLang awareness is a language server plus a shell, which a VS Code extension would do better; it survives as the markup pane. And there is no reason to keep authoring out any longer than it keeps itself out: a rendered, editable document can be started from nothing as easily as opened, and the only thing that costs is a New command. The user is the DocLang community: whoever has a document and wants to change what is in it.

A session looks like this. Open a document. It renders. Click a heading and change its level from the element pane, or click into its text and retype it; the rendering follows. Select a paragraph in the rendered view and the same element highlights in the tree on the left and fills the element pane on the right. Fix a misread word. Move a paragraph earlier. Set a table's header row. If the archive has scans, open the reference pane to check a doubtful line against the page. Validation runs on every edit and the problems list stays visible. Save.

## 4. Architecture

Three crates in one workspace, and one boundary between them that is the design.

`segler-core` owns everything that is not pixels. The document model, the parser, the serializer, validation, the OTSL grid, the archive format, selection, every edit command, undo and redo, and save. It has no dependency on any user-interface crate and never will.

`segler` is the command-line tool over the core. Inspect, validate, pack, unpack, format, diff, and the conformance corpus runner. It proves the core on real files, and it is a product in its own right.

`segler-desktop` is the window. It asks the core for a view-model of the current page and the current selection, draws it, and sends commands back. It holds no document state of its own beyond what is being drawn.

**The boundary is a view-model and a command set, and it is the reason a native front-end stays possible.** The core exposes a session: open, the view-model for page N, apply a command, undo, validate, save. The view-model is plain data: boxes with ids and normalised coordinates, tree rows with ids and labels, the markup as text with id-to-span mapping. Commands are plain data too: set label, set heading level, set text, move box, reorder, set cell kind. egui is one consumer. A Swift or C# front-end over UniFFI would be another, consuming the same data and sending the same commands, and would need nothing from the egui crate. Logic that appears in `segler-desktop` and that any other front-end would also need is in the wrong crate, and the test for that is whether it touches egui types.

**The model is lossless, and that is the property that separates it from docling.rs.** Every element, attribute, text run, CDATA section and comment in a file survives a parse and a serialize, in order. The spec allows things the editor has no button for, custom vocabularies and `custom` elements among them, and they pass through untouched. If the model cannot represent something the spec allows, the model is wrong. docling.rs reads DocLang into a flat node list with inline formatting baked into Markdown-style strings and its reader ignores what it does not know, which is right for a converter and disqualifying here. It is an import engine behind a feature flag, for opening a PDF that is not DocLang yet, and it never touches the model.

**The serializer's target is the file itself.** A document parsed and written back without edits is the same bytes. The tree keeps the source of every start tag and every text run and writes it back verbatim until an edit touches that node; only an edited node is regenerated, in one fixed form. So an unedited save diffs clean against the original file rather than against somebody's pretty-printer, and an edited save diffs at the edit and nowhere else. `segler corpus` measures identity on every file under the specification checkout.

**Undo is a log of what each command displaced**, not a log of inverse commands: the children of the element a command touched, an attribute's old value, an old name. Undo puts those nodes back as they were, raw source and all, and redo re-runs the command. An inverse command would regenerate the nodes it puts back, and a regenerated node has lost its raw source, so the lossless property would hold only until the first undo. `segler corpus` edits every element of every file through every command that applies to it and undoes it all; every file comes back byte-identical. The session also refuses what it cannot represent, text carrying a character XML cannot hold among them, before it reaches a node.

**The session is the boundary.** `segler-core::session` holds the document, the original archive bytes, the selection, and both stacks, and offers three view-models: the document (version, resolution, page count, whether it is dirty), a page (its image part, its boxes as fractions of the page, its semantic elements as tree rows with an excerpt each), and one element in full (head, attributes, text, markup, position). Commands are plain data addressed by element id: set text, set attribute, set label, set layer, set bounds, rename, move, insert, remove. The CLI's `page` and `edit` commands drive all of it, which is how the boundary is exercised against real files without a window.

---

## 5. Dependencies

The rule is slipcase-desktop's, borrowed whole: nothing compiles C. A crate that links a system library is fine; one that builds C is not. Across the fleet this is a preference rather than a rule, held in `~/notes/pure_rust_preference.md` together with what taking C costs; this repository keeps it as a rule for itself, as slipcase-desktop does. The measurement is the artefact rather than the manifest, because the manifest lies in one direction: `cargo tree -i cc` names `wayland-backend`, which declares `cc` as a build dependency and uses it only under its `client_system` feature, and nothing here enables that. No `.o` or `.a` anywhere under the build directories, and `cargo tree -i cmake` and `-i bindgen` coming back empty, is what the rule means. A build needs a Rust toolchain and nothing else, on all three platforms.

The framework is egui, through eframe. It was chosen over Tauri for pipeline reasons rather than performance: slipcase-desktop is an egui application already through Microsoft Store certification with a three-platform packaging tree, and none of that would transfer to a web-view application. It also keeps both of David's desktop applications on one open-source framework, so a problem found in one is fixable for both. What egui does natively covers most of `§7`: images are textures, boxes are painter calls, the tree is collapsing headers, the table grid is `egui_extras`. Its weak surface is free-form text editing, which `§7` confines. Tauri is the fallback if that surface proves inadequate, and the boundary in `§4` is what makes the fallback cheap.

Reading XML is `xmlparser`, the tokenizer under `roxmltree`, taken directly. A DOM has already decoded entities, merged text runs and dropped the spelling of each start tag, which is the information a lossless tree needs; the tokenizer hands back raw spans and the tree keeps them. Writing XML is this crate's own, for the same reason: the writer's job is to reproduce those spans and regenerate only what changed, and no library does that. Document type declarations are refused on parse, as the reference toolkit refuses them, since DocLang has nothing for one to declare and it is how entity expansion arrives.

Archives are `zip` with default features off and `deflate` named, because the defaults pull `zstd` and `xz` and both compile C. OPC never uses either.

File dialogs are `rfd`, whose defaults are the XDG portal and Wayland, both pure Rust; its `gtk3` feature links C and stays off.

Import is `docling`, docling.rs, behind a feature flag that is off by default. It brings ONNX Runtime and model downloads, and a release that does not offer import should not carry them.

UniFFI is named here and not taken. It is the route to a native front-end when one is wanted, and taking it before then would be building a port nobody has asked for.

---

## 6. Validation

Two layers, and neither is the reference toolkit called out of process. Segler runs on a stock Windows or Mac with no Python, and a validator that shells out to one is not a validator there.

The XSD is ported into the model. A typed model that can only represent valid structure catches most of what the schema catches at parse time, and what remains, cardinalities and attribute value sets, is a pass over the model. The port is per spec version, and a spec bump is a regeneration, which is why the model's shape follows the schema's rather than any convenience of the editor's.

The Schematron is ported by hand. Thirteen patterns holding eighteen assertions is a size a person can carry, and each becomes a Rust check with the pattern's own identifier and message, so that a finding here reads the same as a finding from the reference toolkit. The port is what tracks spec drift: every rule change upstream is a diff in this file and a diff in that check.

**Where the port and the reference toolkit disagree, the corpus decides.** The toolkit transpiles the Schematron to XSLT itself and prefixes every rule context with `//`, which in XPath binds only to the first alternative of a union, so most of `element-head-placement`'s contexts are evaluated against the document node and match nothing. The port scopes that rule to a list's or table's own head and implements every other pattern as written, which means it reports things the toolkit misses — a head element after text in a `heading` among them. Those are defects in the document, not disagreements to paper over, and the transpiler is a finding to take upstream.

**The conformance corpus is a command, never a test.** `segler corpus /path/to/doclang` walks the specification checkout's `examples/` and `tests/`, parses and re-serializes each file, and compares the serialized bytes against the file itself and the verdict and findings against the Python toolkit's. All of them must agree. It needs the checkout and the toolkit installed, which `cargo test` does not imply, and a test that has to choose between skipping quietly and failing on a machine that was never going to have those is worse than a command run on purpose. Run it before and after any change to the model, the parser, the serializer or the validator.

---

## 7. Shape

One window. The rendered document in the middle, the structure tree on the left, the element pane on the right, the problems list below, and one selection across all of them.

**The document pane** renders the tree as a reader would see it and is where editing happens. Headings at their level, paragraphs with their inline formatting, lists with their markers, tables as grids, pictures from their `src` with captions under them, formulas and code set apart, page breaks as rules. Clicking an element selects it. Clicking into text edits it in place, in a field styled like the element it belongs to, committing when the field is left. Structural edits — a heading's level, a list's kind, a table cell's role, a picture's class — are on the element pane and take effect in the rendering at once. Bounding boxes are not drawn here; that is the reference pane's business.

A block's frame registers its click sense before its children draw, over last frame's rect, so a click on a table cell selects the cell and not the table: egui hands a click to the topmost widget. Table rows are laid out by hand at fixed widths with borders painted after, since the grid does not hold its columns, and a header fill goes into a paint-list slot reserved before the row draws, or it covers the text it sits behind.

**What a text edit may touch.** A list item is a range, exactly as an OTSL cell is: `ldiv` is an empty separator and the item's content is the siblings after it, so `SetListItemText` addresses it by position the way `SetCellText` addresses a cell by row and column, and `blocks::item_bodies` is the one place a range is computed. A formatted paragraph shows its tags: a formatting element takes no attributes and carries no head, so `<bold>…</bold>` is the whole of one and a field holding that spelling loses nothing. `inline.rs` writes it, reads it back through the same parser the rest of the application uses, and refuses malformed markup by name rather than writing something else. Formatting can be added that way. A plain paragraph is unchanged and needs no escaping, because tags are shown only where there are tags. The bodies with no inline spelling — a CDATA section, a comment, a nested `text` in a table cell — keep the weaker answer: the edit is applied, each element is put back wherever its own text can still be found in what was typed, and where one cannot the window asks before writing plain text.

**The structure pane** is the tree of the document's semantic elements in document order, with an excerpt each: select a row and the document pane and element pane follow, and a selection made elsewhere scrolls the tree to it. A picture's inner text rows fold under the picture by default, with a disclosure triangle, and a picture opens itself when the selection lands inside it — the image already shows those words, and forty one-word rows push the page's paragraphs out of sight.

**The element pane** shows the selected element in full, with its controls and its markup. It is where the properties that have no visual form in the rendering are edited — label, layer and bounds — and where a structural change is made deliberately rather than by typing. `thread`, the cross-reference, renaming, `Move` and `Insert` are commands in the core and in the CLI and are not in the window yet; none is decided against.

**The reference pane** shows the page scan for the current page with a rectangle over every located element, linked to the selection both ways, zoomable. It opens on request, beside the document pane, only for an archive that carries scans, and a bare document has no such pane. It claims its own width, or the scroll area and the fitted image shrink each other a little every frame. The page opens fitted to the pane rather than at the image's size, since a page image is wider than the pane on any screen, and a slider, a Fit button and a 100% button sit in the toolbar while the pane is open and nowhere at all while it is shut, because they drive the pane and nothing else.

**The problems list** sits below and shows validation findings, each linking to its element. It is never hidden while a document has findings. Clicking a finding scrolls the tree without flashing its last row, which is only visible at a fractional display scale.

**The structure and element panes fold away**, from toolbar toggles or Ctrl+1 and Ctrl+2, so the document can have the width when the tree and the controls are not wanted.

Text edits commit when the field loses focus, label edits likewise, level and class and layer on change, and removal asks first even though undo restores it, because a keystroke on the wrong row is easy. The Linux theme defect that slipcase-desktop measured reaches this window too, so its `system_theme` module is here unchanged.

What is not here: no rendering of PDF or of any format other than DocLang, and no model inference. Segler renders and edits what a model said.

## 8. Packaging

Cloned from slipcase-desktop, one directory per platform under `packaging/`, with that repository's store listing and submission notes as the template. What was measured there is taken as measured here: the Visual C++ runtime linked in with `+crt-static` and the import table checked before packaging; the Linux `Depends` checked against `/proc/PID/maps` on both Wayland and X11; the Mac bundle with the App Sandbox; `CFBundleVersion` from the first-parent commit count. A Store build needs slipcase-desktop's pinned `[patch.crates-io]` on `excelano/winit` until a winit release ships the upstream fix behind its `private-apple-apis` feature, because `winit` 0.30 declares a private CoreGraphics symbol whether or not it is called and the Mac App Store refuses it under Guideline 2.5.1. `build-app.sh` refuses any binary importing a symbol no public framework header declares.

Both stores carry the name **Segler**. The App ID is `com.excelano.segler-desktop` and the App Store Connect Apple ID is 6808761705.

**Linux** is one package for one product, `segler`, carrying both executables, the desktop entry, the two media types with their icons, and a manual page for each binary. The specification names `application/vnd.doclang.document+xml` for the markup and nothing for the archive, so `packaging/linux/mime/doclang.xml` declares `application/vnd.doclang.archive+zip` as a provisional name beside it and says so; the declaration ships in the package because nothing else on a Linux machine makes it, and the day a second DocLang product for Linux exists it moves to a shared package. Apache-2.0 means a DEP-5 copyright file rather than a copied `LICENSE`, which is what Debian wants.

**Windows.** `.dclx` and `.dclg` each get a `uap:FileTypeAssociation` in the manifest, a ProgID in the scripts, and their own icon, so the two kinds tell apart in Explorer the way they do in a Linux file manager; three icon directories is what that costs. `build.rs` is the one build script in this tree and compiles nothing: it hands the MSVC linker `/MANIFEST:EMBED` so the DPI declaration is in the binary before any of this program's code runs, and the certification kit reads the manifest rather than the process. The certification baseline is filled from a kit run against this application's own package and never copied from another.

**A packaged association never claims an extension's default value and the script install does**, so with both installed the script's copy answers a double-click, in either install order, silently. `packaging/windows/README.md` says to run `uninstall.ps1` before installing the package. The two are told apart in Add/Remove Programs by name — the package is **Segler**, the scripts write **Segler (user install)** — because a remedy nobody can aim is not a remedy.

**macOS.** The two type declarations are **imported** where slipcase-desktop's is exported: DocLang is the Foundation's format and `spec.md` is its authority, so exporting the type would claim an ownership the platform would then believe and rank by. The identifiers `com.excelano.doclang-archive` and `com.excelano.doclang-document` are provisional the way the archive media type is, and yield to the project's own the day it declares any. The minimum system version is 11.0, where Apple silicon begins, and the category is Productivity.

Two things the sandbox forces. macOS delivers a double-clicked document as an Apple Event and never as an argument, and receiving one needs an Objective-C method: `crates/segler-desktop/src/opened_document.rs` is that module and the one `allow(unsafe_code)` in this repository. And Save cannot create its temporary file beside the document, since the open panel's grant covers the file and not its directory, so `crates/segler-core/src/replace.rs` carries a macOS arm that stages the rewrite in the directory the platform provides on the document's own volume and lands it with `replaceItemAtURL:`, through `objc2-foundation` bindings that are safe functions, leaving the core's `forbid` untouched. A file saved under the sandbox comes back carrying `com.apple.quarantine` and the process's primary group; both are the platform's and neither reaches a person.

eframe substitutes its own icon for a viewport that names none and hands it to `setApplicationIconImage:`, which outranks the bundle, so the macOS arm passes an empty `IconData` and declines the icon rather than replacing it. Finder and Launch Services draw the bundle's icon either way; only the Dock shows the difference.

`.github/workflows/apple-silicon.yml` runs the suite, clippy's macOS arms, the corpus and a Launch Services open on arm64 on every push, which is the only arm64 execution before TestFlight, since the Mac lane is an Intel machine.

crates.io carries all three crates: `publish-crate.yml` publishes from the version tag, `segler-core` first, so the library, the CLI and the application are each a `cargo install` away on any platform with a toolchain.

---

## 9. Naming

The product is **Segler**, German for sailor. Everything that is an identifier stays lowercase: the crates `segler-core`, `segler` and `segler-desktop`, the binaries `segler` and `segler-desktop`, the application id `segler-desktop`, the bundle identifier `com.excelano.segler-desktop`. The desktop binary is named in full so that it never collides with the command-line tool on `PATH`, which is slipcase-desktop's arrangement and the reason for it.

Whether the library should also be published under the name `doclang` on crates.io is open. The name is free and it is the name the DocLang project would want for a Rust reference library. Claiming it on their behalf without asking is squatting; the answer is to announce the crate on their list and offer it, and the decision is theirs.

---

## 10. The language a person reads

The window draws in German where the desktop asks for German, and in English
everywhere else. The mechanism is the `potext` crate, written in
slipcase-desktop and made a crate so that more than one application could have
it; this section is what the rest of this repository may assume.

The catalogues live in `crates/segler-desktop/po/`, inside the crate that reads
them rather than beside the workspace, because `include_str!` reaching above a
crate's own directory compiles here and fails in `cargo package`: the tarball
carries only what is under the crate root, and `publish-crate.yml` publishes
all three crates.

**A message is looked up by its English text, never by a key.** So a call site
reads as the sentence a person sees, and a message with no translation is the
original rather than a placeholder.

**A translation that has gone stale is not shown.** When the English changes,
`msgmerge` carries the old German onto the new text and marks it `#, fuzzy`;
`potext` will not load a fuzzy entry, so the window falls back to English until
somebody has read the new sentence.

**`segler-core` has no catalogue and gets none.** It is the model and the
command boundary. Every sentence a person reads is produced in
`segler-desktop`, including `describe`, which turns a `Command` into the status
line — so §7's rule that the interface is a renderer holds for language too,
and a second front end would translate its own words rather than inherit these.
The CLI is not translated: it prints for a terminal and a pipeline, and the
fleet's other command-line tools are English.

**What stays in English is what the file says.** An element's name, an
attribute's name as it is written, and every attribute *value* a person picks
from a list — `unordered`, `chart`, `read_only`, `body`, `background`,
`furniture` — go into the document as they stand, and a translated one would be
a different document. The labels beside them are the window's own words and are
translated: *Kind*, *Level*, *Class*, *Layer*, *Box*. The cell kinds in the
element pane are also the window's, not OTSL's tags, so they are translated
too.

**`crates/segler-desktop/po/update-po.sh` is the only way the catalogues move**, and its `pseudo.sh`
writes the pseudolocale that finds a string which never went through `t` and a
label built to the width of English. Run the second before writing any
translation rather than after.
