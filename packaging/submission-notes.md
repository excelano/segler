# Submission notes

What a store submission needs from a person and no file supplies: the notes an
App Review or certification reader is handed, the answers a form asks that no
build can give, and the reasoning behind the screenshots.

The listing text itself is not here. It is `store-listing.toml` beside this,
which `ship` checks before the tag and pushes to both stores on every release,
and what a release tells them changed is `release-notes.toml`. A field edited
in this file would reach nobody.

## App Review notes

Segler is an editor for DocLang documents. DocLang is the open markup format for documents that language models read and write, from the LF AI & Data Foundation. No account, no sign-in, no test credentials, and no network connection of any kind are needed to test it.

**Open a document first — with nothing open, the window is empty and there is nothing to try.** One is at https://github.com/excelano/segler/releases/download/v0.1.3/sailing-directions.dclx — *Sailing directions for the western approaches*, a two-page archive written for this purpose and carrying every feature the listing claims: a heading hierarchy, running text, a four-column table with a caption, ordered and unordered lists, a picture, and the two page images the document was read from. It is the same document the screenshots show. Download it and open it, or launch the application and use Open. Nothing in it is anybody else's copyright.

What to look at once it is open: the document rendered in reading order in the main pane, the element tree beside it, and the selected element's properties below that. Click a line of text and edit it where you read it. With the page images present, the located boxes are drawn over the page so a doubtful line can be checked against the scan. Save writes the same file back with the edit and nothing else changed.

The application declares the DocLang document and archive types and claims both at rank Default, not Owner, with a role of Editor. It is one editor for a format other applications also open, and Save writes the document back, which is what Editor rather than Viewer says.

The App Sandbox is on with exactly two entitlements: the sandbox itself and read-write access to user-selected files, which is the grant a person gives by choosing a document in the open panel. There is no network entitlement, no temporary exception, and the application makes no network request. A save replaces the file the person chose, staged in the replacement directory macOS provides on the file's own volume, because that grant does not allow a temporary file beside the document.

The full privacy statement is at https://excelano.com/legal/#segler and the complete source is at https://github.com/excelano/segler.

## Screenshots

Each lane takes its own with its platform's script, against the packaged
application, light theme first because both platforms ship light by default,
with the pointer parked off the window and the window photographed by its
handle. The page image panel is open in at least one shot, because it is the
one thing in this application no other DocLang tool has. **Something is
selected in every shot**, and an edit is under way or just done in more than
one, because a screenshot of an editor at rest is what guideline 2.3.3 sends
back.

**Windows, 2026-09-06**, from `v0.1.0` (`b1b71f0`), taken with
`packaging/windows/screenshot.ps1` against the **installed MSIX** - the
association is what opens the document, so the window photographed is the
packaged build and not a developer one - at 1366x768, light theme:

    01  the document, the tree and the element pane, at rest
    02  the same with the page image panel open, boxes drawn over the scan
    03  page two: a picture from the archive's assets, with its caption
    04  a document with six findings, each naming the element that carries it

They are in `dist/screenshots/` with the documents they were taken from, and
are not committed; `dist` is where every built artefact goes.

**These four are the set Apple rejected, in its Windows spelling**, and they
have not been retaken. Nothing is selected in any of them and no edit is under
way, which is the whole of what guideline 2.3.3 objected to on the other
platform. Partner Center has not asked, and the same criticism is true of them:
retake them before the Microsoft Store submission goes in.

**The replacement four are recipes.** `packaging/windows/shots.ps1` carries
them, written to match the macOS set frame for frame, and every coordinate they
name was read off a 1366x768 frame of this document: the page image toggle at
423,40, the forward arrow at 353,40, the picture on page two at 620,440,
Newlyn's high water cell at 956,385, the opening line at 600,139 and the
*Hazards* heading at 350,494. `shots.ps1 -Reference` takes that frame, which is
how they are read again when the window changes; a recipe naming a coordinate
that has been blanked stops all four rather than photographing a click that
landed on nothing.

**The set is taken on the runner.** `windows.yml` has an optional `screenshots`
input - `reference` for the one frame, `full` for the set - and the run raises
the desktop from the 1024x768 a hosted runner starts at, installs the
association with `install.ps1` so the shell opens the document with Segler, and
uploads the result. The window photographed is therefore the release binary of
the commit, not the MSIX; the Mac has the same gap for the same reason and the
commit is what makes them the same application.

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
copyright. **It is not a substitute for the real corpus**, which `segler corpus`
runs against the specification checkout.

**The document used is `packaging/review/sailing-directions.dclx`**, which is
the same archive under the name it has now that it is committed rather than
sitting in one machine's `dist/screenshots/`. It is the document the review
notes send a tester to, at
`https://github.com/excelano/segler/releases/download/v0.1.0/sailing-directions.dclx`,
so the file the listing shows and the file a reviewer downloads are one file.
`packaging/review/README.md` is why it is served from the release.

**Three of the four were retaken on 2026-09-06, and why the check that passed
them was the wrong check.** The 2026-09-05 set was compared with a fresh
capture from the tagged package pixel by pixel, and `01-document.png` came back
identical inside the window. That proved the binary draws the same thing. It
did not prove the pictures were any good, and one of them was not:
`03-picture.png` was a photograph of the desktop - a terminal, an Explorer
ribbon, and a sliver of Segler - which had been in the store folder since it
was taken. Diffing one frame and concluding four is the mistake; each frame is
now looked at.

The cause was in `screenshot.ps1`, which called `SetForegroundWindow` and did
not check it. That call is advisory - Windows refuses it from a process that
does not own the foreground and returns false - and the capture is
`CopyFromScreen` over the window's rectangle, so a window that stayed behind is
photographed as whatever is on top of it. The script now taps ALT to release
the foreground lock, retries, verifies with `GetForegroundWindow`, and refuses
rather than writing; it polls the frame until two reads agree; and it checks
the foreground again between settling and the shutter. Those first two are what
the paragraph above already claimed it did, which is its own lesson.

It also drives the window, for the same reason `packaging/macos/screenshot.sh`
does: a frame of an editor with nothing selected is a frame of a viewer, and
the toolbar has no shortcut for the page image or the page arrows. `-Do` takes
the actions as one ordered list - `click`, `double`, `type`, `key` - rather
than as the four repeatable flags the Mac script has, because PowerShell binds
a parameter once and four separate parameters could not say which came first.

**The four, all from the installed v0.1.0 package at 1366x768, light theme:**

    01  nothing pressed - the document, the tree and the element pane, at rest
    02  423,40 - the page image panel open, boxes drawn over the scan
    03  353,40 then 620,440 - page two, then the picture itself: it is
        selected in the tree and on the page at once, and the element pane
        carries its kind, path, class, layer, box and markup
    04  the six findings, from `problems.dclg`

01, 02 and 03 are `packaging/review/sailing-directions.dclx`, so the name in
the title bar is the name of the file a reviewer downloads. 04 is the faulty
copy, which is a different document on purpose and is not committed.

Shot 03 is better than the one it replaces was ever going to be. The 09-05
attempt reached page two but selected nothing, so the element pane sat empty
under a listing that claims an element's properties are in reach; the clicks
now land because the window is verified in front before they are sent.

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
correct frame.

**macOS, 2026-09-06**, from `v0.1.0` (`b1b71f0`, build 41), at 1440x900,
light appearance. Four frames of the document at rest, the same as Windows.
**Apple rejected them on 2026-09-09 under guideline 2.3.3**, submission
`7a2e17b6-64db-40a1-bcea-2d8e529f881b`: *the Mac screenshots do not show the
actual app in use in the majority of the screenshots*. The rejection is right,
and reading the four back says why in one line. Nothing was selected in any of
them, so the element pane read *Select an element on the page or in the
structure* in all four; no edit was in progress in any of them; and three of
the four were one short document sitting still. Every feature the description
claims was present in the window and none of it was happening. A frame of an
editor with nothing selected is a frame of a viewer.

**macOS, 2026-09-09**, from the same commit and the same bundle, at 1440x900,
light appearance. Five, and every one of them has the element pane populated,
which is the thing the rejected set never had:

    01  correcting a table cell in place: the cell open with the new value
        being typed, the table selected, and the pane showing the cell's row,
        column and kind
    02  the page image beside the document, a paragraph selected, and its
        located box drawn over the scan with the same coordinates in the pane
    03  the correction committed: the title carries the modified mark, Save
        and Undo have come on, the status line says which cell changed, and a
        heading is selected with its level, layer and box in reach
    04  page two: the picture selected, its class, its caption, its box on
        the scan, and its markup
    05  a document with six findings, one of them clicked, and the element it
        names selected with the class the specification does not have showing
        in the pane

The first four are `packaging/review/sailing-directions.dclx`, the document the
review notes point at. The fifth is a copy of it with six faults put in by
hand, the kind a model makes: an element that is not DocLang, a heading at
level 0, a list class the spec does not have, a table row one cell short, a
location block with its x coordinates reversed, and a location past the page's
edge. It is in `dist-dev/screenshots/` beside the shots and is not committed;
`segler validate` on it reports the six.

`screenshot.sh` grew what the five needed: `--double` for the double click that
opens a block for typing, `--type` for the correction, and `--key` for the
select-all before it. The recipes, each against `dist-dev/Segler.app` with the
document under `/Users/Shared/DocLang/`:

    01  --double 1020,384 --key cmd+a --type "-0:38"
    02  --click 422,39 --click 520,300
    03  --double 1020,384 --key cmd+a --type "-0:38" --click 352,500
    04  --click 422,39 --click 352,39 --click 70,153
    05  --click 400,831

Two faults in the driving are worth naming, because both looked like the
window ignoring input and neither was. The loop that runs the actions read them
as its own standard input, so the helper it ran inherited the file and
swallowed the actions after the one it was called for; they never ran. And a
`--key cmd+a` left command down in the session's modifier state, so every
character typed after one arrived as a shortcut and typed nothing. The list is
read on its own descriptor now, the helper is given no input at all, and a
modifier is let go as its own event.

**A shot of a level-0 heading is the one frame this set cannot have**, and
finding that out is what the fourth finding's click was changed away from. In
0.1.0 selecting a heading whose `level` is outside 1 to 6 rewrote it on the
spot: the element pane's drag value clamps to draw such a heading, and the
clamp reached the document as though it had been typed. Clicking the finding
that reports `level="0"` set it to 1, marked the document modified and made the
finding disappear, and a valid `level="8"` heading became a level-6 one the
moment it was selected. `05` clicks the list's finding instead, which changes
nothing. The fault itself is not a screenshot matter and is fixed separately.

Before either goes to Partner Center, look at it. A correct size is not a good
screenshot, and the script says so when it writes one.
