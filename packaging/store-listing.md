# Store listing text

One draft, used twice. Both stores want the same things at different lengths,
so everything here is written to the shorter limit. Nothing here is submitted
yet; the Windows and Mac lanes copy from this file into their forms and record
in their `SUBMITTING.local.md` what the form did with it.

This is written from `CHANGELOG.md`, not beside it. Every claim below appears
there first, checked against the built application. If the two disagree, the
changelog is right and this is stale.

Limits, so a later edit does not overrun them:

| Field | Microsoft Store | Mac App Store |
| --- | --- | --- |
| App name | unmeasured | 30 |
| Description | 10,000 | 4,000 |
| Short description | 1,000 | — |
| Subtitle | — | 30 |
| Promotional text | — | 170 |
| Keywords | 7 terms | 100 characters |

## App name

    Microsoft Store   Segler
    Mac App Store     Segler

Both reservations took the bare name, on 2026-09-04, so the five places that
carry it agree: Partner Center, App Store Connect,
`Package/Properties/DisplayName`, `CFBundleDisplayName`, and the product page
when there is one. The name is German for sailor. The DocLang logo is a
stylised document and not a sail; the two marks are meant to look unrelated.

## Subtitle (Mac App Store, 30)

Edit DocLang documents

## Promotional text (Mac App Store, 170)

Open a DocLang document, read it as a reader would, correct it where you read it, and save the same file with your edits and nothing else changed.

## Short description (Microsoft Store, 1,000)

DocLang is the open markup format for documents that language models read and write: the structure, the text, the layout and the reading order of a document in one file. Most DocLang is produced by a model from a PDF or a scan, and a model is sometimes wrong.

Segler opens a DocLang document or archive and shows it as a reader would see it: headings, paragraphs, lists, tables and pictures in reading order, with the element tree beside it. Retype a misread line where you read it. Change a heading's level, fix a table cell, move a paragraph, correct a bounding box. When the archive carries the page images the model read, open them beside the document to check a doubtful line against the page.

Every edit can be undone. What is saved is the file you opened with your edits and nothing else changed.

## App features (Microsoft Store, up to 20 bullets of 200 characters)

    The document as a reader sees it: headings, paragraphs, lists, tables and pictures in reading order.
    Edit text where you read it, and a table cell where it sits.
    The element tree beside the document, and every property of the selected element in reach: kind, level, label, layer, bounding box.
    Page images beside the document when the archive carries them, with the located boxes drawn over them.
    Undo and redo for every edit.
    Validation against the DocLang specification, with each finding a click from its element.
    Saves the file you opened with your edits and nothing else changed. Markup you did not touch is written back byte for byte.
    No network connection of any kind. No account, no telemetry, nothing sent anywhere.
    Open source, and so is the format it edits.

## Description (both, written to 4,000)

DocLang is the open markup format for documents that language models read and write: the structure, the text, the layout and the reading order of a document in one file, from the LF AI & Data Foundation. Most DocLang is produced by a model from a PDF or a scan, and a model is sometimes wrong: a misread line, a heading taken for a paragraph, a table cell in the wrong column.

Segler is an editor for those documents.

WHAT YOU SEE

The document as a reader would see it: headings, paragraphs, lists, tables and pictures in reading order. The element tree beside it, so the structure the model found is visible next to the text it found. The selected element's properties: its kind, its level, its label, its layer, its bounding box on the page. And when the archive carries the page images the model read, those images beside the document with the located boxes drawn over them, so a doubtful line can be checked against the page.

WHAT YOU CAN DO

Retype a line where you read it. Change a heading's level. Fix a table cell, or change its kind from body to header. Move a paragraph. Correct a bounding box. Remove an element that should not be there. Every edit can be undone.

Segler checks the document against the DocLang specification as you work and lists what it finds, each finding a click from its element.

WHAT A SAVE DOES

Save writes the file you opened with your edits applied and nothing else changed. Markup you did not touch is written back byte for byte, including comments and whitespace. An archive's page images and assets are carried across untouched. A document you did not change is not rewritten at all.

WHAT IT DOES NOT DO

No network connection of any kind. No account. No telemetry, no analytics, no crash reporting. Nothing about you or your documents is sent anywhere, because there is nowhere for it to be sent.

It does not convert. Segler edits DocLang that already exists; producing DocLang from a PDF is a converter's job.

OPEN SOURCE

Segler is open source under the same licence as DocLang itself, and the library underneath it is a separate crate anyone can build on: github.com/excelano/segler.

## Keywords

**Mac App Store** (100 characters, comma-separated, no spaces after commas):

    doclang,dclx,dclg,document,editor,markup,xml,docling,layout,review

**Microsoft Store** (seven terms):

    doclang, dclx, document editor, markup, XML, docling, layout

## Screenshots

Each lane takes its own with its platform's script, against the packaged
application, light theme first because both platforms ship light by default,
with the pointer parked off the window and the window photographed by its
handle. The page image panel is open in at least one shot, because it is the
one thing in this application no other DocLang tool has.

**Windows, 2026-09-04.** Two taken with `packaging/windows/screenshot.ps1`
against the installed MSIX, at 1366x768, in the light theme:

    01  the document, the tree and the element pane, at rest
    02  the same with the page image panel open, boxes drawn over the scan

They are in `dist/screenshots/` and are not committed; `dist` is where every
built artefact goes and these are built from a document rather than from
source.

**The document is not the one this file first named.** It says the DocLang
viewer's `2501.17887.dclx`, and that archive is not on the Windows machine —
`~/clones` is a Linux path. What was used instead is a two-page archive written
for the purpose, *Sailing directions for the western approaches*: a heading
hierarchy, running text with bold and italic, a four-column table with a header
row and a caption, an ordered and an unordered list, and two page images drawn
to match. It is a better listing shot than a research paper, because every
feature the listing claims is visible in one frame and none of it is somebody
else's copyright. **It is not a substitute for the real corpus**, and
`CHECKLIST.md` still asks for an archive with pictures as well, which this one
has none of.

Before either goes to Partner Center, look at it. A correct size is not a good
screenshot, and the script says so when it writes one.
