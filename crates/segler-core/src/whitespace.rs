//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use crate::doclang::{self, Kind};
use crate::tree::{Element, Node, Text};

pub fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

pub fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_space = false;
    for c in text.chars() {
        if is_space(c) {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(c);
            in_space = false;
        }
    }
    out
}

pub fn read(nodes: &[Node]) -> String {
    let mut reader = Reader::default();
    reader.nodes(nodes);
    reader.out
}

#[derive(Default)]
struct Reader {
    out: String,
    pending_space: bool,
}

impl Reader {
    fn nodes(&mut self, nodes: &[Node]) {
        for node in nodes {
            match node {
                Node::Text(t) => self.raw(t.value()),
                Node::CData(s) => self.raw(s),
                Node::Comment(_) | Node::Pi { .. } => {}
                Node::Element(e) => match doclang::kind(e) {
                    Some(Kind::Content) => {
                        self.flush();
                        self.out.push_str(&e.text());
                    }
                    Some(k) if k.is_property() => {}
                    _ => self.nodes(e.children()),
                },
            }
        }
    }

    fn raw(&mut self, text: &str) {
        for c in text.chars() {
            if is_space(c) {
                self.pending_space = !self.out.is_empty();
            } else {
                self.flush();
                self.out.push(c);
            }
        }
    }

    fn flush(&mut self) {
        if self.pending_space {
            self.out.push(' ');
            self.pending_space = false;
        }
    }
}

pub fn encode(text: &str, whole: bool) -> Vec<Node> {
    encode_between(text, whole, true, true)
}

pub fn encode_between(text: &str, whole: bool, block_start: bool, block_end: bool) -> Vec<Node> {
    if whole {
        return if read(&[raw(text)]) == text {
            vec![raw(text)]
        } else {
            vec![content(text)]
        };
    }
    let mut nodes = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        let word_end = rest.find(is_space).unwrap_or(rest.len());
        plain.push_str(&rest[..word_end]);
        rest = &rest[word_end..];
        let space_end = rest.find(|c| !is_space(c)).unwrap_or(rest.len());
        if space_end == 0 {
            continue;
        }
        let space = &rest[..space_end];
        let at_start = block_start && plain.is_empty() && nodes.is_empty();
        let at_end = block_end && space_end == rest.len();
        if space == " " && !at_start && !at_end {
            plain.push(' ');
        } else {
            if !plain.is_empty() {
                nodes.push(raw(&std::mem::take(&mut plain)));
            }
            nodes.push(content(space));
        }
        rest = &rest[space_end..];
    }
    if !plain.is_empty() {
        nodes.push(raw(&plain));
    }
    nodes
}

fn raw(text: &str) -> Node {
    Node::Text(Text::new(text))
}

fn content(text: &str) -> Node {
    let mut el = Element::new(Kind::Content.name());
    el.push(raw(text));
    Node::Element(el)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Document;

    fn body(xml: &str) -> Vec<Node> {
        Document::parse(&format!("<text>{xml}</text>"))
            .unwrap()
            .root
            .children()
            .to_vec()
    }

    fn markup(nodes: &[Node]) -> String {
        let mut el = Element::new("text");
        el.replace_children(nodes.to_vec());
        let mut out = String::new();
        el.write_to(&mut out);
        out
    }

    #[test]
    fn reading_follows_the_spec_example() {
        let nodes = body("\n  Water (H<subscript>2</subscript>O) is <bold>clear </bold>\n  <italic> and odorless</italic>.\n  This keeps <content>two  spaces</content> as written.\n");
        assert_eq!(
            read(&nodes),
            "Water (H2O) is clear and odorless. This keeps two  spaces as written."
        );
    }

    #[test]
    fn heads_are_not_text_and_a_no_break_space_is() {
        let nodes = body("<location value=\"1\"/>a\u{a0}\u{a0}b");
        assert_eq!(read(&nodes), "a\u{a0}\u{a0}b");
    }

    #[test]
    fn single_spaces_stay_raw_and_the_rest_goes_in_content() {
        assert_eq!(markup(&encode("one two", false)), "<text>one two</text>");
        assert_eq!(
            markup(&encode(" lead  two\nlines ", false)),
            "<text><content> </content>lead<content>  </content>two<content>\n</content>lines<content> </content></text>"
        );
        assert_eq!(markup(&encode("", false)), "<text/>");
    }

    #[test]
    fn a_single_space_inside_the_block_stays_raw_at_a_node_edge() {
        assert_eq!(
            markup(&encode_between("The ", false, true, false)),
            "<text>The </text>"
        );
        assert_eq!(
            markup(&encode_between(" are wide", false, false, true)),
            "<text> are wide</text>"
        );
        assert_eq!(
            markup(&encode_between("  are", false, false, true)),
            "<text><content>  </content>are</text>"
        );
    }

    #[test]
    fn whole_wraps_once_or_not_at_all() {
        assert_eq!(markup(&encode("x = 1", true)), "<text>x = 1</text>");
        assert_eq!(
            markup(&encode("fn a() {\n    b();\n}", true)),
            "<text><content>fn a() {\n    b();\n}</content></text>"
        );
    }

    #[test]
    fn what_is_encoded_reads_back_as_typed() {
        for typed in [
            "plain words",
            " leading",
            "trailing ",
            "two  spaces",
            "a\nline break",
            "tab\there",
            "\n",
            "  ",
        ] {
            assert_eq!(read(&encode(typed, false)), typed, "{typed:?}");
            assert_eq!(read(&encode(typed, true)), typed, "{typed:?} whole");
        }
    }
}
