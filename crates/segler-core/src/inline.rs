//! Text with its formatting shown as tags, for an edit field.
//!
//! DocLang puts inline formatting in child elements, so a paragraph reading
//! `The <bold>western approaches</bold> are wide` is five sibling nodes and
//! not one string. An edit field holds one string, and the question this
//! module answers is which string.
//!
//! **A formatting element is exactly its tag name.** `bold`, `italic`,
//! `underline`, `strikethrough`, `superscript`, `subscript`, `handwriting`
//! and `rtl` take no attributes — they fall through to the empty case in the
//! validator's attribute table — and carry no element head, unlike `marker`
//! beside them. So spelling one as `<bold>…</bold>` in a text field loses
//! nothing at all, and reading it back is exact rather than a guess. That is
//! what makes this a better answer than matching the words up again
//! afterwards, which is what `session::rewrite` does for everything this
//! cannot represent.
//!
//! What this deliberately does not cover, and why each falls back:
//!
//! - **CDATA**, because a CDATA section is a spelling as well as a value and
//!   `session::retext` already keeps it; escaping it into `&lt;` here would
//!   quietly rewrite the file's shape.
//! - **Comments and processing instructions**, which have no inline form.
//! - **`content`**, whose entire purpose is whitespace kept exactly as
//!   written, which is the one thing an edit field is careless with.
//! - **Any other element** — an `xref`, a nested `text` in a table cell —
//!   because those carry attributes and structure that a tag name does not.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use crate::doclang::{self, Category};
use crate::tree::{Document, Node};

/// The synthetic root a fragment is parsed inside. Never a name the person
/// typed, and never a name reported back to them.
const ROOT: &str = "inline";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("{0}")]
    Syntax(String),
    /// A tag left open, named. Its own variant because the wrapped parse
    /// cannot report it: the closing tag of the synthetic root is matched
    /// against whatever the person left open, so the parser pops that and
    /// reports the root as unclosed - naming an element nobody typed.
    #[error("<{0}> is never closed")]
    Unclosed(String),
    #[error("<{0}> is not inline formatting; only bold, italic, underline, strikethrough, superscript, subscript, handwriting and rtl can be typed here")]
    NotFormatting(String),
    #[error("<{0}> cannot carry attributes")]
    Attributes(String),
}

/// Whether these nodes can be spelled as tags and read back exactly.
///
/// Text and formatting elements, all the way down, and nothing else.
pub fn is_inline(nodes: &[Node]) -> bool {
    nodes.iter().all(|n| match n {
        Node::Text(_) => true,
        Node::Element(e) => {
            doclang::kind(e).is_some_and(|k| k.category() == Category::Formatting)
                && e.attrs().is_empty()
                && is_inline(e.children())
        }
        Node::CData(_) | Node::Comment(_) | Node::Pi { .. } => false,
    })
}

/// Whether an edit field for these nodes should show tags at all.
///
/// Only where there is formatting to show. A plain paragraph edits as plain
/// text, exactly as it did before this module existed, and that is the point:
/// putting `&lt;` in front of somebody correcting a scan of ordinary prose
/// would be a regression paid by everyone to serve the few lines that carry a
/// tag.
pub fn shows_tags(nodes: &[Node]) -> bool {
    is_inline(nodes) && nodes.iter().any(|n| matches!(n, Node::Element(_)))
}

/// These nodes as inline markup.
pub fn to_inline(nodes: &[Node]) -> String {
    let mut out = String::new();
    write_inline(nodes, &mut out);
    out
}

fn write_inline(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match node {
            Node::Text(t) => escape(t.value(), out),
            Node::Element(e) => {
                out.push('<');
                out.push_str(e.name());
                out.push('>');
                write_inline(e.children(), out);
                out.push_str("</");
                out.push_str(e.name());
                out.push('>');
            }
            // `is_inline` refused these, so this is unreachable for anything
            // this module offered a field for. Written out rather than
            // panicked on: a caller that skipped the check gets its text
            // through with the node dropped, not a crash in an editor.
            Node::CData(s) => escape(s, out),
            Node::Comment(_) | Node::Pi { .. } => {}
        }
    }
}

/// `&` and `<` only. `>` needs no escape outside a CDATA close, and leaving
/// it alone keeps an arrow in a sentence looking like an arrow.
fn escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            c => out.push(c),
        }
    }
}

/// Inline markup back to nodes.
///
/// The document parser rather than a second one written here: it decodes the
/// five entities and character references, reports a position when the markup
/// is malformed, and refuses a document type declaration — and being the same
/// parser the rest of the application uses means an edit field and a file
/// cannot disagree about what a string means.
pub fn from_inline(text: &str) -> Result<Vec<Node>, Error> {
    // A synthetic root, whose name cannot collide with anything typed, since
    // a name the vocabulary knows would be accepted as formatting below.
    let wrapped = format!("<{ROOT}>{text}</{ROOT}>");
    let doc = Document::parse(&wrapped).map_err(|e| {
        // An unclosed tag is named before the parser's own message is used,
        // because that message would name the wrapper. Measured by typing
        // `The <bold>western approaches are wide.` into the window and being
        // told that `<inline>` is never closed.
        match unclosed(&wrapped) {
            Some(name) => Error::Unclosed(name),
            None => Error::Syntax(e.to_string()),
        }
    })?;
    let nodes = doc.root.children().to_vec();
    check(&nodes)?;
    Ok(nodes)
}

/// The innermost tag left open in `wrapped`, if any.
///
/// Takes the wrapped string rather than what was typed, because a fragment
/// beginning with a word is not well-formed XML on its own and the tokenizer
/// stops at the first character. The wrapper is on the stack throughout and
/// falls out of the answer: the mismatch is always found against a tag inside
/// it, since it is the thing whose closing tag does not match.
///
/// Only for the message: the parse is what decides whether the markup is
/// good, and this is asked only once it has refused.
fn unclosed(wrapped: &str) -> Option<String> {
    let mut stack: Vec<String> = Vec::new();
    for token in xmlparser::Tokenizer::from(wrapped) {
        match token {
            Ok(xmlparser::Token::ElementStart { local, .. }) => stack.push(local.to_string()),
            Ok(xmlparser::Token::ElementEnd { end, .. }) => match end {
                // `<a>`: it stays open.
                xmlparser::ElementEnd::Open => {}
                // `<a/>`: it does not.
                xmlparser::ElementEnd::Empty => {
                    stack.pop();
                }
                // `</a>`: it does, if `a` is what is open. Where it is not,
                // whatever is open is the tag that was left that way, and
                // that is the name worth printing.
                xmlparser::ElementEnd::Close(_, local) => match stack.last() {
                    Some(top) if top.as_str() == local.as_str() => {
                        stack.pop();
                    }
                    Some(top) => return Some(top.clone()),
                    None => return None,
                },
            },
            Ok(_) => {}
            // Whatever is wrong is past this point, and what is on the stack
            // now is as much as can honestly be said about it.
            Err(_) => break,
        }
    }
    // The wrapper is the first thing on this stack and is never the answer:
    // it is closed in the string this function was given, so finding it here
    // means the tokenizer stopped early on something else - a bare `<`, say -
    // and there is no unclosed tag to name.
    let name = stack.pop()?;
    (!(stack.is_empty() && name == ROOT)).then_some(name)
}

fn check(nodes: &[Node]) -> Result<(), Error> {
    for node in nodes {
        match node {
            Node::Text(_) => {}
            Node::Element(e) => {
                let formatting =
                    doclang::kind(e).is_some_and(|k| k.category() == Category::Formatting);
                if !formatting {
                    return Err(Error::NotFormatting(e.name().to_owned()));
                }
                if !e.attrs().is_empty() {
                    return Err(Error::Attributes(e.name().to_owned()));
                }
                check(e.children())?;
            }
            Node::CData(_) => return Err(Error::NotFormatting("![CDATA[".to_owned())),
            Node::Comment(_) => return Err(Error::NotFormatting("!--".to_owned())),
            Node::Pi { target, .. } => return Err(Error::NotFormatting(format!("?{target}"))),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(markup: &str) -> Vec<Node> {
        let doc = Document::parse(&format!("<text>{markup}</text>")).unwrap();
        doc.root.children().to_vec()
    }

    #[test]
    fn a_formatted_body_round_trips_exactly() {
        for markup in [
            "The <bold>western approaches</bold> are wide",
            "Plain <bold>bold <italic>both</italic></bold> and more",
            "<italic>all of it</italic>",
            "a &amp; b &lt; c",
            "<superscript>2</superscript> and <subscript>n</subscript>",
        ] {
            let nodes = body(markup);
            assert!(is_inline(&nodes), "{markup}");
            let spelled = to_inline(&nodes);
            assert_eq!(spelled, markup, "spelling {markup}");
            let back = from_inline(&spelled).unwrap();
            assert_eq!(to_inline(&back), markup, "reading back {markup}");
        }
    }

    #[test]
    fn a_literal_angle_bracket_survives_the_round_trip() {
        let nodes = from_inline("a &lt; b &amp; c").unwrap();
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            Node::Text(t) => assert_eq!(t.value(), "a < b & c"),
            other => panic!("{other:?}"),
        }
        assert_eq!(to_inline(&nodes), "a &lt; b &amp; c");
    }

    /// Only where there is something to show. A plain paragraph keeps its
    /// plain field.
    #[test]
    fn tags_are_shown_only_for_a_body_that_has_them() {
        assert!(!shows_tags(&body("just words")));
        assert!(shows_tags(&body("some <bold>words</bold>")));
        assert!(!shows_tags(&body("")));
    }

    #[test]
    fn what_cannot_be_spelled_inline_says_so() {
        assert!(!is_inline(&body("<![CDATA[raw]]>")));
        assert!(!is_inline(&body("before<!-- why -->after")));
        assert!(!is_inline(&body("a <content> b </content>")));
        assert!(!is_inline(&body("see <xref thread_id=\"1\"/>")));
    }

    #[test]
    fn bad_markup_is_refused_rather_than_written() {
        // Named, and named as the tag that was typed rather than as the
        // wrapper this module parses inside.
        assert!(matches!(
            from_inline("an <bold>unclosed tag"),
            Err(Error::Unclosed(name)) if name == "bold"
        ));
        assert!(matches!(
            from_inline("<bold>a <italic>nested one</bold>"),
            Err(Error::Unclosed(name)) if name == "italic"
        ));
        assert_eq!(
            from_inline("an <bold>unclosed tag")
                .unwrap_err()
                .to_string(),
            "<bold> is never closed"
        );
        assert!(matches!(
            from_inline("a <blink>tag we do not have</blink>"),
            Err(Error::NotFormatting(name)) if name == "blink"
        ));
        assert!(matches!(
            from_inline("<bold class=\"x\">no</bold>"),
            Err(Error::Attributes(name)) if name == "bold"
        ));
        assert!(matches!(
            from_inline("a bare < bracket"),
            Err(Error::Syntax(_))
        ));
        // A non-formatting element of the vocabulary is refused by name, so
        // the message says which one rather than "not well-formed".
        assert!(matches!(
            from_inline("see <xref thread_id=\"1\"/>"),
            Err(Error::NotFormatting(name)) if name == "xref"
        ));
    }
}
