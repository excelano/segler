//! Segler's core: the DocLang document model, its parser and serializer,
//! validation, and the `.dclx` archive format.
//!
//! Nothing here draws a window or prints to a terminal. The desktop
//! application and the command-line tool are both renderers over this crate,
//! and anything that is not pixels or text on a terminal belongs here.
//!
//! The specification at <https://github.com/doclang-project/doclang> is the
//! authority on the format. This crate implements it and does not restate it;
//! where a doc comment cites a rule, the rule is the spec's.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]

pub mod archive;
pub mod summary;

/// The DocLang namespace. Documents may omit it, and the spec's own validator
/// has a switch for injecting it, so a missing namespace is not an error here.
pub const NAMESPACE: &str = "https://www.doclang.ai/ns/v0";

/// The specification version this crate targets, in the `MAJOR.MINOR` form
/// the `version` attribute of the root element carries. Every 0.x minor is a
/// breaking change by the spec's own rule, so this is exact rather than a floor.
pub const SPEC_VERSION: &str = "0.7";
