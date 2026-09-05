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

Cloned from `slipcase-desktop/packaging/windows` by the Windows lane. What
carries over unchanged: `check-imports.ps1`, which walks the PE import table
and refuses any DLL not known to ship with Windows; `.cargo/config.toml`,
already here, which links the CRT in for that target alone because 0.1.1 of
slipcase-desktop linked `VCRUNTIME140.dll` and failed certification; and
`build-msix.ps1`'s four refusals. What changes: the manifest's names and
identity, the icon drawn from `packaging/linux/icons/segler-desktop.svg`
through `make-ico`, and the file type associations, which are two here
(`.dclx` and `.dclg`) where Slipcase had one. `build.rs` for the embedded
application manifest is the one build script the tree may carry, and it
compiles nothing.

Do not rebuild before uploading: a rebuild of identical source is a different
file, and the artefact uploaded has to be the one the certification kit passed.

The Partner Center reservation is **Segler**.

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
