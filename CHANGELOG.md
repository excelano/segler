# Changelog

What changed for a person who installed Segler, one section per version. The
store release notes and the apt changelog are written from this file, and every
claim here is checked against the built application rather than remembered.
`git log` is the record of why the code is the way it is; this is not that.

## [0.1.2] - 2026-09-09

### Fixed

- Selecting a heading no longer changes it. A heading whose `level` the
  element pane cannot show, because the document spells it as zero or deeper
  than the field's range, was rewritten to the nearest level the field could
  show the moment the heading was selected: the document was marked modified,
  and a save would have written the substitution to the file. Two documents hit
  it. One whose `level="0"` a finding reports, where clicking the finding is
  the way to reach the heading and clicking it made the finding disappear; and
  a valid one with `level="8"`, which carries no finding at all and was quietly
  turned into a level-6 heading. Selecting an element is not an edit. The
  field's range is the schema's now, `xs:positiveInteger` with no upper bound,
  so a level the specification allows is shown as itself.

## [0.1.1] - 2026-09-09

- **German.** Segler comes up in German on a machine set to German — the
  toolbar, both panes, the dialogs, and the status line after every edit. There
  is nothing to choose: it reads the language the desktop already knows, and
  falls back to English for any other.
- What the document says stays as the document says it. An element's name, an
  attribute's name and the values you pick from a list are the file's own words
  and are never translated; only the window's words around them are.

## [0.1.0] - 2026-09-06

The first packaged build.

- A window whose main pane is the document rendered from the tree and edited
  in place: headings, paragraphs, lists, tables and pictures in reading order.
  Text is edited where it is read, and a table cell where it sits.
- The element tree on the left and the selected element's properties on the
  right: kind, level, label, layer, bounding box, and a table cell's kind.
  Both panes fold away.
- An archive's page images as an optional side panel with the located boxes
  drawn over them, so a doubtful line can be checked against the page.
- Text with bold or italic in it is edited with its tags shown, so a line can
  be corrected without losing its formatting, and formatting can be added.
  Malformed markup is refused, naming the tag, rather than written.
- List items are edited in place, one item at a time.
- Undo and redo for every edit, and a save that writes the opened file with
  the edits applied and nothing else changed. Markup that was not touched is
  written back byte for byte; an archive's page images and assets are
  carried across untouched; a document with nothing changed is not rewritten
  at all.
- Validation against the DocLang schema and rules, with each finding a click
  from its element.
- The `segler` command-line tool over the same library: inspect, validate,
  page, render, table, edit and corpus.
- No network connection of any kind. No account, no telemetry.
- On Windows, `.dclx` and `.dclg` each get their own icon and open here from a
  double-click, whether Segler came from the Microsoft Store or from the
  per-user install script.
- On macOS, the same two icons in Finder, a double-click that opens here
  whether Segler is running or not, and a save that works inside the App
  Sandbox every Mac App Store application runs in.
