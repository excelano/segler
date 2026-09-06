# Changelog

What changed for a person who installed Segler, one section per version. The
store release notes and the apt changelog are written from this file, and every
claim here is checked against the built application rather than remembered.
`git log` is the record of why the code is the way it is; this is not that.

## [0.1.0] (unreleased)

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
