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

Segler opens a DocLang document or archive and shows it as a reader would see it: headings, paragraphs, lists, tables and pictures in reading order, with the element tree beside it. Retype a misread line where you read it, in a paragraph, a list item or a table cell. Change a heading's level, change a cell from body to header, correct a bounding box. When the archive carries the page images the model read, open them beside the document to check a doubtful line against the page.

Every edit can be undone. What is saved is the file you opened with your edits and nothing else changed.

## App features (Microsoft Store, up to 20 bullets of 200 characters)

    The document as a reader sees it: headings, paragraphs, lists, tables and pictures in reading order.
    Edit text where you read it: a paragraph, a list item, or a table cell where it sits.
    Bold and italic are shown as tags while you type, so a correction keeps them and formatting can be added.
    The element tree beside the document, and the selected element's properties in reach: kind, class, level, label, layer, bounding box, and a table cell's kind.
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

Retype a line where you read it - a paragraph, a list item, a table cell. Where a line carries bold or italic, its tags are shown in the field so a correction keeps them, and so formatting can be added. Change a heading's level, or a list's or picture's class. Change a cell's kind from body to header. Correct or clear a bounding box. Change an element's label or its layer. Remove an element that should not be there. Every edit can be undone.

Segler checks the document against the DocLang specification as you work and lists what it finds, each finding a click from its element.

WHAT A SAVE DOES

Save writes the file you opened with your edits applied and nothing else changed. Markup you did not touch is written back byte for byte, including comments and whitespace. An archive's page images and assets are carried across untouched. A document you did not change is not rewritten at all.

WHAT IT DOES NOT DO

No network connection of any kind. No account. No telemetry, no analytics, no crash reporting. Nothing about you or your documents is sent anywhere, because there is nowhere for it to be sent.

It does not convert. Segler edits DocLang that already exists; producing DocLang from a PDF is a converter's job.

OPEN SOURCE

Segler is open source under the same licence as DocLang itself, and the library underneath it is a separate crate anyone can build on: github.com/excelano/segler.

## URLs

Both forms ask for the same three, and both lanes take them from here:

| Field | URL |
| --- | --- |
| Privacy policy | https://excelano.com/legal/#segler |
| Support | https://excelano.com/segler/#support |
| Marketing / website | https://excelano.com/segler/ |

The page at `excelano.com/segler/` is the support and marketing URL both, the
way slipcase-desktop's is. It went live on 2026-09-06 saying the release is on
its way; the store badges and the apt install block appear on it as each lands,
by switches at the top of its source. The reviewer's document is a release
asset on GitHub, not a page; `packaging/review/README.md` has the URL shape.

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

**Windows, 2026-09-05**, retaken after the editing work and the toolbar fix,
so no shot shows a control that is no longer there. Four, with
`packaging/windows/shot` scripts against the **installed MSIX** - each one
reports the `WindowsApps` path it opened, so a developer build cannot creep
into a listing by accident - at 1366x768, light theme:

    01  the document, the tree and the element pane, at rest
    02  the same with the page image panel open, boxes drawn over the scan
    03  page two: a picture from the archive's assets, with its caption
    04  a document with six findings, each naming the element that carries it

They are in `dist/screenshots/` with the documents they were taken from, and
are not committed; `dist` is where every built artefact goes.

**Two things the capture script now refuses on**, both learned by watching it
lie. It checks the window is actually in the foreground, because a shell
launch does not always accept `SetForegroundWindow` and the first attempt
wrote a correctly sized photograph of a terminal and reported success. And it
polls the geometry until it stops moving, because a cold start of the packaged
application is still positioning itself seconds in, and a rect read during
that produced a window sitting 69 pixels below where it was asked to be with
the desktop showing along two edges. A correct size is not a good screenshot,
and neither is a correct size and a foreground window.

**The document is not the one this file first named.** It said the DocLang
viewer's `2501.17887.dclx`, which is not on the Windows machine - `~/clones`
is a Linux path. What was used instead is a two-page archive written for the
purpose, *Sailing directions for the western approaches*: a heading hierarchy,
running text with bold and italic, a four-column table with a header row and a
caption, an ordered and an unordered list, a picture with a caption and its
own inner text, and two page images drawn to match. Every feature the listing
claims is visible across four frames and none of it is somebody else's
copyright. **It is not a substitute for the real corpus**, which is what
`CHECKLIST.md` asks for and what the Linux lane has.

**macOS.** `packaging/macos/screenshot.sh` against a development-signed
universal bundle built from the release commit with `--outdir dist-dev`,
because the Store package cannot be launched off the Store and so no shot can
ever be of the exact artefact uploaded; the commit is what makes them the
same application. Its default is 1440x900, one of the four sizes App Store
Connect accepts and the largest an Intel Mac without a Retina panel can make;
2560x1600 and 2880x1800 want a backing scale of 2. It captures the window by
its id rather than its rectangle, parks the pointer in a corner first, and
refuses a capture of the wrong size. It needs Accessibility permission for
the terminal that runs it. The script was run on 2026-09-06 against the
specification's own `archive-demo`, packed into a `.dclx`, and produced a
correct frame; the four shots for the listing are taken from the release
commit with the same document set as Windows, and recorded here when they
are.

Before either goes to Partner Center, look at it. A correct size is not a good
screenshot, and the script says so when it writes one.
