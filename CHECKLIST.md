# Checklist: the things only a hand can test

Every window defect this project has found was found by David at the keyboard,
and none by a test: a typed box value that never committed, a heading level
that reached the document clamped, a one-frame flash of the last tree row, a
click on a table cell that selected the table, a side panel that collapsed to
a sliver. The suite passed and the corpus agreed through all of them. This
file is what a person runs instead.

It is the list, not the log. What each run found is in `git log`. A finding
earns a place here only when it changes how you run the list.

Run against the packaged application, not a developer build: on Linux the
`.deb` installed with `apt`, on Windows the MSIX, on macOS the bundle. Several
items are properties of the package rather than the code.

The document to run against is a scratch copy of an archive with page images,
tables and pictures; the DocLang viewer's `2501.17887.dclx` is the one both
walkthroughs used.

## Every platform: the window

1. **The window follows the desktop's theme**, light and dark, and switching
   the desktop while the window is up switches the window. On Linux this is
   the XDG portal path in `system_theme.rs`, which nothing else exercises.
2. **A double-click on a `.dclx` and on a `.dclg` in the file manager opens
   it here**, with the document rendered and the tree filled. The file
   manager draws each with its own icon: a blue page for the archive, a cream
   one for the markup, both carrying the sail.
3. **Double-click a paragraph, retype a line, click elsewhere.** A single
   click selects; the second opens the field. The text changes in the
   document, the element pane shows the new text, Undo restores it. Escape in
   the middle of an edit discards it.
3a. **A paragraph with bold or italic in it opens showing its tags**, as
    `The <bold>western approaches</bold> are wide`, in the document pane and
    in the element pane's Text box alike. Fix a word away from the tags and
    they survive. Add an `<underline>` and it takes. Leave a tag unclosed and
    the commit is refused with that tag named, the document untouched. Type
    the line with no tags at all and they go, deliberately and without a
    question, because the field was showing them.
3b. **A list item edits like a paragraph.** Double-click one, retype it,
    click away: only that item changes, and the status line names it. Until
    2026-09-04 a list was the one thing in a document that could not be
    corrected at all.
4. **Click a table cell.** The cell is selected, not the table: the status
   line names the cell one-based, and the element pane offers the cell's kind.
   Header cells have a visible fill in both themes and their text is legible
   on it.
5. **Type a box coordinate in the element pane and press Tab.** The value
   commits; Undo restores it.
   Then the level clamp, **on a level-1 heading**: set its level to 0. The
   field clamps to 1, which is what it already was, so nothing reaches the
   document - no dirty marker and nothing to undo. On a heading at any other
   level the same keystroke is a real change to 1 and *should* go dirty; the
   defect this guards was a clamp that wrote an edit when nothing had
   changed. Said in full because the run of 2026-09-04 spent a step deciding
   which of the two it was looking at.
6. **Click a finding in the problems pane.** The element is selected and the
   tree scrolls to it without the last row flashing. Run this at the desktop's
   fractional scale as well as at 1x; the flash only ever showed at 1.25.
7. **Toggle the page image panel, drag its divider, toggle it off and on.**
   The width holds. Fit and 100% behave. The button reads *Page image*. With
   the panel closed the zoom slider, the percentage, Fit and 100% are not in
   the toolbar at all: they drive the panel and nothing else, and they were
   shown always until the run of 2026-09-04 asked what they did when it was
   shut.
8. **Ctrl+1 and Ctrl+2 fold the structure and element panes**, and the
   document pane takes the space.
9. **A picture's inner rows are folded in the tree** until something
   *inside* it is selected - a click on one of its inner boxes in the page
   image panel, or a finding that points there. Selecting the picture itself
   leaves it folded, deliberately: `DESIGN.md` 7 says the selection has to
   land inside, and forty axis labels unfolding at a click on the image would
   be the thing the folding exists to prevent. The triangle is how a person
   opens it by hand.
10. **Delete asks first.** Delete on a selected element opens the confirm;
    Cancel leaves the document alone; confirming removes it and Undo brings it
    back.
11. **Save, then diff against the original.** The only difference is the edit.
    Framing whitespace, CDATA and every untouched element are byte for byte.
    For an archive, `pages/` and `assets/` are unchanged.
12. **Close with unsaved edits.** The dialog offers Save and close, Discard,
    Cancel, and each does what it says.

## Every platform: the language the window comes up in

Added 2026-09-09 with German. Two runs, and the second one needs no German.

**In the pseudolocale, first.** `./po/pseudo.sh`, then a debug build with
`POTEXT_LANG=en-x-pseudo`. Every string this application owns comes back
bracketed, accented and 40% long, so three things show themselves: a sentence
still in plain English never went through `t`; one with no brackets is a
message the catalogue never saw; and a label with its end cut off is a layout
built to the width of English, which is what German meets first.

**A screenshot only proves the paths that drew.** The toolbar, both panes and
the status bar were checked that way on 2026-09-09; the three dialogs were not,
because none of them was open. Walk them: edit a run so the flatten prompt
appears, remove an element, and close the window with unsaved changes. Each is
a heading and a sentence and two or three buttons, and each is a place a
missed string would sit unseen.

**Then in German**, with the machine set to German or `POTEXT_LANG=de`. Open a
`.dclg` and a `.dclx`, edit a value, undo it, save, and read the status line
after each: those sentences are built from `describe`, so they are the half a
screenshot of a fresh window never reaches. An element's name and every
attribute value stay English on purpose — `DESIGN.md` §10 says which and why —
so a `<paragraph>` in the structure pane beside a German label is right rather
than a miss.

## Linux: the package

13. `sudo apt install ./dist/segler_X.Y.Z_amd64.deb` installs with no
    maintainer-script output, and `dpkg -V segler` is silent.
14. `man segler-desktop` and `man segler` open, and `segler --version` says
    the version the package declares.
15. Files, at list size and at icon size, draws a `.dclx` and a `.dclg` with
    their own icons and offers Segler under *Open With*.
16. `packaging/linux/check-libraries.sh` passes on both backends. It is a
    command and not part of CI because it needs a display.

## Windows: the package

Against the MSIX, installed. `dist/Segler-X.Y.Z.0-x64-signed-certified.msix` is
the copy to install; the one beside it with no suffix is unsigned and is what
the Store is given, and the shell will not accept it. Items 21 and 22 are the
script route and want the package uninstalled first.

17. `Add-AppxPackage dist\Segler-X.Y.Z.0-x64-signed-certified.msix` succeeds,
    and **Segler** appears in the Start menu with the sailboat on it. Pin it:
    the pinned tile and the taskbar button carry the same drawing.
18. **The taskbar button is not on a coloured square.** `BackgroundColor` is
    `transparent`, so a missing `altform-unplated` asset puts the icon on a
    plate of the user's accent colour; the assets exist and `resources.pri` is
    what makes them resolve, and both fail silently. This is the one item that
    needs looking at rather than reading, and slipcase-desktop found it by
    photographing a taskbar.
19. **Explorer draws a `.dclx` and a `.dclg` with their own icons**, blue and
    cream, at list size and at extra-large size, and does not draw the same
    picture for both. Both offer Segler under *Open with*.
20. **Double-click each.** The packaged binary opens it - check the path is
    under `WindowsApps` and not a copy somewhere else - and no console window
    appears behind the window.
21. **`install.ps1` with the package uninstalled.** Both extensions register,
    `reg query "HKCU\Software\Classes\.dclx" /s` shows the ProgID, and a
    double-click opens the copy under `%LOCALAPPDATA%\Programs\Segler`. Then
    `uninstall.ps1`, and both extensions go back to having no handler rather
    than to a broken one.
22. **The overlap, deliberately.** Register both, then double-click a
    `.dclx` and check *which* Segler answers. The script's copy under
    `%LOCALAPPDATA%` wins, silently, in either install order - no picker, no
    prompt - because a packaged association never claims the extension's
    default and the script's does. That is why `README.md` says to run
    `uninstall.ps1` before installing the package: without it somebody
    installs from the Store and goes on running the older copy. Read the
    running process's path rather than the window, which looks the same
    either way.
23. **The window at 125% and 150% display scaling**, and moved between two
    monitors at different scalings if there are two. The DPI declaration is in
    the embedded application manifest and takes effect before any of this
    program's code runs; winit sets the same awareness at run time, so a defect
    here shows as a wrong first frame rather than a wrong window.
24. **Add/Remove Programs.** With both installed there are two rows:
    **Segler** at `X.Y.Z.0`, which is the package and Windows' own entry, and
    **Segler (user install)** at `X.Y.Z`, which is the scripts'. They read
    alike but for that suffix - same icon, same publisher - and the suffix is
    there because on 2026-09-04 they did not, and the wrong one was removed.
    Remove the user install: the association, the Start menu shortcut and the
    files go, and a double-click falls back to the package.

## macOS: the package

Against a **signed** bundle, because almost nothing below is true of an
unsigned one: the App Sandbox is inert until the entitlement is inside a
signature, so an unsigned bundle carrying `Segler.entitlements` is not
sandboxed and every measurement against it is meaningless. A Store-signed
bundle cannot be launched here at all, so the walkthrough is against a
development-signed universal bundle from the same commit, and the real
article through TestFlight.

    ./packaging/macos/build-app.sh --universal --sign "Apple Development: …" --outdir dist-dev
    /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f dist-dev/Segler.app
    ./packaging/macos/check-install.sh dist-dev/Segler.app

25. **`check-install.sh` reports nothing mechanical wrong**, and says which
    kind of build it is looking at. Its Launch Services rows read NO after a
    `--store` build in the same `dist/`, correctly: that build withdraws its
    own claim because the kernel would kill it if a double-click chose it.
26. **Finder draws a `.dclx` and a `.dclg` with their own icons**, blue and
    cream, at icon size and in list view, and Get Info names the Kind as
    *DocLang archive* and *DocLang document* rather than *Document*.
27. **Double-click each, cold.** With Segler not running, the window comes
    up showing that document, its name in the title, and **no dialog**. The
    failure this guards against is specific: *Segler cannot open files in the
    "DocLang archive" format*, which is what AppKit says when nothing is
    listening for the Apple Event.
28. **Double-click one, warm.** With Segler already running and a different
    document open, double-click another. It replaces what is on screen. This
    is a different code path from the cold launch and one of the three
    registration moments passes it while failing the cold one. Like Cmd+O it
    does not ask about unsaved edits, which is the same on every platform.
29. **Save under the sandbox.** Edit a line and press Cmd+S. The file is
    rewritten and only the edit differs. Then look at what the platform did:
    the file carries `com.apple.quarantine` naming `segler-desktop` as its
    agent, because a sandboxed process's writes are marked, and it took the
    process's primary group, as a renamed replacement would anywhere. Both
    are the platform's and neither reaches a person; measured 2026-09-06.
30. **Save a document on a second volume.** A mounted disk image is enough.
    The rewrite has to wait on the document's own volume or the replacement
    fails with `EXDEV`; slipcase-desktop found that with the rewrite under
    `TMPDIR`, and `replace.rs` asks for the replacement directory beside the
    document for that reason.
31. **The window at 2x**, on a Retina panel, since the Intel Mac this was
    built on has none. Every `.icns` size is a true rendering, and the layout
    numbers were taken at 1x and 1.25x on Linux. Ask the machine rather than
    trusting a note: `system_profiler SPDisplaysDataType | grep 'UI Looks
    like'`.
32. **On Apple silicon**, the same list against the TestFlight build, and
    `check-install.sh` reporting the running process native rather than
    under Rosetta. CI opens a document and sees a window on arm64 on every
    push; it does not see a sandbox, an icon, or a person.

## What earlier runs cost

**Walk through the first usable slice, not the fourth.** Four stages were
built before the first keyboard walkthrough, and the walkthrough found the
product frame wrong rather than a defect. `DESIGN.md` §3 records it.

**One step at a time, one report per step.** David runs the list and reports
each item as pass or as what he saw; a screenshot or a screencast for
anything visual. A step that needs a restart in the middle is noted, not
retried from the top.
