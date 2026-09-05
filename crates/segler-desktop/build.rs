//! Embed the Windows application manifest, and do nothing else ever.
//!
//! This is the workspace's only build script and `DESIGN.md` §5 is why it is
//! worth reading before adding a second thing to it. That section's rule is
//! that nothing compiles C and that a build needs a Rust toolchain and nothing
//! else. This holds to both: it prints two linker arguments and the linker that
//! was already linking the binary embeds
//! `packaging/windows/segler-desktop.manifest`. No resource compiler, no object
//! file, nothing compiled that was not compiled before.
//!
//! The distinction matters because the obvious way to do this is `rc.exe` or
//! `windres`, and `packaging/windows/README.md` rejects exactly that for the
//! window icon - which is why the icon travels through `include_bytes!`. That
//! rejection is of the resource compiler and not of the outcome, and the linker
//! route needs no compiler.
//!
//! **A second use for this file is a decision, not a precedent.** The one
//! opened here is narrow on purpose. `RELEASE.md` names it as the one build
//! script the tree may carry.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]

use std::path::Path;

fn main() {
    // The manifest lives with the rest of this platform's files under the
    // workspace's `packaging/`, which is the same rule as staying inside your
    // own directory there. Two levels up from the crate, where slipcase-desktop
    // needs one, because that repository is a single crate and this is a
    // workspace.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("packaging")
        .join("windows")
        .join("segler-desktop.manifest");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", manifest.display());

    // Read from the environment rather than from `cfg!`, because a build script
    // is compiled for the host and `cfg!(windows)` in here answers about the
    // machine doing the building. Cross-checking from Linux with
    // `--target x86_64-pc-windows-msvc` is a thing this repository does, and it
    // would take the wrong branch.
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os != "windows" || env != "msvc" {
        return;
    }

    // `/MANIFEST:EMBED` is MSVC's, which is why the guard above tests the
    // environment and not just the operating system: a `windows-gnu` target
    // links with something that would not understand it.
    //
    // Named rather than `-bins`, because the workspace builds two binaries and
    // `segler` is a command-line tool with no window to be aware about. The
    // name is this crate's own binary either way, since a build script only
    // ever speaks for its own package.
    for arg in [
        "/MANIFEST:EMBED".to_string(),
        format!("/MANIFESTINPUT:{}", manifest.display()),
    ] {
        println!("cargo:rustc-link-arg-bin=segler-desktop={arg}");
    }
}
