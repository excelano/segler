#!/bin/sh
# Photograph the application's own window at a size App Store Connect accepts.
#
# The counterpart of `packaging/windows/screenshot.ps1`, and written for the
# same reason: `RELEASE.md` filed screenshots under *by hand, because no script
# can*, which was an assumption. What a script cannot do is decide which
# document to open or whether the result is a good advertisement. What it can
# do is every mechanical part — size the window, front it, move the pointer out
# of the frame, capture, and refuse if what came back is the wrong size.
#
#   ./packaging/macos/screenshot.sh --app dist-devid/Segler.app \
#       --document dist/archive-demo.dclx --out shots/01-window.png
#
# `--click X,Y`, repeatable, presses a control before the capture, X and Y
# measured from the frame's top-left corner on a shot of the same size; it is
# how a shot shows anything other than a document at rest. The press goes
# through the window server as a move, a press and a release, because a
# System Events `click at` toggled nothing here.
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
# Points to click, in order, after the window is sized and before the pointer
# is parked: what puts the window in the state the shot is of, since a listing
# wants more than one document at rest. Each is X,Y from the top-left corner
# of the frame, title bar included, so a coordinate read off an earlier shot
# of the same size is the coordinate to give.
clicks=""
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
    sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

while [ $# -gt 0 ]; do
    case "$1" in
        --app) app="${2:?--app needs a bundle}"; shift 2 ;;
        --document) document="${2:?--document needs a file}"; shift 2 ;;
        --out) out="${2:?--out needs a path}"; shift 2 ;;
        --click) clicks="$clicks ${2:?--click needs X,Y}"; shift 2 ;;
        --width) width="${2:?}"; shift 2 ;;
        --height) height="${2:?}"; shift 2 ;;
        --x) x="${2:?}"; shift 2 ;;
        --y) y="${2:?}"; shift 2 ;;
        -h|--help) usage 0 ;;
        *) echo "screenshot.sh: unknown argument $1" >&2; usage 2 ;;
    esac
done

refuse() { echo "screenshot.sh: $1" >&2; exit 1; }

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
# answers "Unable to find application named 'dist-devid/Segler.app'" — which
# reads like the bundle is missing when it is sitting right there.
app=$(cd "$(dirname "$app")" && pwd)/$(basename "$app")
document=$(cd "$(dirname "$document")" && pwd)/$(basename "$document")

mkdir -p "$(dirname "$out")"

# The helper does the two things no shell command on this platform will: it
# reads the window server for an ordinary window's id, and it puts the pointer
# somewhere harmless. Run rather than compiled — it is a second either way and
# a build product here would want cleaning up.
helper=$(mktemp -d)/helper.swift
trap 'rm -rf "$(dirname "$helper")"' EXIT INT TERM
cat > "$helper" <<'SWIFT'
import CoreGraphics
import Foundation

// Park the pointer in the far corner. The corner rather than a constant: a
// fixed coordinate is off-screen on a smaller display, and the window server
// clamps to an edge, which could be the edge the window is on.
// Press a control: move there, then a press and a release a moment apart, which is
// what egui reads as a click. A System Events `click at` at the same point
// toggled nothing here, measured twice; this did, and why was not chased.
if CommandLine.arguments.contains("--click") {
    let p = CGPoint(x: Double(CommandLine.arguments[2])!, y: Double(CommandLine.arguments[3])!)
    for (kind, pause) in [(CGEventType.mouseMoved, 150_000), (.leftMouseDown, 80_000), (.leftMouseUp, 100_000)] {
        CGEvent(mouseEventSource: nil, mouseType: kind, mouseCursorPosition: p, mouseButton: .left)!.post(tap: .cghidEventTap)
        usleep(UInt32(pause))
    }
    exit(0)
}

if CommandLine.arguments.contains("--park") {
    let screen = CGDisplayBounds(CGMainDisplayID())
    CGWarpMouseCursorPosition(CGPoint(x: screen.maxX - 1, y: screen.maxY - 1))
    exit(0)
}

let wanted = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "Segler"
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

for click in $clicks; do
    swift "$helper" --click "$(( x + ${click%,*} ))" "$(( y + ${click#*,} ))"
    sleep 2
done

swift "$helper" --park
sleep 1

id=$(swift "$helper" Segler) || refuse "could not find the window"
screencapture -x -o -l "$id" "$out"

got_w=$(sips -g pixelWidth "$out" | sed -n 's/.*pixelWidth: *//p')
got_h=$(sips -g pixelHeight "$out" | sed -n 's/.*pixelHeight: *//p')
if [ "$got_w" != "$width" ] || [ "$got_h" != "$height" ]; then
    refuse "asked for ${width}x${height} and got ${got_w}x${got_h} — App Store Connect refuses anything but its own sizes"
fi

echo "${out}: ${got_w}x${got_h}, window ${id}"
