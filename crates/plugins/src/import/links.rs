//! A description's links and pictures: a pin link stays, a web link stays only on one of the plugin's link
//! hosts, any other shows as its words; pictures show as their alt text, HTML is dropped. Parsed as GFM, as the
//! card reads it, so reference links and bare URLs are caught too.

use std::ops::Range;

use markdown::{ParseOptions, mdast::Node};
use model::PIN_LINK;

use crate::scope::Scope;

// A rewrite can make a new link (a label holding a bare URL): parsed again until nothing changes.
const PASSES: usize = 4;

pub fn filter(text: &str, scope: &Scope) -> String {
    // Most descriptions have no link at all: no parse.
    if !["[", "<", "://", "www."].iter().any(|mark| text.contains(mark)) {
        return text.to_owned();
    }
    let mut text = text.to_owned();
    for _ in 0..PASSES {
        let Ok(tree) = markdown::to_mdast(&text, &ParseOptions::gfm()) else { return String::new() };
        let edits = edits(&text, &tree, &|url: &str| url.starts_with(PIN_LINK) || scope.opens(url));
        if edits.is_empty() {
            return text;
        }
        for (range, with) in edits.into_iter().rev() {
            text.replace_range(range, &with);
        }
    }
    // Still changing: plain words are safe.
    text.replace(['[', ']', '<', '>'], "")
}

/// What to replace, in order and not overlapping.
fn edits(text: &str, tree: &Node, opens: &dyn Fn(&str) -> bool) -> Vec<(Range<usize>, String)> {
    let mut refused = Vec::new();
    walk(tree, &mut |node| {
        if let Node::Definition(definition) = node
            && !opens(&definition.url)
        {
            refused.push(definition.identifier.clone());
        }
    });
    let mut edits = Vec::new();
    walk(tree, &mut |node| {
        let Some(range) = span(node) else { return };
        let edit = match node {
            Node::Link(link) if !opens(&link.url) => Some(words(text, node.children(), &link.url)),
            Node::LinkReference(link) if refused.contains(&link.identifier) => Some(words(text, node.children(), "")),
            Node::Definition(definition) if refused.contains(&definition.identifier) => Some(String::new()),
            Node::Image(image) => Some(image.alt.clone()),
            Node::ImageReference(image) => Some(image.alt.clone()),
            Node::Html(_) => Some(String::new()),
            _ => None,
        };
        edits.extend(edit.map(|with| (range, with)));
    });
    edits.sort_by_key(|(range, _)| range.start);
    let mut end = 0;
    edits.retain(|(range, _)| {
        let outside = range.start >= end;
        end = end.max(range.end);
        outside
    });
    edits
}

/// A link's words as written; a bare or angle-bracketed URL as code, so it is not a link again.
fn words(text: &str, children: Option<&Vec<Node>>, url: &str) -> String {
    let spans = children.into_iter().flatten().filter_map(span);
    let label = spans.reduce(|a, b| a.start.min(b.start)..a.end.max(b.end)).map_or("", |range| &text[range]);
    if label == url || label.is_empty() { format!("`{}`", url.replace('`', "")) } else { label.to_owned() }
}

fn span(node: &Node) -> Option<Range<usize>> {
    node.position().map(|at| at.start.offset..at.end.offset)
}

fn walk(node: &Node, visit: &mut dyn FnMut(&Node)) {
    visit(node);
    for child in node.children().into_iter().flatten() {
        walk(child, visit);
    }
}

#[cfg(test)]
mod tests {
    use model::SourceId;

    use super::*;
    use crate::scope::Hosts;

    fn scope() -> Scope {
        Scope { source: SourceId::new("plugin-t").unwrap(), hosts: Hosts::default(), links: Hosts::parse(&["wiki.io".to_owned()]).unwrap() }
    }

    #[test]
    fn only_pin_links_and_links_to_a_link_host_stay() {
        let text = "See [the gate](pin:p2), [wiki](https://wiki.io/a) and [this](https://evil.io/x).";
        assert_eq!(filter(text, &scope()), "See [the gate](pin:p2), [wiki](https://wiki.io/a) and this.");
    }

    #[test]
    fn bare_and_reference_links_and_pictures_lose_their_reach() {
        let scope = scope();
        assert_eq!(filter("Go to https://evil.io/x now", &scope), "Go to `https://evil.io/x` now");
        assert_eq!(filter("A [shop][s] here.\n\n[s]: https://evil.io/s", &scope).trim_end(), "A shop here.");
        assert_eq!(filter("![a chest](https://evil.io/c.png) <b>x</b>", &scope), "a chest x");
        assert_eq!(filter("[see https://evil.io](https://evil.io/y)", &scope), "see `https://evil.io`");
        assert_eq!(filter("No links here.", &scope), "No links here.");
    }
}
