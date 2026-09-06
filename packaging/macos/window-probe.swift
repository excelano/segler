// Ask the window server whether an application has drawn a window.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)
//
// This exists for CI. `.github/workflows/apple-silicon.yml` runs the suite and
// the corpus natively on arm64, and the one thing no runner reaches otherwise
// is the window, which is exactly the code the arm64 walkthrough exists for:
// `src/opened_document.rs` is the only `unsafe` in the desktop crate, and
// macOS delivers a double-clicked document through an Apple Event rather than
// argv.
//
// A screenshot is the wrong assertion. slipcase-desktop records `screencapture`
// returning the desktop with every window omitted while reporting no error, so
// a job asserting on pixels would go green against a build that drew nothing.
// This asks the window server directly.
//
// `CGWindowListCopyWindowInfo` gives the owner name and the bounds without
// Screen Recording permission. The window *title* needs it, so the title is
// read when it is there and never required: a runner that withholds it still
// gets a verdict on the window, and the title, which is the document's name,
// is printed as a further fact when the platform allows. This prints what it
// saw rather than only its verdict, because an empty list means the API is
// restricted and the answer is unknown, while a list holding other
// applications' windows and none of ours means the application drew nothing,
// and those are different findings a bare exit code cannot tell apart.

import CoreGraphics
import Foundation

let wanted = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "Segler"

guard
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID)
        as? [[String: Any]]
else {
    print("the window server returned nothing at all — the list is unavailable")
    exit(2)
}

var owners: Set<String> = []
var ours: [(Double, Double, Int, String?)] = []

for w in windows {
    guard let owner = w[kCGWindowOwnerName as String] as? String else { continue }
    owners.insert(owner)
    guard owner == wanted else { continue }
    let layer = w[kCGWindowLayer as String] as? Int ?? -1
    guard
        let b = w[kCGWindowBounds as String] as? [String: Any],
        let width = b["Width"] as? Double,
        let height = b["Height"] as? Double
    else { continue }
    ours.append((width, height, layer, w[kCGWindowName as String] as? String))
}

print("on-screen windows: \(windows.count), from \(owners.count) applications")
print("owners: \(owners.sorted().joined(separator: ", "))")

if windows.isEmpty {
    print("VERDICT: unknown — the window server listed nothing, so this says")
    print("         nothing about \(wanted). Not a pass and not a failure.")
    exit(2)
}

// Layer 0 is an ordinary application window. A menu, a panel or a shadow sits
// elsewhere, and counting one of those as the application's window would make
// this pass against a build that draws no interface at all.
let real = ours.filter { $0.0 > 1 && $0.1 > 1 && $0.2 == 0 }
for (w, h, layer, name) in ours {
    let title = name.map { " titled \"\($0)\"" } ?? " (title withheld)"
    print(String(format: "  %@ window: %.0f x %.0f at layer %d", wanted, w, h, layer) + title)
}

if real.isEmpty {
    print("VERDICT: FAILED — \(windows.count) windows are listed and none of them is")
    print("         an ordinary window belonging to \(wanted).")
    exit(1)
}

print("VERDICT: passed — \(wanted) has \(real.count) ordinary window(s) on screen")
exit(0)
