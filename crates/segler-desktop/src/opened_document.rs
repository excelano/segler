//! The document macOS was asked to open, which does not arrive as an argument.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)
//
//! Linux and Windows both hand a double-clicked document to this application
//! as `argv[1]`, which is what `main` reads. macOS does not: it launches the
//! application with no arguments at all and then sends the document as an
//! Apple Event. With nothing listening, `AppKit` refuses the event and Finder
//! reports *Segler cannot open files in the "DocLang archive" format*, an
//! accusation against an application whose association is correct. This
//! module listens.
//!
//! **This is the one place in this application that writes `unsafe`**, and
//! `main.rs` carries the `allow` that permits it. `segler-core` is still
//! `#![forbid(unsafe_code)]` and nothing here is reachable from it. It is
//! slipcase-desktop's `opened_document.rs` with the class renamed, and every
//! measurement below was made there.
//!
//! It handles the event rather than replacing the application delegate.
//! `NSApplication` has exactly one delegate, winit sets its own inside
//! `EventLoop::new` and depends on it for startup, and there is no point in
//! the sequence where this code holds control in between. `NSAppleEventManager`
//! is additive: registering here displaces nothing of winit's.

use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::{Mutex, OnceLock};

use eframe::egui;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{define_class, msg_send, sel, AnyThread};
use objc2_app_kit::NSApplicationWillFinishLaunchingNotification;
use objc2_foundation::{
    NSAppleEventDescriptor, NSAppleEventManager, NSNotification, NSNotificationCenter, NSString,
    NSURL,
};

// Four-character codes, which are what the Apple Event world names things
// with. Spelled from their bytes rather than as hexadecimal so that the name
// and the number cannot drift apart: `kCoreEventClass` really is the four
// characters `aevt`. They are `FourCharCode`, which is `u32`, big-endian.
const CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
const OPEN_DOCUMENTS: u32 = u32::from_be_bytes(*b"odoc");
const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
const FILE_URL: u32 = u32::from_be_bytes(*b"furl");

/// The document macOS last asked for, waiting to be picked up by the window.
///
/// One rather than a list. An Apple Event can carry several documents, because
/// a person can select three and press Open, but this application shows one
/// document at a time and has nowhere to put the others. The first of the
/// event is taken and the rest are ignored, which is at least the one a person
/// clicked when they clicked one.
static ARRIVED: Mutex<Option<PathBuf>> = Mutex::new(None);

/// How to wake the window when a document arrives.
///
/// egui draws when something happens, and an Apple Event is not something that
/// happens to egui: without this the document would sit in `ARRIVED` until a
/// person moved the mouse over a window they had not asked to be looking at.
static WAKE: OnceLock<egui::Context> = OnceLock::new();

define_class!(
    // SAFETY:
    // - `NSObject` has no subclassing requirements.
    // - This type does not implement `Drop`.
    #[unsafe(super(NSObject))]
    // Named, because the Apple Event manager is told about it by selector and
    // a class with a generated name is harder to recognise in a crash report.
    #[name = "SeglerOpenDocuments"]
    struct Handler;

    impl Handler {
        // SAFETY: the signature is the one the Apple Event manager calls this
        // selector with, an event and a reply, both descriptors.
        #[unsafe(method(handleOpenDocuments:withReplyEvent:))]
        fn handle_open_documents(
            &self,
            event: &NSAppleEventDescriptor,
            _reply: &NSAppleEventDescriptor,
        ) {
            let Some(path) = first_path(event) else {
                return;
            };
            if let Ok(mut arrived) = ARRIVED.lock() {
                *arrived = Some(path);
            }
            if let Some(context) = WAKE.get() {
                context.request_repaint();
            }
        }
    }

    unsafe impl NSObjectProtocol for Handler {}
);

/// The first document in an open-documents event, as a path.
///
/// The direct object is a list of one descriptor per document, indexed from
/// one rather than zero. Each is coerced to `typeFileURL` and read as the bytes
/// of a URL, which is the shape Apple documents; a path is not taken from the
/// descriptor directly, because what it holds is an alias or a bookmark as
/// often as anything a path could be read out of. `NSURL` then does the
/// percent-decoding, so a document with a space in its name arrives with the
/// space rather than with `%20`.
fn first_path(event: &NSAppleEventDescriptor) -> Option<PathBuf> {
    let list = event.paramDescriptorForKeyword(DIRECT_OBJECT)?;
    for index in 1..=list.numberOfItems() {
        let Some(item) = list.descriptorAtIndex(index) else {
            continue;
        };
        let Some(url) = item.coerceToDescriptorType(FILE_URL) else {
            continue;
        };
        let Ok(text) = String::from_utf8(url.data().to_vec()) else {
            continue;
        };
        let Some(url) = NSURL::URLWithString(&NSString::from_str(&text)) else {
            continue;
        };
        if let Some(path) = url.path() {
            return Some(PathBuf::from(path.to_string()));
        }
    }
    None
}

/// Start listening for documents macOS asks this application to open.
///
/// Called once, before `eframe` runs. What this actually registers is a
/// notification observer; the Apple Event handler goes on at
/// `applicationWillFinishLaunching:`, which is the only moment that works and
/// was found by measuring the two that do not. Registering **before**
/// `NSApplication` exists is overwritten: `AppKit` installs its own handler
/// for this event while starting up, and its handler is the one that refuses
/// the document. Registering **after** `eframe`'s creation closure is too late
/// for the launch itself: a document double-clicked into a running window
/// arrived and the one that started the process did not, because `AppKit` had
/// already dispatched and refused it. Between them is
/// `applicationWillFinishLaunching:`, which is where Apple's own documentation
/// says to install Apple Event handlers, and it is right.
pub fn watch() {
    let block = RcBlock::new(|_notification: NonNull<NSNotification>| {
        register();
    });

    // SAFETY: the name is `AppKit`'s own constant, no object filters the
    // notification, and `None` for the queue means the block runs on the
    // thread that posted it, which is the main thread. The block outlives the
    // call because `RcBlock` is copied by the observer.
    unsafe {
        let observer = NSNotificationCenter::defaultCenter()
            .addObserverForName_object_queue_usingBlock(
                Some(NSApplicationWillFinishLaunchingNotification),
                None,
                None,
                &block,
            );
        // The observer is the process's for as long as the process lasts, and
        // releasing it would unregister the only thing that installs the
        // handler.
        std::mem::forget(observer);
    }
}

/// Put the handler on, at the moment `watch` arranged for.
fn register() {
    let manager = NSAppleEventManager::sharedAppleEventManager();
    let handler: Retained<Handler> = unsafe { msg_send![Handler::alloc(), init] };

    // SAFETY: the handler is the class defined above, and the selector is the
    // method defined on it, whose signature is the one this call promises.
    unsafe {
        let object: &AnyObject = &handler;
        manager.setEventHandler_andSelector_forEventClass_andEventID(
            object,
            sel!(handleOpenDocuments:withReplyEvent:),
            CORE_EVENT_CLASS,
            OPEN_DOCUMENTS,
        );
    }

    // The Apple Event manager does not retain a handler, so dropping this one
    // would leave it holding a dangling pointer and the first double-clicked
    // document would be the last thing this process did. It lives as long as
    // the process, which is exactly how long the registration lasts.
    std::mem::forget(handler);
}

/// How to wake the window, once there is a window to wake.
///
/// Separate from `watch` because the handler has to be installed before
/// `eframe` runs and the context does not exist until after. A document that
/// arrives in between is not lost: it waits in `ARRIVED`, and the first frame
/// draws anyway.
pub fn wake_with(context: &egui::Context) {
    let _ = WAKE.set(context.clone());
}

/// The document macOS asked for since this was last called, if any.
///
/// Taken rather than read, so that one event opens one document and the window
/// does not reopen it on every frame after.
#[must_use]
pub fn taken() -> Option<PathBuf> {
    ARRIVED.lock().ok()?.take()
}
