#!/bin/sh
# The Mac App Store screenshots, as recipes rather than as prose.
#
# `screenshot.sh` beside this is the driver and knows nothing about Segler: it
# sizes the window, fronts it, drives the actions given, parks the pointer off
# the frame, captures, and refuses a result of the wrong size. This file is the
# other half, the part that is Segler's — which document, which shot, and what
# has to happen in the window before the shutter.
#
# It is one file on purpose. Everything a person would change to retake a set
# lives at the top, and running it by hand on the Mac is the same command CI
# runs:
#
#     ./packaging/macos/shots.sh --app dist-dev/Segler.app
#
# WHAT THE SET HAS TO SHOW, AND WHY
#
# Apple rejected the first four under guideline 2.3.3 — *the Mac screenshots do
# not show the actual app in use in the majority of the screenshots* — and the
# rejection was right. Nothing was selected in any of them, so the element pane
# read *Select an element on the page or in the structure* in all four, and no
# edit was under way. A frame of an editor with nothing selected is a frame of
# a viewer. So: something is selected in every shot below, and an edit is under
# way or just done in more than one.
#
# The coordinates are measured from the frame's top-left corner at the size
# declared here. Change the size and they all move; that is why the size is a
# constant beside them rather than an argument with a default.
#
# LIGHT AND DARK
#
# All four earn a second slot. None of them is the frame the 2.3.3 rejection
# was about — every one already shows a selection or an edit in progress — so
# repeating the same four actions in dark carries the "app in use" property
# into the dark set rather than risking it. Four frames plus their dark
# repeats is eight, still under Apple's ten and the Microsoft Store's own ten.
# Light leads, numbered 01-04; the dark repeats are 05-08, so the file names
# alone keep light first without depending on where `shots()` calls
# `appearance`.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)

set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "${here}/../.." && pwd)

# --- the configuration ------------------------------------------------------

# App Store Connect takes 1440x900 for macOS. Every coordinate below was read
# off a shot of this size.
WIDTH=1440
HEIGHT=900

# The document the review notes point at, which rides every release. One per
# language: the document is most of what these frames show, so a German
# listing opening the English archive would be mostly English pixels whatever
# language the window chrome is in.
ENGLISH_DOCUMENT="${root}/packaging/review/sailing-directions.dclx"
GERMAN_DOCUMENT="${root}/packaging/review/sailing-directions.de.dclx"

# Where the shots land. Not committed: dist is where every built artefact goes.
OUTDIR="${root}/dist/screenshots"

# One shot to a line: name, then the actions that put the window into the state
# being photographed, in the order they are given.
#
#   --click X,Y    press a control
#   --double X,Y   press it twice inside the double-click interval, which is
#                  how a block in the document pane opens for typing
#   --type TEXT    type
#   --key NAME     one key, optionally with modifiers: cmd+a, return
shots() {
    appearance light

    # Correcting a table cell in place: the cell open with the new value being
    # typed, the table selected, and the pane showing row, column and kind.
    shot 01-correcting-a-cell \
        --double 1020,384 --key cmd+a --type "-0:38"

    # The page image beside the document, a paragraph selected, and its located
    # box drawn over the scan with the same coordinates in the pane. The page
    # image panel is in the set because it is the one thing in this application
    # no other DocLang tool has.
    shot 02-the-page-beside-the-document \
        --click "$PAGE_IMAGE_PANEL" --click 520,300

    # The correction committed: the title carries the modified mark, Save and
    # Undo have come on, the status line says which cell changed, and a heading
    # is selected with its level, layer and box in reach.
    shot 03-the-correction-committed \
        --double 1020,384 --key cmd+a --type "-0:38" --click 352,500

    # Page two: the picture selected, its class, its caption, its box on the
    # scan, and its markup.
    shot 04-a-picture-on-page-two \
        --click "$PAGE_IMAGE_PANEL" --click "$NEXT_PAGE" --click 70,153

    appearance dark

    shot 05-correcting-a-cell-dark \
        --double 1020,384 --key cmd+a --type "-0:38"

    shot 06-the-page-beside-the-document-dark \
        --click "$PAGE_IMAGE_PANEL" --click 520,300

    shot 07-the-correction-committed-dark \
        --double 1020,384 --key cmd+a --type "-0:38" --click 352,500

    shot 08-a-picture-on-page-two-dark \
        --click "$PAGE_IMAGE_PANEL" --click "$NEXT_PAGE" --click 70,153
}

# --- the driving ------------------------------------------------------------

# Which document this language opens, and where the toolbar puts the two
# controls a shot clicks. `take-shots.sh` beside this does the rest and is
# generated by ship.
#
# $PAGE_IMAGE_PANEL and $NEXT_PAGE sit in the toolbar row, to the right of
# Open…/Save/Undo/Redo or their German counterparts, all four of them
# translated text and not icons. A German label is not the same width as its
# English one, which moves every control to its right - measured on Windows at
# about 130 pixels for this same toolbar, and read off a German reference
# frame here rather than assumed, since the two platforms do not share a
# window width. The content-pane coordinates elsewhere in `shots()` are not
# toolbar-relative and hold across both languages.
for_language() {
    case "$1" in
        en|en-US|en-us)
            document=$ENGLISH_DOCUMENT
            PAGE_IMAGE_PANEL='422,39'
            NEXT_PAGE='352,39'
            ;;
        de|de-DE|de-de)
            document=$GERMAN_DOCUMENT
            PAGE_IMAGE_PANEL='552,39'
            NEXT_PAGE='482,39'
            ;;
        *) echo "shots.sh: no set is written for $1" >&2; exit 2 ;;
    esac
}

. "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/take-shots.sh"
take_shots "$@"
