#!/bin/sh
# Photograph the application's own window at a size App Store Connect accepts.
#
# The counterpart of `packaging/windows/screenshot.ps1`, and written for the
# same reason: `RELEASE.md` filed screenshots under *by hand, because no script
# can*, which was an assumption. What a script cannot do is decide which
# document to open or whether the result is a good advertisement. What it can
# do is every mechanical part — size the window, front it, drive it into the
# state the shot is of, move the pointer out of the frame, capture, and refuse
# if what came back is the wrong size.
#
#   ./packaging/macos/screenshot.sh --app dist-dev/Segler.app \
#       --document dist/archive-demo.dclx --out shots/01-window.png
#
# FOUR ACTIONS, IN THE ORDER GIVEN
#
# `--click X,Y` presses a control. `--double X,Y` presses it twice inside the
# double-click interval, which is how a block in the document pane opens for
# typing. `--type TEXT` types. `--key NAME` sends one key, optionally with
# modifiers: `--key cmd+a`, `--key return`. X and Y are measured from the
# frame's top-left corner on a shot of the same size, so a coordinate read off
# an earlier shot is the coordinate to give.
#
# They exist because a listing wants more than a document at rest. Apple
# rejected this application's first Mac screenshots under guideline 2.3.3 for
# exactly that: four frames of a document sitting still, nothing selected, and
# the element pane reading *Select an element* in every one. A shot has to
# show the application being used, and driving it is the only way to get one.
#
# THREE THINGS MEASURED RATHER THAN ASSUMED
#
# **It captures the window by its id, not by its rectangle.** `screencapture -R`
# photographs whatever is on screen in that region, so anything overlapping the
# window lands in the picture — which happened here on the first attempt and
# came back as a screenful of terminal. `-l` takes the window's own buffer and
# is indifferent to what is in front of it.
#
# **The pointer is moved off the window first.** Windows found this the
# expensive way: a shot came back 2292 pixels different from its predecessor and
# none of them were the change being photographed, because the pointer was
# resting on a field and egui drew it hovered and focus-ringed with the scroll
# bar showing. Neither is wrong, and both read as an interface caught mid-use.
#
# **It photographs a bundle, never the bare executable.** A bare Unix executable
# has no bundle identifier and no icon, so it is not the thing anybody installs.
# On Windows the equivalent is photographing the packaged application. The
# closest this platform can get is a *signed bundle built from the same commit*:
# the Store package cannot be launched at all off the Store — the kernel
# refuses it — so no screenshot can ever be of the exact artefact
# that gets uploaded. Build the bundle from the commit being released and say so
# in `packaging/store-listing.md`.
#
# Needs Accessibility permission for whatever runs it, because sizing another
# application's window goes through System Events. System Settings → Privacy &
# Security → Accessibility.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

app=""
document=""
out=""
# 1440x900 is one of the four sizes App Store Connect accepts for macOS, and the
# largest reachable without a Retina display. The other two — 2560x1600 and
# 2880x1800 — need a backing scale of 2, which is why they are not the default.
width=1440
height=900
# Anywhere the window fits entirely on screen; the capture does not depend on
# this, but a window hanging off the edge is clipped by the window server.
x=100
y=80

usage() {
    sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

refuse() { echo "screenshot.sh: $1" >&2; exit 1; }

# The actions, one to a line, in the order they were given. A file rather than
# a variable because `--type` takes text with spaces in it, and a
# space-separated list would split a sentence into words.
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM
actions="$work/actions"
: > "$actions"

while [ $# -gt 0 ]; do
    case "$1" in
        --app) app="${2:?--app needs a bundle}"; shift 2 ;;
        --document) document="${2:?--document needs a file}"; shift 2 ;;
        --out) out="${2:?--out needs a path}"; shift 2 ;;
        --click) echo "click ${2:?--click needs X,Y}" >> "$actions"; shift 2 ;;
        --double) echo "double ${2:?--double needs X,Y}" >> "$actions"; shift 2 ;;
        --type) echo "type ${2?--type needs text}" >> "$actions"; shift 2 ;;
        --key) echo "key ${2:?--key needs a name}" >> "$actions"; shift 2 ;;
        --width) width="${2:?}"; shift 2 ;;
        --height) height="${2:?}"; shift 2 ;;
        --x) x="${2:?}"; shift 2 ;;
        --y) y="${2:?}"; shift 2 ;;
        -h|--help) usage 0 ;;
        *) echo "screenshot.sh: unknown argument $1" >&2; usage 2 ;;
    esac
done

[ -n "$app" ] || refuse "no --app given"
[ -n "$document" ] || refuse "no --document given"
[ -n "$out" ] || refuse "no --out given"
[ -d "$app" ] || refuse "no bundle at $app"
[ -f "$document" ] || refuse "no document at $document"

case "$app" in
    *.app) ;;
    *) refuse "--app wants a .app bundle; a bare executable has no icon and is not what anybody installs" ;;
esac

# `open -a` reads a relative path as an application *name* to look up, and
# answers "Unable to find application named 'dist-dev/Segler.app'" — which
# reads like the bundle is missing when it is sitting right there.
app=$(cd "$(dirname "$app")" && pwd)/$(basename "$app")
document=$(cd "$(dirname "$document")" && pwd)/$(basename "$document")

mkdir -p "$(dirname "$out")"

# The helper does the things no shell command on this platform will: it reads
# the window server for an ordinary window's id, it puts the pointer somewhere
# harmless, and it posts the pointer and keyboard events that drive the window.
# Compiled once rather than interpreted at each call, because a shot that drives
# the window calls it six or seven times and `swift` pays its compile every
# time; the temporary directory goes with the trap either way.
source="$work/helper.swift"
helper="$work/helper"
cat > "$source" <<'SWIFT'
import CoreGraphics
import Foundation

// Park the pointer in the far corner. The corner rather than a constant: a
// fixed coordinate is off-screen on a smaller display, and the window server
// clamps to an edge, which could be the edge the window is on.
//
// Press a control: move there, then a press and a release a moment apart, which
// is what egui reads as a click. A System Events `click at` at the same point
// toggled nothing here, measured twice; this did, and why was not chased.
//
// A double press is the same two events twice with `mouseEventClickState`
// counting them, which is the field egui reads to tell a second click from a
// first. Without it, two presses a moment apart are two selections and never an
// opened editor, and opening one is the whole of what a double click does here.
//
// Typing goes in as a Unicode string on a keyboard event rather than as a key
// code, so a line with punctuation in it needs no layout table. Forty
// milliseconds a character, because the window redraws between them and faster
// than that drops characters the way `xdotool` does on the Linux lane.

let args = CommandLine.arguments

func post(_ type: CGEventType, at p: CGPoint, clicks: Int64) {
    let event = CGEvent(
        mouseEventSource: nil, mouseType: type, mouseCursorPosition: p, mouseButton: .left)!
    event.setIntegerValueField(.mouseEventClickState, value: clicks)
    event.post(tap: .cghidEventTap)
}

func press(at p: CGPoint, times: Int64) {
    post(.mouseMoved, at: p, clicks: 1)
    usleep(150_000)
    for n in 1...times {
        post(.leftMouseDown, at: p, clicks: n)
        usleep(40_000)
        post(.leftMouseUp, at: p, clicks: n)
        // Under the double-click interval, which is what makes a second press a
        // second click rather than another first one.
        usleep(60_000)
    }
}

if args.contains("--click") || args.contains("--double") {
    let p = CGPoint(x: Double(args[2])!, y: Double(args[3])!)
    press(at: p, times: args.contains("--double") ? 2 : 1)
    usleep(100_000)
    exit(0)
}

if args.contains("--type") {
    for character in Array(args[2]) {
        for down in [true, false] {
            let event = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: down)!
            var utf16 = Array(String(character).utf16)
            event.keyboardSetUnicodeString(stringLength: utf16.count, unicodeString: &utf16)
            // Emptied rather than left alone. An event made from a nil source
            // takes the session's current modifier state, and a `--key cmd+a`
            // just before this leaves command in it, so every character after
            // one arrives as a command shortcut and types nothing at all.
            event.flags = []
            event.post(tap: .cghidEventTap)
            usleep(20_000)
        }
        usleep(40_000)
    }
    exit(0)
}

// Only the keys a screenshot run has wanted; add to the table rather than
// reaching for a layout API.
if args.contains("--key") {
    let codes: [String: CGKeyCode] = [
        "a": 0, "s": 1, "z": 6, "return": 36, "escape": 53, "tab": 48,
        "delete": 51, "left": 123, "right": 124, "down": 125, "up": 126,
        "home": 115, "end": 119,
    ]
    var flags: CGEventFlags = []
    var name = ""
    for part in args[2].lowercased().split(separator: "+") {
        switch part {
        case "cmd", "command": flags.insert(.maskCommand)
        case "shift": flags.insert(.maskShift)
        case "alt", "option": flags.insert(.maskAlternate)
        case "ctrl", "control": flags.insert(.maskControl)
        default: name = String(part)
        }
    }
    guard let code = codes[name] else {
        FileHandle.standardError.write("no key named \(name)\n".data(using: .utf8)!)
        exit(2)
    }
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down)!
        event.flags = flags
        event.post(tap: .cghidEventTap)
        usleep(40_000)
    }
    // The modifier is let go here as its own event, so that the session state
    // the next event inherits has nothing held down in it.
    if !flags.isEmpty {
        let cleared = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: false)!
        cleared.type = .flagsChanged
        cleared.flags = []
        cleared.post(tap: .cghidEventTap)
        usleep(40_000)
    }
    exit(0)
}

if args.contains("--park") {
    let screen = CGDisplayBounds(CGMainDisplayID())
    CGWarpMouseCursorPosition(CGPoint(x: screen.maxX - 1, y: screen.maxY - 1))
    exit(0)
}

let wanted = args.count > 1 ? args[1] : "Segler"
guard
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID)
        as? [[String: Any]]
else {
    FileHandle.standardError.write("the window server returned nothing\n".data(using: .utf8)!)
    exit(2)
}
for w in windows {
    guard w[kCGWindowOwnerName as String] as? String == wanted,
          (w[kCGWindowLayer as String] as? Int ?? -1) == 0,
          let number = w[kCGWindowNumber as String] as? Int
    else { continue }
    print(number)
    exit(0)
}
FileHandle.standardError.write("no ordinary window belonging to \(wanted)\n".data(using: .utf8)!)
exit(1)
SWIFT

swiftc -O -o "$helper" "$source" || refuse "the helper did not compile"

# Anything already running is stopped, so the window photographed is the one
# holding the document this run was given rather than one left over.
pkill -f "$(basename "$app")/Contents/MacOS/" 2>/dev/null || true
sleep 1

open -a "$app" "$document"
sleep 5

osascript >/dev/null <<OSA || refuse "could not size the window — is Accessibility granted?"
tell application "System Events"
    set p to first process whose name contains "segler"
    set frontmost of p to true
    tell p
        set position of window 1 to {$x, $y}
        set size of window 1 to {$width, $height}
    end tell
end tell
OSA
sleep 1

# Pointer coordinates are given in the frame and posted on the screen, so the
# window's own origin is added here and nowhere else.
#
# The list is read on a descriptor of its own and the helper is given no input
# at all. Both matter: with the loop reading the file as stdin, the helper
# inherits it, reads what is left of it, and the actions it swallowed never run
# — which looked exactly like typing that did not reach the window, and cost an
# afternoon.
while IFS= read -r action <&3; do
    verb=${action%% *}
    rest=${action#* }
    case "$verb" in
        click|double)
            "$helper" "--$verb" \
                "$(( x + ${rest%,*} ))" "$(( y + ${rest#*,} ))" </dev/null
            ;;
        type) "$helper" --type "$rest" </dev/null ;;
        key) "$helper" --key "$rest" </dev/null ;;
        *) refuse "unknown action $verb" ;;
    esac
    sleep 1
done 3< "$actions"

"$helper" --park
sleep 1

id=$("$helper" Segler) || refuse "could not find the window"
screencapture -x -o -l "$id" "$out"

got_w=$(sips -g pixelWidth "$out" | sed -n 's/.*pixelWidth: *//p')
got_h=$(sips -g pixelHeight "$out" | sed -n 's/.*pixelHeight: *//p')
if [ "$got_w" != "$width" ] || [ "$got_h" != "$height" ]; then
    refuse "asked for ${width}x${height} and got ${got_w}x${got_h} — App Store Connect refuses anything but its own sizes"
fi

echo "${out}: ${got_w}x${got_h}, window ${id}"
