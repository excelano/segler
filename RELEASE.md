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
    gh release create vX.Y.Z dist/segler_X.Y.Z_amd64.deb \
        packaging/review/sailing-directions.dclx --notes-file …
    apt-ship segler vX.Y.Z -y

The tag push also runs `publish-crate.yml`, which publishes the three crates, `segler-core` first, to crates.io
with the organisation's token before the GitHub release exists; the fleet's
`~/notes/releasing.md` step 5 is the procedure and the rule that a version
there is never re-published. Confirm it ran:

    gh run list --workflow=publish-crate.yml --limit 1

The archive is the reviewer's document; `packaging/review/README.md` says why
it rides the release rather than a website.

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
**FAIL: Blocked executables**, five messages: a reference to
`kernel32.dll!CreateProcessW`, and blocked-executable references to `cmd.exe`,
`\cmd.exe`, `Csi` and `CMd`.

Traced rather than tolerated. The first three are the Rust standard library's
batch-file spawn path in `std::process`, linked in because `webbrowser` is - it
arrives under `egui-winit` and is what egui opens a hyperlink with. Nothing in
this repository calls `Command::new`. The last two are a substring scan hitting
bytes that are not a name: the binary holds `Csinhf`, the statically linked
UCRT's complex-sinh symbol, and a three-byte run inside `.text`. Neither is
csi.exe and there is nothing to remove.

**`CMd` was four on 2026-09-04 and five on 2026-09-06**, against the v0.1.0
package, and the difference is the binary and not the kit. All three of its
occurrences were located and none is a name: one is the displacement bytes
`43 4d 64 00` of a `lea rax, [rip+0x644d43]` in `.text`, and the other two are
inside the embedded font data, the same run twice because two faces are
embedded. A displacement moves whenever any code above it moves, and two
commits landed between the two runs, so a new coincidental match is what to
expect rather than a surprise. It is the `Csi` finding again with different
bytes.

**The gate did not catch this, and it is not meant to.** `$KNOWN_FINDINGS`
matches on a test's name and verdict; the individual messages are printed for
a person to read. A new message inside a known finding therefore passes
quietly, which is the right trade for a test that will read FAIL on every run
this project ever does - but it means the messages are read at each release
rather than trusted. Reading them is what found this one.

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

**Built, signed, sandboxed and packaged on 2026-09-06**, on an Intel Mac
running macOS 15.7 with Xcode 26.3. What follows is the process;
`packaging/macos/README.md` is the reasoning and `git log` is what each run
found.

    MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release -p segler-desktop --target x86_64-apple-darwin
    MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release -p segler-desktop --target aarch64-apple-darwin
    ./packaging/macos/build-app.sh --store ~/Downloads/Segler_Mac_App_Store.provisionprofile
    ./packaging/macos/check-install.sh dist/Segler.app

`--store` produces what a submission is: a universal bundle carrying the
profile as `embedded.provisionprofile`, signed for distribution, wrapped by
`productbuild --component` into a signed `.pkg`. Nothing account-specific is
written down; the team and application identifier are read out of the
profile, so the profile is the only copy and cannot drift from a second one.
It refuses before it builds on a missing, invalid or expired profile, on an
application identifier that does not match `CFBundleIdentifier`, on a slice
built for a floor other than the one `Info.plist` declares, and on anything
but exactly one matching signing identity of each kind. It refuses after on a
signature missing the sandbox or the identifier, on any file still carrying
`com.apple.quarantine`, and on a bundle that does not verify.

`build-app.sh` also refuses a binary importing a symbol from a system
framework that the framework's public headers do not declare, the macOS
counterpart of `check-imports.ps1` and `check-libraries.sh`. slipcase-desktop
was refused under Guideline 2.5.1 for a private CoreGraphics symbol `winit`
links unconditionally, and this tree's release binary carried the same two
symbols until the workspace `Cargo.toml` took that repository's
`[patch.crates-io]` on `excelano/winit`. Delete the patch when a winit
release carries the gate, and the check is what says whether it is safe to.

**The deployment floor is not optional.** Without it the x86_64 slice is
built for 10.12 while the bundle declares 11.0, and `build-app.sh --universal`
refuses the disagreement rather than shipping a bundle promising a floor its
executable does not keep.

**A Store-signed build cannot be launched off the Store**: AMFI refuses its
restricted entitlements without a profile covering the machine, and a Mac App
Store profile covers none. `--store` therefore withdraws the bundle's Launch
Services claim as its last step, and `check-install.sh` on the fresh package
reports Launch Services not knowing it, which is correct there and wrong for
an installed copy. Two things follow. Screenshots can never be of the exact
artefact uploaded, so build a development or Developer ID bundle from the
same commit with `--outdir dist-dev`, photograph that with
`packaging/macos/screenshot.sh`, and say so in `packaging/store-listing.md`.
And the walkthrough against the real article goes through TestFlight, which
exists for macOS and is the cheapest way onto an Apple silicon machine.

Then validate before uploading, because an upload refused for something local
is a slow way to learn it, and upload:

    xcrun altool --validate-app -f dist/Segler.pkg -t macos --apiKey KEY_ID --apiIssuer ISSUER_ID
    xcrun altool --upload-app   -f dist/Segler.pkg -t macos --apiKey KEY_ID --apiIssuer ISSUER_ID

The key is the one installed at `~/.appstoreconnect/private_keys`; the issuer
is in App Store Connect under Users and Access, Integrations, and is written
in `SUBMITTING.local.md` and nowhere else. **A rejection can arrive only by
email**: an upload can answer *UPLOAD SUCCEEDED with no errors* and be refused
afterwards with nothing in the web interface saying so, which is how
slipcase-desktop learned about ITMS-91109. Check mail after every upload.

**The two things the sandbox changed, both measured here.** A double-clicked
document does not arrive as an argument on macOS; it arrives as an Apple
Event, and `crates/segler-desktop/src/opened_document.rs` is the one module in
this repository that writes `unsafe` to receive it. And Save could not create
its temporary file beside the document, because the open panel's grant
covers the file and not its directory; `crates/segler-core/src/replace.rs`
carries the macOS arm. Both were run on 2026-09-06 against a
development-signed universal bundle: a cold `open` on an archive drew a window
titled for it, a second `open` replaced the document in that window, and
Cmd+S under the sandbox rewrote the file byte for byte with nothing left
beside it.

### What the App Store Connect submission needs

- `dist/Segler.pkg`, built from the tagged commit.
- `packaging/store-listing.md` for every text field.
- Screenshots at one of App Store Connect's sizes, `screenshot.sh`'s default
  being 1440x900, taken against a development or Developer ID bundle from
  the same commit.
- `SUBMITTING.local.md` beside the packaging, which is not committed and is
  where what the form did with all of it is written down, with the account's
  identifiers.

The App Store Connect record is **Segler**, App ID `com.excelano.segler-desktop`,
SKU `segler-desktop`. Apple drops a reserved name after an unstated period
with no build uploaded, so the first Store build is a deadline as well as a
step. The profile in `~/Downloads` is *Segler Mac App Store*, expiring
2027-08-29.

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
