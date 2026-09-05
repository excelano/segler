# Release: getting Segler into apt and two stores, repeatably

**This is the process, not the history.** What a given release did is in
`git log` and `CHANGELOG.md`; what is here is what the next one costs and in
what order. Anything a machine can do is a script under `packaging/`; where a
step is prose, that is a claim it cannot be scripted, and a later reader is
invited to prove it wrong. The loop is slipcase-desktop's, which is the
hand-cut loop in the fleet's `~/notes/releasing.md`.

Two documents are named here and not committed, because each carries an
account's own identifiers: `packaging/windows/SUBMITTING.local.md` and
`packaging/macos/SUBMITTING.local.md`. `packaging/windows/identity.psd1` is the
same and has an `identity.psd1.example` beside it.

## The order

1. **Linux**, which needs no other machine and where most shared work lands.
2. **Windows**, on the Windows lane.
3. **macOS**, on the Mac lane.
4. **Back on Linux**, for the readiness review across all three.

Nothing is submitted to a store until step 4. apt is the exception, taken
deliberately: it is our own repository, publishing is one command and
unpublishing is a prune, and nothing sits in anybody's review queue meanwhile.
What that costs is that the readiness review has one more thing to check: that
what apt is serving is a version the stores also have, or a later one whose
difference is understood.

## One number, three spellings

The workspace `Cargo.toml` holds the version and nothing else should.
`packaging/version.sh` is the only thing that reads it.

| Where | Shape | Rule |
| --- | --- | --- |
| `Cargo.toml` | `X.Y.Z` | The source. |
| `AppxManifest.xml` | `X.Y.Z.0` | Four parts, and the Store requires the fourth to be `0`. |
| `Info.plist` `CFBundleShortVersionString` | `X.Y.Z` | What a person sees. |
| `Info.plist` `CFBundleVersion` | the first-parent commit count | Must increase on every upload, including a rejected one resubmitted unchanged. |

**Bump only for a number that has been tagged.** A code change costs a
certification re-run either way; only a published number costs a version as
well. Ask `git tag --list` before deciding.

## Linux

    cargo build --release --workspace
    ./packaging/linux/check-libraries.sh          # both display backends
    ./packaging/debian/build-deb.sh
    ./packaging/preflight.sh --corpus ~/clones/doclang --ci

`preflight.sh` is the gate: a clean tree, nothing unpushed, both changelogs
naming the version, a version the Appx spelling can represent, silent clippy, a
formatted tree, a passing suite, the corpus round-tripping and validating, and
CI green on `HEAD` rather than on some earlier commit. It refuses and never
repairs.

Then tag, release, and ship:

    git tag -a vX.Y.Z            # the commit the store packages were built from
    gh release create vX.Y.Z dist/segler_X.Y.Z_amd64.deb --notes-file …
    apt-ship segler vX.Y.Z -y

amd64 only, and say so wherever the install is written. Nothing here
cross-compiles and there is no arm64 machine to run a build on.

## Windows

**Built, signed, installed and certified on 2026-09-04.** What follows is the
process; `packaging/windows/README.md` is the reasoning and `git log` is what
each run found.

    cargo build --release --workspace
    powershell -ExecutionPolicy Bypass -File packaging\windows\build-msix.ps1 -SelfSign
    # from an elevated prompt, once the package is worth certifying:
    powershell -ExecutionPolicy Bypass -File packaging\windows\build-msix.ps1 -SelfSign -Certify

`build-msix.ps1` refuses rather than repairs, and its refusals are the point of
it: a missing `identity.psd1`, a `Publisher` that is not an X.500 string, a
version `version.sh` will not spell four ways, a binary of the wrong
architecture, a debug binary (which is a console-subsystem one, and packaging
that puts a console window behind the application), a binary importing a DLL
Windows does not ship, a manifest placeholder it does not substitute, a
`makepri` run that split resources into a bundle's shape, and a certification
report that is missing, stale, or says something new.

Then take the certified package aside and repack the one that is uploaded:

    mv dist\Segler-X.Y.Z.0-x64.msix dist\Segler-X.Y.Z.0-x64-signed-certified.msix
    powershell -ExecutionPolicy Bypass -File packaging\windows\build-msix.ps1

**The Store is given the unsigned package** - it signs what it distributes - and
the signed copy is only for installing here. Both come from one
`target/release/segler-desktop.exe` with no rebuild between, which is what "do
not rebuild before uploading" means: only the signature differs between the file
the kit passed and the file that goes up. Rebuilding the *binary* is what must
not happen.

**One administrator action, once per machine.** The throwaway signing
certificate has to reach `LocalMachine\TrustedPeople`; the per-user store is not
read for this and importing there leaves deployment failing `0x800B0109` just
the same. `build-msix.ps1` prints the two commands rather than attempting them.
On a machine that has test-signed slipcase-desktop this is already spent:
Partner Center assigns `Publisher` per account, so both applications carry the
same X.500 subject and one certificate signs both.

### The certification finding, and the decision

The Windows App Certification Kit reports **PASS overall** with one test reading
**FAIL: Blocked executables**, four messages: a reference to
`kernel32.dll!CreateProcessW`, and blocked-executable references to `cmd.exe`,
`\cmd.exe` and `Csi`.

Traced rather than tolerated. The first three are the Rust standard library's
batch-file spawn path in `std::process`, linked in because `webbrowser` is - it
arrives under `egui-winit` and is what egui opens a hyperlink with. Nothing in
this repository calls `Command::new`. The fourth is a substring scan hitting
bytes that are not a name: the binary holds `Csinhf`, the statically linked
UCRT's complex-sinh symbol, and a three-byte run inside `.text`. Neither is
csi.exe and there is nothing to remove.

The test is `OPTIONAL="TRUE"` in the report and the package is
`APP_TYPE="Centennial"`, which is why an overall of PASS sits over a test
reading FAIL. **The decision is to submit with it failing**, which is the
decision slipcase-desktop took on 2026-08-28 for the same finding and which its
certification then accepted. `build-msix.ps1`'s `$KNOWN_FINDINGS` records it so
the gate is quiet about this and loud about anything else; that gate was checked
in both directions, by running with the list empty and watching it refuse.

`DPIAwarenessValidation` passed on the first run, which is the one non-optional
test in the report's last requirement. slipcase-desktop failed it until
`build.rs` existed; this repository had the embedded manifest from its first
build.

### What the Partner Center submission needs

- The unsigned `dist/Segler-X.Y.Z.0-x64.msix`.
- `packaging/store-listing.md` for every text field.
- `packaging/windows/listing/store-logo-1080.png` for the *Store logo* field,
  which refuses anything but 1080x1080 or 2160x2160 - not the 300x300 the older
  documentation describes.
- Screenshots at 1366x768 or larger, taken with `packaging/windows/screenshot.ps1`
  against the installed package. Record which document was used in
  `store-listing.md`.
- `SUBMITTING.local.md` beside the packaging, which is not committed and is
  where what the form did with all of it is written down.

The Partner Center reservation is **Segler**, and `identity.psd1` holds what it
assigned.

## macOS

Cloned from `slipcase-desktop/packaging/macos` by the Mac lane. What carries
over: `build-app.sh`, the entitlements and the sandbox, the universal binary,
`CFBundleVersion` from `version.sh --build`. What changes: two exported type
declarations rather than one, `CFBundleTypeRole` **Editor** for both because
this application writes the document back, and the `.icns` from the same SVG.

**One thing is a blocker rather than a clone.** A Mac App Store submission of
slipcase-desktop was refused under Guideline 2.5.1 for a private CoreGraphics
symbol that `winit` 0.30 declares whether or not it is called, and review reads
the symbol table. Until a winit release ships the upstream gate, a Store build
needs slipcase-desktop's `[patch.crates-io]` on `excelano/winit`, pinned by
revision, in the workspace `Cargo.toml`. Its `Cargo.toml` says which revision
and why the lockfile moves by one line when it is added. Add it on the Mac lane
and commit it; it is harmless on the other two platforms.

The App Store Connect record is **Segler**, App ID `com.excelano.segler-desktop`,
SKU `segler-desktop`. Apple drops a reserved name after an unstated period
with no build uploaded, so the first Store build is a deadline as well as a
step.

## Step 4: the readiness review

Before either store submission, on Linux, with all three artefacts built from
one tagged commit:

- `packaging/store-listing.md` agrees with `CHANGELOG.md`, claim by claim,
  against the built application and not against memory. This is the drift
  slipcase-desktop caught most often.
- The version is the same in every spelling, and `CFBundleVersion` is higher
  than the last upload's.
- `CHECKLIST.md` has been run on each platform against the packaged
  application, not a developer build.
- apt is serving the version the stores are about to be given.
