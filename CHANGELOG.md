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
- Undo and redo for every edit, and a save that writes the opened file with
  the edits applied and nothing else changed. Markup that was not touched is
  written back byte for byte; an archive's page images and assets are
  carried across untouched.
- Validation against the DocLang schema and rules, with each finding a click
  from its element.
- The `segler` command-line tool over the same library: inspect, validate,
  page, render, table, edit and corpus.
- No network connection of any kind. No account, no telemetry.
