# Windows packaging

`DESIGN.md` §8. The two DocLang extensions and their media types registered on
the platform that has no freedesktop database to put them in, and the MSIX the
Microsoft Store distributes.

    powershell -ExecutionPolicy Bypass -File packaging\windows\install.ps1
    powershell -ExecutionPolicy Bypass -File packaging\windows\uninstall.ps1

`install.ps1 -NoBinary` registers the associations without copying an executable,
and `-Prefix DIR` puts the files somewhere else. `uninstall.ps1 -KeepFiles`
removes the associations and leaves them.

**This directory is `slipcase-desktop/packaging/windows` cloned, and where a
file here says something was measured, it was measured there unless it says
otherwise.** That repository has been through Microsoft Store certification, a
rejection, and a resubmission; its README is the long form of everything below
and is worth reading before changing anything here. What is genuinely different
in this copy is short enough to list:

- **Two file types where Slipcase has one.** `.dclx` and `.dclg`, each with its
  own ProgID, its own content type, and its own icon. `install.ps1` is written
  from a table of two rather than a run of constants, and `AppxManifest.xml.in`
  carries two `uap:Extension` elements.
- **Three icon directories where Slipcase has one**, for the same reason: the
  application, plus one per file kind so a `.dclx` and a `.dclg` do not draw
  the same picture in Explorer.
- **An empty certification baseline.** `build-msix.ps1`'s `$KNOWN_FINDINGS` is
  empty until a kit run puts something in it. Copying Slipcase's would be
  carrying a list of things somebody else measured.
- **The signing certificate is already trusted**, because Partner Center
  assigns `Publisher` per account and not per product: Segler's is the same
  X.500 string Slipcase's is, so the throwaway certificate in
  `LocalMachine\TrustedPeople` from that application's test signing matches
  this one. The one administrator action Slipcase's README describes has
  already been spent on this machine, and a different machine will have to
  spend it again.

**If you install with the scripts and later install Segler from the Microsoft
Store, run `uninstall.ps1` first.** The reason is not the one slipcase-desktop
records, and this paragraph said that reason for as long as it took to run the
checklist once.

**Measured here on 2026-09-04, in both orders, CHECKLIST item 22.** With the
package and the scripts both registered, Windows puts up no picker and asks
nothing: it opens the **script's** copy from `%LOCALAPPDATA%`, every time. The
extension's default value in `HKCU\Software\Classes\.dclx` is the script's
ProgID, and a packaged association only ever adds itself to `OpenWithProgids` -
it never claims the default. So the script install silently shadows the Store
one, and it does so whether the package went on before the scripts or after.
There was no `UserChoice` involved in either run.

That is worse than the picker slipcase-desktop describes, not better. A picker
at least asks. This way, somebody who installs from the Store to get a newer
Segler goes on running whatever the script left behind, with nothing anywhere
saying so - and the Store copy they are looking at in Settings is installed,
present, and never reached. `uninstall.ps1` is the fix and is why it is the
thing to run first.

Why slipcase-desktop saw a picker and this did not is not established. That
machine may have carried a `UserChoice` from a person having chosen "always
open with" at some point, which outranks everything; this one had none. Nothing
here says that repository's note was wrong when it was written - it says this
one had to be measured rather than inherited, which is the same lesson its own
`README` keeps recording about itself.

**The stale-`UserChoice` states below are still slipcase-desktop's measurement
and have not been reproduced here.** They are kept because `uninstall.ps1`
removes that key on their authority, and because none of them is repairable
from inside a package: an MSIX runs no code at install time, and one running
later cannot write the key back, because a package's registry writes are
virtualised.

## What is here

| File | What it is |
| --- | --- |
| `install.ps1` | Writes the registry keys, copies the files, makes the Start menu shortcut |
| `uninstall.ps1` | Removes all of it. Copied into the install directory, because Add/Remove Programs points at it and a checkout may be gone |
| `segler.ico` | The application icon, nine sizes. Built from the Linux SVG, not drawn separately |
| `dclx.ico`, `dclg.ico` | The two document icons, the same nine sizes, from the two Linux document SVGs |
| `assets/` | The six PNGs `AppxManifest.xml` names and their scale variants, from the same three SVGs. Committed for the same reason the `.ico` files are |
| `listing/` | The Store logo at the two sizes Partner Center's listing form accepts. Not in the package |
| `make-ico/` | The tool that builds all of it |
| `AppxManifest.xml.in` | The MSIX manifest, with the identity and the version left as placeholders |
| `identity.psd1` | What Partner Center assigned when the name was reserved. Not committed; `identity.psd1.example` is the template |
| `build-msix.ps1` | Builds the package from a release binary, and optionally signs it and runs the certification kit |
| `check-imports.ps1` | Walks the PE import table and refuses any DLL not known to ship with Windows |
| `screenshot.ps1` | Photographs the window at a size the Store accepts |
| `segler-desktop.manifest` | The Win32 application manifest, embedded by `crates/segler-desktop/build.rs` |

## Two scripts rather than an installer

The lightest thing that registers two extensions properly and uninstalls
cleanly, which is what the application needs: one executable, three icons, and
no runtime files of its own. MSI through WiX, Inno Setup, and NSIS were all
considered and all rejected on slipcase-desktop for the same reason - each needs
a toolchain that is not on a stock Windows and not in this repository's build,
to produce a package that would do what forty lines of registry writes do.
`packaging/linux` is a pair of shell scripts for the same reason, and these two
are its counterpart, argument for argument.

That rejection stands here, and now the channel decides it too. The Store takes
MSIX, WiX builds MSI, and the two are not steps on one path. The two scripts
stay as well: they are the per-user, no-toolchain, no-account route, and a Store
listing is not a reason to take that away from somebody who would rather not
have one.

## Why the Store at all

The same reason macOS took the Mac App Store. A person who has been sent a
DocLang file double-clicks it, Windows offers to search the Store by file type,
and outside the Store that search returns nothing. For a format almost nobody
has a handler for, that search is the discovery path.

## What gets written by the scripts

Everything under `HKEY_CURRENT_USER`, per-user and with no elevation, which is
the counterpart of the Linux script's default of `~/.local`. There is no
all-users variant: the machine-wide half of every key here needs administrator,
and a script that sometimes needs it and sometimes does not is worse than one
that never does.

Two of each of the first six rows, one per file kind.

| Key | Value | Why |
| --- | --- | --- |
| `Software\Classes\.dclx` | `Excelano.Segler.Archive` | The extension names the type |
| `Software\Classes\.dclx` → `Content Type` | `application/vnd.doclang.archive+zip` | Name to type |
| `Software\Classes\MIME\Database\Content Type\…` → `Extension` | `.dclx` | The same statement, type to name |
| `Software\Classes\Excelano.Segler.Archive` | `DocLang archive` | What Explorer's Type column shows |
| `…\DefaultIcon` | `dclx.ico,0` | What Explorer draws |
| `…\shell\open\command` | `"…\segler-desktop.exe" "%1"` | What a double-click runs |
| `…\Application` → `ApplicationName` | `Segler` | The name a person recognises |
| `Software\Classes\Applications\segler-desktop.exe` | `FriendlyAppName`, both types under `SupportedTypes` | The Open With list |
| `Software\Microsoft\Windows\CurrentVersion\Uninstall\Segler` | — | Add/Remove Programs |

and `.dclg` the same, as `Excelano.Segler.Document`, *DocLang document*, and
`dclg.ico`.

**Only the ProgIDs and the type names are chosen here.** The extensions are the
specification's; `application/vnd.doclang.document+xml` is the specification's;
`application/vnd.doclang.archive+zip` is Segler's own provisional name and
`packaging/linux/mime/doclang.xml` is where that choice is argued and where it
will be changed if the DocLang project adopts one. This directory follows that
file rather than restating it, and the manifest follows it too, so all three say
the same strings.

Neither extension reserves magic bytes, so the extension is the only
identification Windows has: there is nothing to sniff and no `sub-class-of` to
fall back on the way shared-mime-info has one.

`FriendlyTypeName` is written as a plain string. The usual form is a reference
into a binary's resource table — `@C:\path\thing.dll,-123` — which needs
`SHLoadIndirectString` to read back, and nothing this project ships would
resolve one.

## Things measured on slipcase-desktop that apply unchanged here

**Windows PowerShell 5.1 reads a script with no byte order mark as ANSI, and an
em dash in a string literal is then a syntax error.** The UTF-8 bytes of an em
dash decode under Windows-1252 to three characters, the last of which PowerShell
accepts as a *string delimiter* — so the string ends in the middle of a sentence
and the parser reports a missing terminator two hundred lines later. Every
string these scripts print is ASCII, which it should have been anyway: these
messages go to a console whose code page is nobody's to predict. Comments keep
theirs, because a comment is never parsed as a string.

**`assoc` and `ftype` do not see any of this.** They report an extension as
having no association at all after a successful script install, because they
read and write the machine-wide half of the class root only.
`reg query HKCU\Software\Classes\.dclx /s` is the check that works.

**PowerShell's registry provider cannot write a media type key.** The names here
contain forward slashes and the provider reads one as a path separator: it
creates `application` with a child and reports success. Both scripts use
`[Microsoft.Win32.Registry]` instead, which takes the whole string as one name.

**A stale `UserChoice` is the dead association to worry about**, and what a
double-click then does depends on what the stale key points at, which is not a
distinction anyone had anticipated:

| `UserChoice` names | What a double-click does |
| --- | --- |
| A ProgID that exists, whose command names a deleted executable | **Refused: *Application not found***. The package is ignored and no picker appears |
| A ProgID that no longer exists at all | The package wins and launches from `WindowsApps` |
| Nothing | The package wins |

`uninstall.ps1` removes that key for both extensions, which is why it is the
thing to run before installing the package over a script install.

**`AssocQueryString` answers `ERROR_NO_APPLICATION_ASSOCIATED` for the
executable and the command line of a packaged handler** while still returning
its friendly names, because there is no command line — activation goes through
the app model. A script that verifies an install by looking for an executable
path will report a correct package as no association at all.

## The icons

`segler.ico`, `dclx.ico` and `dclg.ico` are built from the three SVGs in
`packaging/linux/icons`, which are the source for every platform's icons and are
not duplicated here:

    cd packaging/windows/make-ico && cargo run --release

Nine sizes each — 16, 20, 24, 32, 40, 48, 64, 128, 256. The three the shell asks
for are 16, 32, and 48; the rest are those again at the display scalings Windows
offers, plus 256 for the extra-large view. Entries above 48 are stored as PNG
and the rest as bitmaps, which is the convention and saves a quarter of a
megabyte on the 256 alone.

The same run writes `assets/`, which is what the MSIX ships, and `listing/`,
which is what Partner Center's listing form takes and which is deliberately
*not* in the package: a file added to `assets` lands in the MSIX, and a package
that gains a file has to be certified again for an image no installed copy would
ever read.

`make-ico` is its own package rather than a workspace member, so nothing it
depends on reaches the shipped binaries. It renders with `resvg` and assembles
with `ico`, both pure Rust; `cargo tree -i cc` finds nothing in it either.

## The window's own icon, and the taskbar

**Neither egui, eframe, nor winit sets an AppUserModelID.** `APP_ID` in
`crates/segler-desktop/src/main.rs` is `with_app_id`, which is Wayland's
`xdg_toplevel.set_app_id` and does nothing at all on Windows. Measured on
slipcase-desktop by reading all three crates.

That is left alone deliberately, and the Start menu shortcut carries no
AppUserModelID either. Setting one on the shortcut without the process declaring
the same identity through `SetCurrentProcessExplicitAppUserModelID` would break
the pairing rather than fix it — and that call is raw FFI, which
`#![deny(unsafe_code)]` puts out of reach. With neither side declaring one,
Windows derives both from the executable's path, they agree, and pinning and
taskbar grouping work.

**The window icon is embedded as bytes, not compiled into a resource.** Windows
takes a window's icon from a resource in the executable, and building one needs
`rc.exe` or `windres` — a build step `DESIGN.md` §5 keeps out. So `main.rs`
carries `segler.ico` through `include_bytes!` and hands the 64-pixel entry to
the window at startup: 64 is a whole multiple of the sizes a display at 100% or
200% asks for — 16 and 32 in the title bar, 32 and 64 in the task bar — so each
of those is an integer downsample rather than a resample of a resample. It is
not a multiple of what 125% and 150% ask for, and those are resampled;
slipcase-desktop looked at both and the cost is nothing a person notices.

This is why the `.ico` files are committed artifacts in a repository that
otherwise holds only sources: the executable references one at compile time, and
Windows has no step that would rasterize an SVG for either purpose.

**The application manifest is the one thing here that is a build step**, and it
compiles nothing. `crates/segler-desktop/build.rs` hands the MSVC linker
`/MANIFEST:EMBED` and `/MANIFESTINPUT` and the linker that was already linking
the binary embeds `segler-desktop.manifest`. Its only content is the DPI
declaration, which the certification kit reads out of the manifest rather than
out of the running process — slipcase-desktop's kit reported that application as
not DPI aware until this existed.

## What a Store build is

`build-msix.ps1` produces it and `RELEASE.md` has the process; what belongs here
is why it is shaped that way.

**Signing is not optional for a local install.** The shell will not accept an
unsigned MSIX, so unlike macOS there is no unsigned build-and-test loop. The
Store signs what it distributes, so the package that goes up is unsigned and the
throwaway-signed copy is only for installing locally — and the two must come
from one release binary with no rebuild between, because a rebuild of identical
source produces a different file. `build-msix.ps1` packs from the same
`target/release/segler-desktop.exe` every time and only the signature differs,
which is what makes it safe to take the signed copy aside and re-run without
`-SelfSign` for the one that is uploaded.

**The manifest declares `runFullTrust` and nothing else.** A capability asked for
and unused is a question at certification with no good answer, and the
justification field caps at 500 characters and truncates silently at the paste.

**The package carries the window and not the command-line tool.** A packaged
application's executables live under `WindowsApps` behind an app-execution
alias, declaring one is a manifest extension nobody has asked for, and a
command-line tool a person cannot type the name of is worse than one they
install another way. The Debian package carries both because one `apt install`
is one product; the Store package is the window. `RELEASE.md` says the CLI ships
through cargo-dist.
