# Packaging

`DESIGN.md` §8. One directory per platform, plus `debian` for the way Linux is
distributed, and three files shared by all of them: `version.sh`, which is the
only thing that reads the version out of `Cargo.toml`; `preflight.sh`, which
asks everything that must be true before a release at once; and
`store-listing.md`, the text both stores are given.

The shape is `excelano/slipcase-desktop`'s, and where a file here says
something was measured, it was measured there first unless the file says
otherwise. Each platform directory here has its own README; that
repository's are the long form where a paragraph here says it was measured
there.

## linux

The freedesktop half: the two DocLang media types, the desktop entry, the
application icon and the two document icons. Install it into a prefix, which
defaults to `~/.local`:

    ./packaging/linux/install.sh
    ./packaging/linux/install.sh --prefix /usr/local     # for everyone
    ./packaging/linux/uninstall.sh

The script installs both executables too, found by asking `cargo metadata`
where the target directory is, because `[build] target-dir` in a Cargo
configuration file moves it and no environment variable then says so.

Check that it took, through `gio` rather than `xdg-mime`. With no desktop
session `xdg-mime` falls back to `file`, which reads magic and knows nothing of
the shared-mime-info database, and neither DocLang extension reserves magic:

    gio info -a standard::content-type some.dclx    # application/vnd.doclang.archive+zip
    gio mime application/vnd.doclang.archive+zip    # segler-desktop.desktop

**The media types are declared here, and one of them is Segler's own name.**
The specification names `application/vnd.doclang.document+xml` for the markup
and nothing for the archive; `mime/doclang.xml` says what was chosen for the
archive and why it is provisional. The declaration ships in the `segler`
package because nothing else on a Linux machine declares it. If a second
DocLang product for Linux ever exists, the declaration and the document icons
move to a package both depend on, the way `slipcase-common` holds Slipcase's:
two packages cannot ship one path, and dpkg refuses the second install
outright.

`check-libraries.sh` runs the window under Wayland and under X11, records
every shared object the process mapped, and refuses any whose package
`Depends` in `debian/control.in` does not transitively reach. Run it after
touching a dependency. It needs a display, so it is a command and never a
test.

## debian

The package the Excelano apt repository ships:

    cargo build --release --workspace
    ./packaging/debian/build-deb.sh

It writes `dist/segler_VERSION_ARCH.deb` and then prints what the executable
links beside what the package declares, because those two lists are almost
disjoint and that is the trap this package exists to avoid. The executable
links libc and libgcc and nothing else; the display stack, the graphics driver
loader and the keyboard map libraries are opened by name at run time, so
`Depends` is written by hand and `check-libraries.sh` is what keeps it true.

One package, `segler`, carrying both `segler-desktop` and the `segler`
command-line tool, with a manual page for each. One product, one thing to
type after `apt install`.

The package carries no maintainer scripts. `shared-mime-info`,
`desktop-file-utils` and `hicolor-icon-theme` own dpkg triggers on the three
directories this package writes into, so the caches are rebuilt by dpkg
without a `postinst` asking for it.

`copyright` is a DEP-5 file rather than a copy of `LICENSE`. Apache-2.0 is one
of Debian's common licences and policy wants it referred to at
`/usr/share/common-licenses` rather than copied; lintian makes the copy an
error. `.github/workflows/linux.yml` runs lintian on every push.

## The icons

`linux/icons/segler-desktop.svg` is the application: a sailboat on a blue
square, mainsail, jib and hull as filled shapes and no strokes at all, on a
64-unit grid. The tile is a plain square, full bleed and unframed, because
that is the shape a store takes and it applies its own corner rounding to
it; a drawing carrying a smaller shape or its own outline into that frame
reads as a sticker. The name is German for sailor. It is
deliberately not DocLang's logo, which is a stylised document and that
project's own. The two document icons beside it are a page carrying the same
boat, blue for an archive and cream for bare markup, so the two kinds tell
apart in a listing while both saying Segler. The palette is
slipcase-desktop's, so the two Excelano applications look like siblings on a
launcher.

David chose it from a sheet of five on 2026-09-04; the file's own comment
records what the other four cost. All three were checked at 16, 24, 32, 48 and
128 pixels on light and dark grounds before committing, and any change should
be. The SVGs are the source for every platform: macOS wants `.icns` and Windows
wants `.ico`, both converted from them, and `windows/make-ico` is the converter
for the second.

**All three drawings are converted, not just the application's.** Windows draws
a file type's icon from the package's assets when the package is installed and
from an icon directory when the scripts are, so `make-ico` writes `segler.ico`,
`dclx.ico` and `dclg.ico` and six PNG sets beside them. Without that a `.dclx`
and a `.dclg` would carry the same picture in Explorer while carrying different
ones in a Linux file manager, which is the sort of difference nobody notices
until they have both machines open.

### Both shapes, for a submission form

`icons/` holds the application icon in two shapes - `segler-square` and
`segler-rounded`, each as an SVG and as PNGs at 256, 512, 1024, 1080 and 2160.
`windows/make-ico` writes the directory and clips the rounded one from the same
source; neither shape is duplicated as a drawing and neither is edited by hand.

Which to upload is a decision taken at the form, which is why both exist and
neither is the default. A store that masks what it is given wants the square:
the iOS and iPadOS Store does, and so does Icon Composer. A form that draws what
it is handed wants the rounded one. The corner is 22.37% of the side, which is
Apple's proportion, drawn as a circular arc rather than the continuous curve
Apple's own tooling produces; below about 512 pixels the two do not tell apart,
and where they would, Icon Composer on a Mac is what draws Apple's shape.

Nothing in `icons/` ships. The deb installs named files out of `linux/icons`
and `build-msix.ps1` copies `windows/assets/*.png`, so neither reaches a
package, and no code reads one at run time.

## windows

The MSIX the Microsoft Store distributes, and a pair of PowerShell scripts that
register the two extensions per-user for somebody who would rather not have a
Store account. `windows/README.md` is the detail; the short version is that it
is slipcase-desktop's directory cloned, with two file types where that
application has one, three icon directories where it has one, and its own
certification baseline.

    cargo build --release --workspace
    powershell -ExecutionPolicy Bypass -File packaging\windows\build-msix.ps1 -SelfSign
    powershell -ExecutionPolicy Bypass -File packaging\windows\install.ps1     # the script route

`check-imports.ps1` is the Windows counterpart of `check-libraries.sh`: it walks
the shipped binary's PE import table and refuses any DLL not known to ship with
Windows. Unlike the Linux one it needs no display, so `windows.yml` runs it on
every push. That check exists because Slipcase 0.1.1 passed everything else and
still failed Store certification, on a clean machine that had no Visual C++
Redistributable.

## macos

The bundle the Mac App Store distributes, and the scripts around it.
`macos/README.md` is the detail; the short version is that it is
slipcase-desktop's directory cloned, with two type declarations where that
application has one, imported rather than exported because DocLang is not
this application's format, three `.icns` where it has one, and a sandbox
that cost the save path a macOS arm.

    cargo build --release
    ./packaging/macos/build-app.sh --sign "Apple Development: …"
    ./packaging/macos/build-app.sh --store PROFILE.provisionprofile

`build-app.sh` is the counterpart of `check-imports.ps1` and
`check-libraries.sh` as well as the packager: it refuses a binary importing
a symbol no public framework header declares, which is what App Store review
refuses as Guideline 2.5.1. `check-install.sh` asks an installed bundle what
it is on the machine it is on, and `window-probe.swift` is what
`.github/workflows/apple-silicon.yml` uses to ask whether the arm64 build
drew a window for a document opened through Launch Services.
