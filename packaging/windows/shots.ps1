# The Microsoft Store screenshots, as recipes rather than as prose.
#
# `screenshot.ps1` beside this is the driver and knows nothing about Segler: it
# sizes the window, fronts it, drives the actions given, parks the pointer off
# the frame, captures, and refuses a result of the wrong size. This file is the
# other half, the part that is Segler's - which document, which shot, and what
# has to happen in the window before the shutter. It is the Windows spelling of
# `packaging/macos/shots.sh`, which is the same split.
#
# It is one file on purpose. Everything a person would change to retake a set
# lives at the top, and running it by hand on the Windows machine is the same
# command CI runs:
#
#     powershell -ExecutionPolicy Bypass -File packaging\windows\shots.ps1
#
# WHAT THE SET HAS TO SHOW, AND WHY
#
# Apple rejected the Mac four under guideline 2.3.3 - *the Mac screenshots do
# not show the actual app in use in the majority of the screenshots* - and the
# rejection was right. Nothing was selected in any of them, so the element pane
# read *Select an element on the page or in the structure* in all four, and no
# edit was under way. A frame of an editor with nothing selected is a frame of
# a viewer. The Windows four are that same set in its Windows spelling and the
# same criticism is true of them, so: something is selected in every shot below,
# and an edit is under way or just done in more than one.
#
# The coordinates are measured from the frame's top-left corner at the size
# declared here. Change the size and they all move; that is why the size is a
# constant beside them rather than an argument with a default.
#
# WHAT THE ASSOCIATION MEANS FOR WHAT IS PHOTOGRAPHED
#
# The driver opens the document through the shell, so what appears is whatever
# is registered for `.dclx` on this machine. With the MSIX installed that is the
# packaged build, which is what a person gets. With `install.ps1` run against a
# release binary it is that binary, which is the build CI has. Both are this
# commit's application; say which one in `packaging/store-listing.md`.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)

[CmdletBinding()]
param(
    # One plain capture of the document at rest, and nothing else. This is how
    # the coordinates below get measured: take it, open it, read the pixel of
    # each control off it, and fill the constants in.
    [switch] $Reference,
    [string] $OutDir
)

$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

# --- the configuration ------------------------------------------------------

# 1366x768 is the Microsoft Store's minimum for a desktop screenshot, and a
# window this size looks like a window rather than like an advertisement.
$WIDTH = 1366
$HEIGHT = 768

# The document the review notes point at, which rides every release.
$DOCUMENT = Join-Path $root 'packaging\review\sailing-directions.dclx'

# Where the shots land. Not committed: dist is where every built artefact goes.
if (-not $OutDir) { $OutDir = Join-Path $root 'dist\screenshots' }

# The controls, by what they do rather than by where they are, so a recipe
# below reads as the thing it is doing. Each is "X,Y" in the frame.
#
# The three with numbers were read off the 2026-09-06 set at this size. The
# three that are empty have never been measured on this platform - the Mac has
# them at 1440x900 against a different toolbar and a different pane width, and
# a coordinate carried across from there lands somewhere else. Take a reference
# frame, read them off it, and fill them in:
#
#     powershell -ExecutionPolicy Bypass -File packaging\windows\shots.ps1 -Reference
#
$PAGE_IMAGE_PANEL = '423,40'   # the toolbar toggle for the page images
$NEXT_PAGE = '353,40'          # the toolbar's forward arrow
$THE_PICTURE = '620,440'       # the picture on page two, on the page itself

$A_TABLE_CELL = ''             # a body cell of the four-column table, page one
$A_PARAGRAPH = ''              # a line of running text, page one
$A_HEADING = ''                # a heading, page one

# What a corrected cell is corrected to. Short, and visibly a correction of
# what the scan shows rather than a different number.
$CORRECTION = '-0:38'

# One shot to a line: name, then the actions that put the window into the state
# being photographed, in the order they are given.
#
#   click X,Y     press a control
#   double X,Y    press it twice inside the double-click interval, which is
#                 how a block in the document pane opens for typing
#   type TEXT     type
#   key NAME      one key, optionally with modifiers: ctrl+a, return
function Get-Shots {
    # Correcting a table cell in place: the cell open with the new value being
    # typed, the table selected, and the pane showing row, column and kind.
    Shot '01-correcting-a-cell' @(
        "double $A_TABLE_CELL", 'key ctrl+a', "type $CORRECTION")

    # The page image beside the document, a paragraph selected, and its located
    # box drawn over the scan with the same coordinates in the pane. The page
    # image panel is in the set because it is the one thing in this application
    # no other DocLang tool has.
    Shot '02-the-page-beside-the-document' @(
        "click $PAGE_IMAGE_PANEL", "click $A_PARAGRAPH")

    # The correction committed: the title carries the modified mark, Save and
    # Undo have come on, the status line says which cell changed, and a heading
    # is selected with its level, layer and box in reach.
    Shot '03-the-correction-committed' @(
        "double $A_TABLE_CELL", 'key ctrl+a', "type $CORRECTION", "click $A_HEADING")

    # Page two: the picture selected, its class, its caption, its box on the
    # scan, and its markup.
    Shot '04-a-picture-on-page-two' @(
        "click $NEXT_PAGE", "click $THE_PICTURE")
}

# --- the driving ------------------------------------------------------------

function Refuse([string] $message) { Write-Error "shots.ps1: $message" }

# Get-Shots describes the set rather than taking it, so the whole set can be
# read before any of it runs.
function Shot([string] $name, [string[]] $actions) {
    [pscustomobject]@{ Name = $name; Actions = $actions }
}

$shots = @(Get-Shots)
$driver = Join-Path $PSScriptRoot 'screenshot.ps1'

if (-not (Test-Path $DOCUMENT)) { Refuse "no document at $DOCUMENT" }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

# The reference frame is what the unmeasured constants are measured off, so it
# has to be reachable while they are still empty - before the check below and
# not after it.
if ($Reference) {
    $out = Join-Path $OutDir 'reference.png'
    Write-Host 'shots.ps1: reference'
    & $driver -Document $DOCUMENT -Out $out -Width $WIDTH -Height $HEIGHT
    Write-Host "shots.ps1: read the coordinates off $out and fill them in at the top of this file"
    return
}

# A recipe naming a control that has never been measured would send its click
# to the frame's corner, and a click that lands on nothing is a frame that
# shows none of what it was for - which is the failure this set exists to
# correct. So an unmeasured control stops all four rather than spoiling one.
$unmeasured = @($shots |
    Where-Object { $_.Actions -match '^(click|double) *$' } |
    ForEach-Object { $_.Name })
if ($unmeasured.Count -gt 0) {
    Refuse ("these shots name a control that has never been measured at " +
        "${WIDTH}x${HEIGHT}: " + ($unmeasured -join ', ') + ". Take a reference " +
        "frame with -Reference, read the coordinates off it, and fill in the " +
        "empty constants at the top of this file.")
}

# The driver refuses by writing a terminating error, which comes back through
# the call rather than as an exit code: `&` on a script runs it in this process,
# and $LASTEXITCODE is the last *native* command's.
$taken = 0
foreach ($shot in $shots) {
    $out = Join-Path $OutDir "$($shot.Name).png"
    Write-Host "shots.ps1: $($shot.Name)"
    & $driver -Document $DOCUMENT -Out $out -Width $WIDTH -Height $HEIGHT -Do $shot.Actions
    $taken++
}

# A set that came back short is a set somebody submits without noticing, so the
# count is said rather than left to be counted.
Write-Host "shots.ps1: $taken shot(s) in $OutDir"
