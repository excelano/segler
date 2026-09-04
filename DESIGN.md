# Segler — Design Document

**Status:** stages one to four of `§10` on Linux: the lossless tree, the vocabulary over it, both validation layers, the editing session, and a window that draws it (`§7`). The window has not had a keyboard walkthrough yet; nothing in it is packaged.
**Document version:** 2026-09-04
**Amendments:** this document is written before the thing it describes. Building it will contradict parts of it. Every change from here on is marked **Amended** and states what was measured, because a design that quietly rewrote itself to match the code would be worth nothing as a record.
**Implements:** DocLang 0.7, from `spec.md` in `doclang-project/doclang`.
**Section references:** a bare `§4` is this document. `spec §Tables` is the specification. `slipcase-desktop §2` is that repository's design document, which this one borrows from and names when it does.

---

## 1. What this repository is

A desktop application that opens a DocLang document or archive, shows the page image beside the structure a model found in it, and lets a person correct that structure and save a valid document back. Underneath it, the first Rust library that represents DocLang without losing anything.

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

Three things could be called an editor for a machine-native format, and one of them is this.

An XML editor with DocLang awareness, meaning syntax colouring, schema-driven completion and live validation, is a language server plus a shell. A VS Code extension would do it better and sooner. It survives here as a mode, the markup pane in `§7`, not as the product.

An authoring tool, a word processor that writes DocLang, fights the format. DocLang is produced by models from existing documents; a person composing it from nothing has no page image and no reason to prefer it over Markdown.

A review-and-correction tool is the gap. Model output is sometimes wrong, and the people who find out are the ones building training data, auditing conversions, or checking that a contract's tables survived the pipeline. What they need is the page image next to the structure, every box linked to its markup in both directions, and the ability to fix a label, a heading level, a cell, a reading order or a box and save a document that still validates. The official viewer draws the first half of that and edits nothing.

The user is the DocLang community: whoever opens a `.dclx` from disk and wants to see and fix what is in it. If governance engagements follow from that, they follow; nothing in the design is shaped for them.

A session looks like this. Open an archive. The page shows with boxes over it; the structure tree lists the elements on that page in reading order; the markup pane shows the XML. Click a box or a tree row or a tag and the other two follow. Change the heading level from a dropdown, retype a misread word, drag a box edge, mark a table header row as `ched`, move an element earlier in reading order. Validation runs on every edit and the problems list stays visible. Save, and what lands on disk is the archive with a new `document.xml` and the same pages and assets.

---

## 4. Architecture

Three crates in one workspace, and one boundary between them that is the design.

`segler-core` owns everything that is not pixels. The document model, the parser, the serializer, validation, the OTSL grid, the archive format, selection, every edit command, undo and redo, and save. It has no dependency on any user-interface crate and never will.

`segler` is the command-line tool over the core. Inspect, validate, pack, unpack, format, diff, and the conformance corpus runner. It exists to prove the core on real files before a window exists, and it stays useful after.

`segler-desktop` is the window. It asks the core for a view-model of the current page and the current selection, draws it, and sends commands back. It holds no document state of its own beyond what is being drawn.

**The boundary is a view-model and a command set, and it is the reason a native front-end stays possible.** The core exposes a session: open, the view-model for page N, apply a command, undo, validate, save. The view-model is plain data: boxes with ids and normalised coordinates, tree rows with ids and labels, the markup as text with id-to-span mapping. Commands are plain data too: set label, set heading level, set text, move box, reorder, set cell kind. egui is one consumer. A Swift or C# front-end over UniFFI would be another, consuming the same data and sending the same commands, and would need nothing from the egui crate. Logic that appears in `segler-desktop` and that any other front-end would also need is in the wrong crate, and the test for that is whether it touches egui types.

**The model is lossless, and that is the property that separates it from docling.rs.** Every element, attribute, text run, CDATA section and comment in a file survives a parse and a serialize, in order. The spec allows things the editor has no button for, custom vocabularies and `custom` elements among them, and they pass through untouched. If the model cannot represent something the spec allows, the model is wrong. docling.rs reads DocLang into a flat node list with inline formatting baked into Markdown-style strings and its reader ignores what it does not know, which is right for a converter and disqualifying here. It is an import engine behind a feature flag, for opening a PDF that is not DocLang yet, and it never touches the model.

**The serializer is canonical.** Output matches the Python reference toolkit's pretty-printed form byte for byte on the conformance corpus, so that a document that was opened and saved without edits diffs clean, and a document that was edited diffs only where it was edited. This is measured by the corpus command in `§6`, not asserted.

**Amended: the reference toolkit has no serializer, so the target is the file itself.** Read before the model was built, the toolkit's `doclang` package does two things, validate and pack, and the pretty-printer the paragraph above had in mind is docling-core's, which writes DocLang from a converter's model and is not a reference for anything. The measurable target is stronger and simpler: a document parsed and written back without edits is the same bytes. The tree keeps the source of every start tag and every text run and writes it back verbatim until an edit touches that node; only an edited node is regenerated, in one fixed form. So an unedited save diffs clean against the original file rather than against somebody's pretty-printer, and an edited save diffs at the edit and nowhere else. `segler corpus` measures identity on every file under the specification checkout.

**Undo is a command log.** Every edit is a command with an inverse, applied to the model and pushed; undo pops and applies the inverse. The model is not persistent or copy-on-write, because documents are small enough that a command log is simpler and simplicity is the priority.

**Amended: the inverse is a snapshot, not a command.** An inverse command would regenerate the nodes it puts back, and a regenerated node has lost its raw source, so a document edited and undone would serialize differently from the file it came from and the lossless property in this section would hold only until the first undo. What the log records instead is what a command displaced: the children of the element it touched, an attribute's old value, an old name. Undo puts those nodes back as they were, raw source and all, and redo re-runs the command. Measured by `segler corpus`, which edits every element of every file in the corpus through every command that applies to it and undoes it all: every file comes back byte-identical. The session also refuses what it cannot represent, text carrying a character XML cannot hold among them, before it reaches a node.

**The session is the boundary, and it exists before the window does.** `segler-core::session` holds the document, the original archive bytes, the selection, and both stacks, and offers three view-models: the document (version, resolution, page count, whether it is dirty), a page (its image part, its boxes as fractions of the page, its semantic elements as tree rows with an excerpt each), and one element in full (head, attributes, text, markup, position). Commands are plain data addressed by element id: set text, set attribute, set label, set layer, set bounds, rename, move, insert, remove. The CLI's `page` and `edit` commands drive all of it, which is how the boundary was tested against real files before anything drew a pixel.

---

## 5. Dependencies

The rule is slipcase-desktop's, borrowed whole: nothing compiles C. A crate that links a system library is fine; one that builds C is not. The measurement is the artefact rather than the manifest, because the manifest lies in one direction: `cargo tree -i cc` names `wayland-backend`, which declares `cc` as a build dependency and uses it only under its `client_system` feature, and nothing here enables that. Measured on the scaffold: no `.o` or `.a` anywhere under the build directories. That check, and `cargo tree -i cmake` and `-i bindgen` coming back empty, is what the rule means. A build needs a Rust toolchain and nothing else, on all three platforms.

The framework is egui, through eframe. It was chosen after Tauri, and the reasons are pipeline rather than performance: slipcase-desktop is an egui application already through Microsoft Store certification with a three-platform packaging tree, and none of that would transfer to a web-view application. It also keeps both of David's desktop applications on one open-source framework, so a problem found in one is fixable for both. What egui does natively covers most of `§7`: images are textures, boxes are painter calls, the tree is collapsing headers, the table grid is `egui_extras`. Its weak surface is free-form text editing, which `§7` confines. Tauri is the fallback if that surface proves inadequate, and the boundary in `§4` is what makes the fallback cheap.

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

**Amended: the reference toolkit does not run the Schematron as written, and the port follows the corpus where the two part.** Measured on the first comparison run, 101 of 104 files agreed and the three that did not were all one rule, `element-head-placement`, on a list or a table. The toolkit transpiles the Schematron to XSLT itself, and it prefixes every rule context with `//`, which in XPath binds only to the first alternative of a union: in `dl:text | dl:heading | … | dl:list | dl:table` only `text` is ever selected, and the other twenty contexts are evaluated relative to the document node and match nothing. The same defect leaves `index` out of both table patterns and checks only `fcel` cells for text before a head. So the corpus was never judged by those rules, and three of its valid files contain text in one cell or item followed by a head element in a later one, which the rule's XPath would flag and which is not wrong: the per-cell and per-item patterns are what the rule meant. The port scopes that rule to a list's or table's own head and otherwise implements each pattern as written, which means it reports things the toolkit misses, a head element after text in a `heading` among them. Those are defects in the document, not disagreements to paper over, and the toolkit's transpiler is a finding to take upstream.

**The conformance corpus is a command, never a test.** `segler corpus /path/to/doclang` walks the specification checkout's `examples/` and `tests/`, parses and re-serializes each file, and compares three things: the serialized bytes against the file itself, and, once validation exists, the verdict and the list of findings against the Python toolkit's. All of them must agree. It needs the checkout and the toolkit installed, which `cargo test` does not imply, and a test that has to choose between skipping quietly and failing on a machine that was never going to have those is worse than a command run on purpose. Run it before and after any change to the model, the parser, the serializer or the validator.

---

## 7. Shape

One window, three panes, one selection.

**The page pane** draws the page image with a rectangle over every located element. Zoom and pan. Click a rectangle to select; drag an edge to move it, and the model gets a command with the new normalised coordinates. Elements without a location draw nothing here and are reachable from the other two panes. Page images load lazily and unload when far from the current page, because a hundred-page archive at a megabyte a page is not a texture set to hold at once.

**The structure pane** is the tree of the current page's elements in reading order: label, kind, and the first words of the content. Select a row and the box and the markup follow. Drag a row to change reading order. A dropdown on a row changes what can be changed from a list, a heading level or a picture class or a list kind. A table row opens the grid editor, where cell kinds are set and the OTSL sequence is recomputed from the grid.

**The markup pane** shows the XML with the selected element's span highlighted. Editing text content happens here or in a field on the selected element, and the two are the same command. Free-form editing of the XML itself, the language-server mode, is not the first version and may never be; the pane is read-mostly with highlighting until use shows a need, and `egui_code_editor` is the crate to reach for if it does.

**The problems list** sits below and shows validation findings, each linking to its element. It is never hidden while a document has findings.

**Amended: built as described, with three things measured on the way.** The page opens fitted to the pane's width rather than at the image's size, because a page image at a thousand pixels across is wider than the pane on every screen this was tried on; a slider, a Fit button and a 100% button sit in the toolbar. The structure pane shows every semantic element on the page including those inside a picture, which on a figure with recognised labels is forty rows of one-word text and wants collapsing; that is left for the walkthrough to judge. And the Linux theme defect that slipcase-desktop measured reaches this window too, so its `system_theme` module is here unchanged. Text edits commit when the field loses focus, label edits likewise, level and class and layer on change, and removal asks first even though undo restores it, because a keystroke on the wrong row is easy.

What is not here: no preview of the document as a rendered page beyond the reading view the structure gives, no PDF rendering, no model inference. Segler shows what a model said and lets a person fix it.

---

## 8. Packaging

Cloned from slipcase-desktop, one directory per platform under `packaging/`, with that repository's `RELEASE.md`, `CHECKLIST.md` and `store-listing.md` as the templates. What was measured there is taken as measured here: the Visual C++ runtime linked in with `+crt-static` and the import table checked before packaging; the Linux `Depends` checked against `/proc/PID/maps` on both Wayland and X11; the Mac bundle with an exported type declaration and the App Sandbox; `CFBundleVersion` from the first-parent commit count. One more is a Mac App Store rejection under Guideline 2.5.1 for a private CoreGraphics symbol that `winit` 0.30 declares whether or not it is called; slipcase-desktop carries a pinned `[patch.crates-io]` on `excelano/winit` until a winit release ships the upstream fix behind its `private-apple-apis` feature, and a Store build of Segler needs the same patch until then.

The Microsoft Store name Segler was reserved on 2026-09-04. App Store Connect is pending and needs a Mac; if the bare name is refused there the display name becomes "Segler for DocLang", the way Slipcase became "Slipcase Desktop", and everything that is an identifier rather than a name is unaffected.

**Amended: App Store Connect accepted the bare name the same day.** The App ID `com.excelano.segler-desktop` was registered in the developer portal and the app record Segler, platform macOS, SKU `segler-desktop`, was created on 2026-09-04 with full user access; its App Store Connect Apple ID is 6808761705. The fallback above was not needed and both stores now say Segler. Two things were measured on the way. Apple's API documentation says new apps are created on the App Store Connect website and not through the API, so this was a browser step and will be for any successor; the installed API key is for uploads. And Apple's help says a reserved name expires if no build is uploaded within a period it does not state on that page, so the reservation is a deadline as well as a name: if the window closes on stage four of `§10` before a Store build exists, a scaffold build through the packaging cloned from slipcase-desktop is what holds it.

The CLI ships the way the fleet's Rust CLIs do, through cargo-dist, with a `dist-workspace.toml` that names the `segler` crate alone. The library ships to crates.io. Neither is wired yet; both arrive when there is a version worth cutting.

---

## 9. Naming

The product is **Segler**, German for sailor, after DocLang's sail logo. Everything that is an identifier stays lowercase: the crates `segler-core`, `segler` and `segler-desktop`, the binaries `segler` and `segler-desktop`, the application id `segler-desktop`, the bundle identifier `com.excelano.segler-desktop`. The desktop binary is named in full so that it never collides with the command-line tool on `PATH`, which is slipcase-desktop's arrangement and the reason for it.

Whether the library should also be published under the name `doclang` on crates.io is open. The name is free and it is the name the DocLang project would want for a Rust reference library. Claiming it on their behalf without asking is squatting; the answer is to announce the crate on their list and offer it, and the decision is theirs.

---

## 10. Order of work

The core first, proved by the CLI, with a window only when there is a model for it to draw.

First, the model and the round trip: parse every file in the conformance corpus and serialize it back to the reference toolkit's bytes. Nothing above this can be built until it holds, and it is a shippable library and CLI by itself.

Second, validation: the XSD port and the Schematron patterns, agreeing with the reference toolkit on the corpus.

Third, the session boundary: view-models, commands, undo, save, exercised by the CLI before any window exists.

Fourth, the window, on Linux first. Then the platform arms and packaging, cloned from slipcase-desktop, Windows and Mac in the order their lanes are free.

Fifth, import through docling.rs behind its flag.
