# Packaging

`DESIGN.md` §8. One directory per platform, plus `debian` for the way Linux is
distributed, and three files shared by all of them: `version.sh`, which is the
only thing that reads the version out of `Cargo.toml`; `preflight.sh`, which
asks everything that must be true before a release at once; and
`store-listing.md`, the text both stores are given.

The shape is `excelano/slipcase-desktop`'s, and where a file here says
something was measured, it was measured there first unless the file says
otherwise. That repository's per-platform READMEs are the detail for Windows
and macOS until this one has its own.

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

`linux/icons/segler-desktop.svg` is the application: one sail on a hull over
water, on a 64-unit grid with 3-unit strokes. The name is German for sailor.
It is deliberately not DocLang's logo, which is a stylised document and that
project's own. The two document icons beside it are a page with the sail on
it, blue for an archive and cream for bare markup, so the two kinds tell apart
in a listing while both saying Segler. The palette is slipcase-desktop's, so
the two Excelano applications look like siblings on a launcher.

All three were checked at 16, 24, 32, 48 and 128 pixels before committing,
and any change should be. The SVG is the source for every platform: macOS
wants `.icns` and Windows wants `.ico`, both converted from it, and
slipcase-desktop's `make-ico` is the converter for the second.

## windows, macos

Not here yet. Each is cloned from slipcase-desktop's directory of the same
name by the lane that can build and test it, and `RELEASE.md` says what each
lane needs to know before starting.
