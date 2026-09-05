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
3. **Click a paragraph, retype a line, click elsewhere.** The text changes in
   the document, the element pane shows the new text, Undo restores it. Escape
   in the middle of an edit discards it.
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
   commits; Undo restores it. Set a heading's level to 0: it stays at 1 and
   nothing reaches the document.
6. **Click a finding in the problems pane.** The element is selected and the
   tree scrolls to it without the last row flashing. Run this at the desktop's
   fractional scale as well as at 1x; the flash only ever showed at 1.25.
7. **Toggle the page image panel, drag its divider, toggle it off and on.**
   The width holds. Fit and 100% behave. The button reads *Page image*.
8. **Ctrl+1 and Ctrl+2 fold the structure and element panes**, and the
   document pane takes the space.
9. **A picture's inner rows are folded in the tree** until the picture or
   something inside it is selected.
10. **Delete asks first.** Delete on a selected element opens the confirm;
    Cancel leaves the document alone; confirming removes it and Undo brings it
    back.
11. **Save, then diff against the original.** The only difference is the edit.
    Framing whitespace, CDATA and every untouched element are byte for byte.
    For an archive, `pages/` and `assets/` are unchanged.
12. **Close with unsaved edits.** The dialog offers Save and close, Discard,
    Cancel, and each does what it says.

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

## What earlier runs cost

**Walk through the first usable slice, not the fourth.** Four stages were
built before the first keyboard walkthrough, and the walkthrough found the
product frame wrong rather than a defect. `DESIGN.md` §3 records it.

**One step at a time, one report per step.** David runs the list and reports
each item as pass or as what he saw; a screenshot or a screencast for
anything visual. A step that needs a restart in the middle is noted, not
retried from the top.
