//! Put bytes where a file is, leaving the old file whole if anything fails.
//!
//! Two platforms do this the obvious way: a temporary file beside the target,
//! written in full and renamed over it, which is atomic on every filesystem a
//! document is likely to live on. macOS cannot, and the reason is measured
//! rather than reasoned about. Under the App Sandbox, which every Mac App
//! Store build runs in, the grant a person gives by choosing a file in the
//! open panel covers that file and not the directory holding it, so creating
//! a randomly named sibling stops with *Operation not permitted* before a byte
//! is written. slipcase-desktop measured it on 2026-08-25 and its `git log`
//! holds the run; this module is that repository's `staging.rs` with the
//! library's own staging folded in, since this crate is the library.
//!
//! The macOS arm reserves the rewrite in the directory the platform provides
//! for exactly this, `NSItemReplacementDirectory` asked for with the target's
//! URL so that it lands on the target's own volume, and lands it with
//! `-[NSFileManager replaceItemAtURL:withItemAtURL:…]`, which is the call
//! Apple sanctions for replacing a file a person chose. It preserves the
//! original's metadata, which is what the rename promised too. The volume
//! matters: slipcase-desktop first staged under `TMPDIR`, and a replacement
//! whose two ends are on different volumes refuses with `EXDEV` on APFS, HFS+,
//! FAT32 and exFAT alike, so nothing on an external drive or a share could be
//! saved.
//!
//! No unsafe. The bindings in `objc2-foundation` are safe functions, and this
//! crate stays `forbid(unsafe_code)` with this module in it.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::fs;
use std::io;
use std::path::Path;

/// Write `bytes` to `path`, replacing whatever is there.
///
/// A file that does not exist yet is created in place on every platform.
/// There is nothing to keep whole, and on macOS a new path chosen in the save
/// panel is granted while a sibling of it is not.
///
/// # Errors
///
/// Whatever the filesystem says, and on macOS whatever the platform says about
/// the replacement.
pub(crate) fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if !path.exists() {
        return fs::write(path, bytes);
    }
    replace(path, bytes)
}

#[cfg(not(target_os = "macos"))]
fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let tmp = tempfile::NamedTempFile::new_in(dir)?;
    fs::write(tmp.path(), bytes)?;
    // A temporary file is created readable by its owner alone, and a rename
    // carries the temporary file's mode, not the original's. Without this
    // every document saved on Linux came back 0600 whatever it had been,
    // which the fleet's CI found the first time the test below ran there:
    // macOS never reached this arm, and no Linux run had asked. The original
    // exists here, since `write` handles the new-file case before this.
    #[cfg(unix)]
    fs::set_permissions(tmp.path(), fs::metadata(path)?.permissions())?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let landing = macos::Landing::reserved_for(path)?;
    fs::write(landing.staged(), bytes)?;
    landing.replace_original()
}

#[cfg(target_os = "macos")]
mod macos {
    use std::io;
    use std::path::{Path, PathBuf};

    use objc2::rc::Retained;
    use objc2_foundation::{
        NSFileManager, NSFileManagerItemReplacementOptions, NSSearchPathDirectory,
        NSSearchPathDomainMask, NSString, NSURL,
    };

    /// A directory the platform made for one replacement, the rewrite waiting
    /// in it, and the file it will become.
    pub(super) struct Landing {
        scratch: Scratch,
        staged: PathBuf,
        original: PathBuf,
    }

    impl Landing {
        /// Reserve a rewrite of `original`, somewhere the replacement can
        /// reach it from and nowhere near it.
        ///
        /// The path is resolved first, for the reason a rename would resolve
        /// it: a document reached through a symbolic link should have the
        /// document replaced and not the link. It also has to be resolved
        /// before it is asked about, since the volume that matters is the
        /// one the document is on rather than the one the link is on.
        pub(super) fn reserved_for(original: &Path) -> io::Result<Self> {
            let original = std::fs::canonicalize(original)?;
            let scratch = Scratch::on_the_volume_holding(&original)?;
            // The same name it will have again, so anything that looks at the
            // staged file sees a document named the way documents are named.
            let name = original
                .file_name()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("{} names no file", original.display()),
                    )
                })?
                .to_owned();
            let staged = scratch.0.join(name);
            Ok(Self {
                scratch,
                staged,
                original,
            })
        }

        pub(super) fn staged(&self) -> &Path {
            &self.staged
        }

        /// Move the rewrite onto the original.
        ///
        /// The original's metadata is what survives, which is what a rename
        /// over it would have kept: permissions come from the file being
        /// replaced rather than from the umask. Passing
        /// `NSFileManagerItemReplacementUsingNewMetadataOnly` would ask for
        /// the other behaviour, and it is not passed.
        pub(super) fn replace_original(self) -> io::Result<()> {
            NSFileManager::defaultManager()
                .replaceItemAtURL_withItemAtURL_backupItemName_options_resultingItemURL_error(
                    &url_for(&self.original),
                    &url_for(&self.staged),
                    None,
                    NSFileManagerItemReplacementOptions::empty(),
                    None,
                )
                .map_err(|e| {
                    io::Error::other(format!(
                        "cannot replace {}: {}{}",
                        self.original.display(),
                        e.localizedDescription(),
                        because_of(&e)
                    ))
                })?;
            // After the replacement, because the staged file lives in the
            // scratch directory until the call above has moved it out.
            drop(self.scratch);
            Ok(())
        }
    }

    /// The directory macOS made for one replacement, removed when it is over.
    struct Scratch(PathBuf);

    impl Scratch {
        /// Ask macOS for a directory a file can be replaced *from*.
        ///
        /// `NSUserDomainMask` is not a choice: `NSItemReplacementDirectory`
        /// is documented to take that one, and `appropriateForURL:` is what
        /// decides where the directory lands. For a document on the boot
        /// volume it is under the per-user temporary area; for one on a
        /// second volume it is on that volume, which is the whole point.
        fn on_the_volume_holding(original: &Path) -> io::Result<Self> {
            let url = NSFileManager::defaultManager()
                .URLForDirectory_inDomain_appropriateForURL_create_error(
                    NSSearchPathDirectory::ItemReplacementDirectory,
                    NSSearchPathDomainMask::UserDomainMask,
                    Some(&url_for(original)),
                    true,
                )
                .map_err(|e| {
                    io::Error::other(format!(
                        "nowhere to rewrite {}: {}",
                        original.display(),
                        e.localizedDescription()
                    ))
                })?;
            // A file URL always has a path; the `Option` is for the ones that
            // do not, and this call cannot return one of those.
            let path = url.path().ok_or_else(|| {
                io::Error::other("macOS named a replacement directory with no path")
            })?;
            Ok(Self(PathBuf::from(path.to_string())))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // The whole tree rather than the directory alone: a successful
            // replacement has moved the staged file out and leaves this
            // empty, and a failed one leaves the file sitting in it.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Whatever the failure was underneath Cocoa's sentence, if it said.
    ///
    /// `localizedDescription` is written for a person looking at a dialog
    /// and hides the only fact worth having: the cross-volume failure
    /// reports *couldn't be saved in the folder* and nothing else, and the
    /// `EXDEV` under it names the defect outright. Appended rather than
    /// substituted, so the sentence a person can act on is still first.
    fn because_of(error: &objc2_foundation::NSError) -> String {
        use std::fmt::Write;
        error
            .underlyingErrors()
            .iter()
            .fold(String::new(), |mut so_far, under| {
                let _ = write!(
                    so_far,
                    " ({} {})",
                    under.domain(),
                    under.localizedDescription()
                );
                so_far
            })
    }

    fn url_for(path: &Path) -> Retained<NSURL> {
        NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
    }
}

#[cfg(test)]
mod tests {
    use super::write;

    /// The defect this catches is a rewrite that never lands: every
    /// platform's arm has to end with the target holding what was written,
    /// and macOS reaches that through a different call from the other two.
    #[test]
    fn what_was_written_ends_up_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("some.dclg");
        std::fs::write(&path, b"before").unwrap();

        write(&path, b"after").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"after");
    }

    /// A save to a name that does not exist yet creates it, and a directory
    /// holding nothing else afterwards is how it is checked that no scratch
    /// file was left beside it on either path.
    #[test]
    fn a_new_file_is_created_and_nothing_is_left_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.dclg");

        write(&path, b"fresh").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"fresh");
        let beside: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(beside, vec![std::ffi::OsString::from("new.dclg")]);
    }

    /// The defect this catches is a file coming back readable by people it
    /// was not readable by before. A rename over the original keeps the
    /// original's mode because the temporary file's is what a rename
    /// carries, and that is set from the umask; the macOS arm depends on
    /// `replaceItemAtURL:` putting the original's metadata back instead.
    /// Both arms are asserted rather than trusted.
    #[cfg(unix)]
    #[test]
    fn the_mode_of_the_original_survives() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("private.dclg");
        std::fs::write(&path, b"before").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();

        write(&path, b"after").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o640,
            "the replacement did not keep the original's mode"
        );
    }

    /// The defect this catches is the rewrite waiting beside the document,
    /// which is what makes Save fail under the App Sandbox. A test cannot
    /// enter a sandbox, so it asserts the property that made the sandbox
    /// refuse: while a rewrite is reserved, the document's own directory
    /// holds nothing but the document.
    #[cfg(target_os = "macos")]
    #[test]
    fn nothing_waits_beside_the_document() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("some.dclg");
        std::fs::write(&path, b"before").unwrap();

        let landing = super::macos::Landing::reserved_for(&path).unwrap();
        std::fs::write(landing.staged(), b"after").unwrap();

        let beside: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            beside,
            vec![std::ffi::OsString::from("some.dclg")],
            "the rewrite is waiting beside the document, where a sandbox cannot create it"
        );
        landing.replace_original().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"after");
    }
}
