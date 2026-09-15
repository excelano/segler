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

# The document the review notes point at, which rides every release.
DOCUMENT="${root}/packaging/review/sailing-directions.dclx"

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
    # Correcting a table cell in place: the cell open with the new value being
    # typed, the table selected, and the pane showing row, column and kind.
    shot 01-correcting-a-cell \
        --double 1020,384 --key cmd+a --type "-0:38"

    # The page image beside the document, a paragraph selected, and its located
    # box drawn over the scan with the same coordinates in the pane. The page
    # image panel is in the set because it is the one thing in this application
    # no other DocLang tool has.
    shot 02-the-page-beside-the-document \
        --click 422,39 --click 520,300

    # The correction committed: the title carries the modified mark, Save and
    # Undo have come on, the status line says which cell changed, and a heading
    # is selected with its level, layer and box in reach.
    shot 03-the-correction-committed \
        --double 1020,384 --key cmd+a --type "-0:38" --click 352,500

    # Page two: the picture selected, its class, its caption, its box on the
    # scan, and its markup.
    shot 04-a-picture-on-page-two \
        --click 422,39 --click 352,39 --click 70,153
}

# --- the driving ------------------------------------------------------------

app=""
while [ $# -gt 0 ]; do
    case "$1" in
        --app) app="${2:?--app needs a bundle}"; shift 2 ;;
        --outdir) OUTDIR="${2:?--outdir needs a directory}"; shift 2 ;;
        -h|--help) sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "shots.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done

[ -n "$app" ] || {
    echo "shots.sh: --app needs the bundle to photograph" >&2
    echo "  ./packaging/macos/build-app.sh --sign 'Apple Development: ...'" >&2
    echo "  ./packaging/macos/shots.sh --app dist-dev/Segler.app" >&2
    exit 2
}
[ -d "$app" ] || { echo "shots.sh: no bundle at $app" >&2; exit 1; }
[ -f "$DOCUMENT" ] || { echo "shots.sh: no document at $DOCUMENT" >&2; exit 1; }

mkdir -p "$OUTDIR"

taken=0
shot() {
    name=$1
    shift
    out="${OUTDIR}/${name}.png"
    echo "shots.sh: ${name}"
    "${here}/screenshot.sh" \
        --app "$app" \
        --document "$DOCUMENT" \
        --width "$WIDTH" --height "$HEIGHT" \
        --out "$out" \
        "$@"
    taken=$((taken + 1))
}

shots

# A set that came back short is a set somebody submits without noticing, so the
# count is said rather than left to be counted.
echo "shots.sh: ${taken} shot(s) in ${OUTDIR}"
