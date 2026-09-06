# macOS

`DESIGN.md` §8. The application bundle, the two type declarations, the three
icons, the sandbox, and the Store package. Five files here and everything
else is generated:

    Info.plist.in         the bundle's property list, @VERSION@ and @BUILD@ substituted
    Segler.entitlements   what a development build is signed with
    build-app.sh          assembles dist/Segler.app, signs it, and for the Store wraps it
    check-install.sh      asks an installed bundle what it is, on the machine it is on
    screenshot.sh         photographs the window at a size App Store Connect accepts
    window-probe.swift    asks the window server whether a window appeared, for CI

Build it, sign it, register it:

    cargo build --release
    ./packaging/macos/build-app.sh --sign "Apple Development: …"
    lsregister -f dist/Segler.app

`lsregister` is not on `PATH`. It lives at
`/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister`,
and `build-app.sh` prints the full line. `lsregister -u` unregisters, which
matters because `dist/` is ignored by git and a deleted bundle otherwise
leaves a claim behind it.

This directory is slipcase-desktop's cloned, and what that repository
measured is taken as measured here unless a paragraph below says it was
measured again. Its `packaging/macos/README.md` is the long form; this file
records what is Segler's own.

## The bundle is the unit

A bare executable draws a window on macOS, but it is not a thing the platform
can associate anything with: `lsappinfo` reports `bundleID=[ NULL ]` for one,
and Launch Services files it as a nameless foreground process. Nothing below
works without the bundle.

## Two types, imported, with two icons

`UTImportedTypeDeclarations` says what a DocLang archive and a DocLang
document *are*; `CFBundleDocumentTypes` says that this application opens and
edits them. A bundle carrying only the second claims documents it never
described; one carrying only the first describes types nothing opens.

**Imported rather than exported, which is where this bundle differs from
slipcase-desktop's.** That application defines its own format and exports the
type. DocLang is the LF AI & Data Foundation's, `spec.md` is its authority,
and this application edits it without defining it; exporting the type would
claim an ownership the platform would then believe and rank by. An imported
declaration is what macOS uses for an extension nothing else declares, which
today is every machine, and it yields to an exported declaration the day the
DocLang project ships one. `LSHandlerRank` is `Default` rather than `Owner`
for the same reason.

The identifiers, `com.excelano.doclang-archive` and
`com.excelano.doclang-document`, are in this application's namespace because
the specification names no UTI. They are provisional in the way
`packaging/linux/mime/doclang.xml` says the archive media type is: if the
project adopts identifiers, these change to them.

Conformance is `public.zip-archive` and `public.data` for the archive and
`public.xml` for the markup, the macOS half of what `sub-class-of` does on
Linux and true for the same reasons. There is no magic-bytes tag: a ZIP
signature identifies every OPC package alike and an XML prolog every XML file.

**Three `.icns`, not one.** `CFBundleIconFile` names the application's, and
each type declaration's `UTTypeIconFile` names its own, because a file type
draws its icon by its own mechanism and both types pointed at the
application's drawing would put one picture on a `.dclx` and a `.dclg` alike.
`build-app.sh` renders all three from the SVGs under `packaging/linux/icons`,
rewriting each SVG's declared size before every rendering so `sips` draws at
that size rather than upscaling a 64-pixel bitmap, and refuses a rendering
that came back the wrong size.

## The role is Editor

`CFBundleTypeRole` is `Editor` rather than `Viewer`. Save writes the document
back, so this application does modify the documents it opens and `Viewer`
would be a claim to the platform that is not true.

## Two identifiers

`com.excelano.segler-desktop` names the application, and it is the reverse-DNS
of `APP_ID` in `src/main.rs`, so the binary, the Linux desktop entry's
basename and this bundle all say `segler-desktop`. The type identifiers above
name the formats. They are different things, and a renamed format would not
move the bundle identifier.

## The double-click, and the one unsafe module

macOS does not deliver an opened document as `argv[1]` the way Linux and
Windows do; it launches the application with no arguments at all and sends an
Apple Event. `crates/segler-desktop/src/opened_document.rs` listens for it,
installing its handler at `applicationWillFinishLaunching:`, which
slipcase-desktop measured as the only moment of three that catches both a
cold launch and a document double-clicked into a running window. It is the
one module in this repository that writes `unsafe`, and `CLAUDE.md` says what
that costs.

Measured here on 2026-09-06 against a development-signed universal bundle on
an Intel Mac running macOS 15.7: `open archive-demo.dclx` with nothing
running produced a window titled for the archive, and `open some.dclg` into
that window replaced the document. `.github/workflows/apple-silicon.yml`
repeats the first half on arm64 on every push.

## Save under the sandbox

Every Store binary is sandboxed, and the sandbox is inert until the
entitlement is inside a signature, so an unsigned bundle carrying
`Segler.entitlements` proves nothing and every measurement here was made
against a signed one. An Apple Development identity is enough for that.

What the sandbox refuses is the obvious save. The grant a person gives by
choosing a document covers that file and not its directory, so a temporary
file beside the document, which is how every other platform replaces a file
whole, stops with *Operation not permitted*. `crates/segler-core/src/replace.rs`
carries the macOS arm: the rewrite waits in the directory macOS provides for
replacing a file from, on the document's own volume, and lands with
`replaceItemAtURL:`. Measured here the same day: Cmd+S on an unchanged
document under the sandbox rewrote it byte for byte, and nothing was left
beside it or in the application's container.

Two things the platform does to a saved file, both its and not this
application's. It marks the file `com.apple.quarantine` with this
application's name as the agent, the way it marks everything a sandboxed
process writes; macOS consults that mark only when something is about to
execute, so a document is unaffected. And the file takes the process's
primary group, as a file renamed over the original would on the other two
platforms; its mode is kept.

## Signing

`build-app.sh --sign IDENTITY` signs the finished bundle with
`Segler.entitlements`, last, because a signature covers what is in the bundle
when it is made. The script reads the entitlements back out of the signature
and refuses a bundle whose signature does not carry the sandbox.

**Which certificate does what.** An **Apple Development** identity signs a
bundle that runs and sandboxes here. An **Apple Distribution** identity is
what a Store upload is signed with, and a **Mac Installer Distribution**
identity, which `security` lists as *3rd Party Mac Developer Installer* and
never under `-p codesigning`, signs the package that carries it. A
**Developer ID Application** identity is for distributing outside the Store,
and only that path involves notarization: a Store submission is reviewed
rather than notarized.

## What a Store build is

`build-app.sh --store PROFILE` produces it and `RELEASE.md` has the process.
A universal bundle carrying the profile as `embedded.provisionprofile`,
signed with the distribution identity and entitlements generated from the
profile, wrapped by `productbuild` into a signed `.pkg`. The script refuses
before it builds on a missing, unreadable or expired profile, an application
identifier that does not match `CFBundleIdentifier`, and anything but exactly
one matching identity of each kind; and after, on a signature missing the
sandbox or the identifier, on any file still carrying `com.apple.quarantine`,
which App Store Connect refuses as ITMS-91109 hours after accepting the
upload, and on a bundle that does not verify.

`keychain-access-groups` is declined although the profile grants it: this
application touches no keychain, and a capability asked for and unused is a
question at review with no good answer.

**A Store-signed bundle cannot be launched here.** AMFI refuses a restricted
entitlement without a profile covering the machine, and a Mac App Store
profile covers none. So `--store` ends by withdrawing the bundle's Launch
Services claim: a submission build that stays registered is a handler
candidate at least as new as the installed copy, and slipcase-desktop
measured every double-click launching it and the kernel killing it. Anything
that needs a running application uses a development or Developer ID build,
and the real article is reached through TestFlight.

## No private symbol reaches the binary

Guideline 2.5.1, and it cost slipcase-desktop a review cycle: a private
CoreGraphics symbol that `winit` 0.30 declares whether or not it is called,
and review reads the symbol table. This tree's first release binary carried it
too, measured 2026-09-06. The workspace `Cargo.toml`'s `[patch.crates-io]`
removes it and says when to delete itself, and `build-app.sh` refuses to
bundle an executable that imports a symbol from a system framework which that
framework's own public headers do not declare.

## The minimum system version is 11.0

Nothing here calls anything newer than the Apple Event manager and
`NSFileManager`, and 11.0 is where Apple silicon begins, so an arm64 slice
cannot claim lower whatever the floor says. The declaration binds the bundle
and not the bare executable, which Cargo builds for 10.12 on x86_64 by
default; `build-app.sh --universal` refuses a slice whose floor disagrees with
the property list's, and

    MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release -p segler-desktop --target …

is what makes them agree.
