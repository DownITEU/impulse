//! No literal caption in `shell`: a text the user reads comes from a catalog.

use std::{fs, str::FromStr};

use proc_macro2::{Delimiter, TokenStream, TokenTree};

use crate::{files, root, verdict};

/// Calls whose string arguments the user never reads.
const QUIET: &[&str] = &[
    "Key",
    "id",
    "expect",
    "context",
    "with_context",
    "trace",
    "debug",
    "info",
    "warn",
    "error",
    "anyhow",
    "bail",
    "include_str",
    "include_bytes",
    "font",
    "assert",
    "assert_eq",
    "debug_assert",
    "unreachable",
    "panic",
    "var",
    "var_os",
    "env",
];
/// Calls that show every string they take: an id-shaped literal given to one is a caption.
const SINKS: &[&str] =
    &["child", "tracked", "mono", "button", "flash_button", "segment", "badge", "tip", "notice", "placeholder", "row", "label"];
/// Names, not words: fonts, key contexts, the saved accent ids, the brand.
const NAMES: &[&str] = &[
    "Chakra Petch",
    // The release list's query, joined to a repository address.
    "/releases?per_page=30",
    // The bug form's address: its `version` field arrives filled in.
    "{REPO}/issues/new?template=bug_report.yml&version={VERSION}",
    "Exo 2",
    "Microsoft YaHei UI",
    "JetBrains Mono",
    "Dialog",
    "Conflict",
    "MapView",
    "MapStage",
    "LIME",
    "AMBER",
    "CYAN",
    "WHITE",
    "Impulse",
    "IMPULSE",
    // The brand before the version in the status bar.
    "IMPULSE ",
];

/// A string literal outside tests and attributes: its line and the call it is an argument of.
struct Literal {
    line: usize,
    text: String,
    call: Option<String>,
}

fn literals(source: &str) -> Vec<Literal> {
    let mut found = Vec::new();
    walk(TokenStream::from_str(source).expect("the file parses"), None, &mut found);
    found
}

/// `call`: the call or macro these tokens are arguments of; a block, a tuple or an array keeps its parent's.
fn walk(tokens: TokenStream, call: Option<&str>, found: &mut Vec<Literal>) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    let punct = |at: usize, char| matches!(tokens.get(at), Some(TokenTree::Punct(punct)) if punct.as_char() == char);
    let mut i = 0;
    while i < tokens.len() {
        // An attribute (`#[..]`, `#![..]`) holds no caption; a test one hides the item after it, up to its block or the
        // `;` or `,` that ends it.
        let inner = usize::from(punct(i + 1, '!'));
        if let (true, Some(TokenTree::Group(attribute))) = (punct(i, '#'), tokens.get(i + 1 + inner)) {
            let body = attribute.stream().to_string().replace(' ', "");
            i += 2 + inner;
            if body == "test" || body.starts_with("cfg(test") {
                while let Some(token) = tokens.get(i) {
                    i += 1;
                    match token {
                        TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => break,
                        TokenTree::Punct(punct) if matches!(punct.as_char(), ';' | ',') => break,
                        _ => {}
                    }
                }
            }
            continue;
        }
        match &tokens[i] {
            TokenTree::Group(group) => {
                let bang = i > 0 && punct(i - 1, '!');
                let named = match i.checked_sub(1 + usize::from(bang)).map(|at| &tokens[at]) {
                    Some(TokenTree::Ident(ident)) if bang || group.delimiter() == Delimiter::Parenthesis => Some(ident.to_string()),
                    _ => None,
                };
                walk(group.stream(), named.as_deref().or(call), found);
            }
            TokenTree::Literal(literal) => {
                if let Some(text) = text(&literal.to_string()) {
                    found.push(Literal { line: literal.span().start().line, text, call: call.map(str::to_owned) });
                }
            }
            _ => {}
        }
        i += 1;
    }
}

/// A string literal's text as written; `None` for bytes, chars and numbers.
fn text(written: &str) -> Option<String> {
    if let Some(quoted) = written.strip_prefix('"') {
        return quoted.strip_suffix('"').map(str::to_owned);
    }
    let raw = written.strip_prefix('r')?;
    let fence = raw.len() - raw.trim_start_matches('#').len();
    raw.get(fence..raw.len() - fence)?.strip_prefix('"')?.strip_suffix('"').map(str::to_owned)
}

/// A literal the user would read: a word that is not a name, markup, path data or a time format, or an id kept off the screen.
fn caption(literal: &Literal) -> bool {
    let (mut depth, mut escaped) = (0, false);
    let words: String = literal
        .text
        .chars()
        .filter(|&c| {
            depth += i32::from(c == '{');
            let outside = depth == 0 && !escaped && c != '\\';
            depth -= i32::from(c == '}' && depth > 0);
            escaped = c == '\\' && !escaped;
            outside && c != '}'
        })
        .collect();
    let text = literal.text.as_str();
    let path = text.starts_with('M') && text[1..].starts_with(|c: char| c.is_ascii_digit());
    let call = literal.call.as_deref().unwrap_or("");
    let id = words.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b':' | b'/' | b'-'));
    // A web or mail address, or a percent-encoded one, written without a space.
    let escape = text.as_bytes().windows(3).any(|at| at[0] == b'%' && at[1].is_ascii_hexdigit() && at[2].is_ascii_hexdigit());
    let address = !text.contains(char::is_whitespace) && (text.contains("://") || (text.contains('@') && text.contains('.')) || escape);
    words.chars().any(char::is_alphabetic)
        && !address
        && !text.starts_with('<')
        && !text.starts_with('%')
        && !path
        && !QUIET.contains(&call)
        && !NAMES.contains(&text)
        && !(id && !SINKS.contains(&call))
}

#[test]
fn shell_has_no_literal_caption() {
    let mut complaints = Vec::new();
    let ours = |path: &&String| path.starts_with("crates/shell/src/") && path.ends_with(".rs") && !path.ends_with("/tests.rs");
    for path in files().iter().filter(ours) {
        let source = fs::read_to_string(root().join(path)).unwrap();
        let found = literals(&source).into_iter().filter(caption);
        complaints.extend(found.map(|literal| format!("{path}:{}: {:?}", literal.line, literal.text)));
    }
    verdict(complaints);
}

#[test]
fn captions_are_told_from_names() {
    let source = r##"
        fn f() { div().id("row-1").child("SAVE").child(format!("{n:02} MODS")).placeholder("name").font_family("x").child("100%OFF"); badge("alpha", 1); }
        fn g() { tracing::warn!("no file: {e}"); x.context("reading the file")?; let a = '"'; let s = r#"<svg/>"#; open("https://a.b/C", "me@a.dev", env!("CARGO_PKG_VERSION")); }
        fn h<'a>(x: &'a str) { if open { "OPEN" } else { "shut" }; vec!["ROW"]; b"BYTES \" B"; format!("{}\n{}"); "M1 2z"; }
        // child("COMMENT")
        /* child("BLOCK") */
        #[cfg(test)]
        mod tests { fn t() { child("TEST") } }
        #[derive(Clone)] struct S; fn k() { child("LAST") }
        fn n() { x.child(if a { "on" } else { "off" }); }
        #[expect(clippy::x, reason = "a ] b")] fn p() { x.child("REAL"); }
        struct T { #[cfg(test)] a: u8, b: u8 } fn q() { x.child("MORE"); }
    "##;
    let shown: Vec<_> = literals(source).into_iter().filter(caption).map(|literal| (literal.line, literal.text)).collect();
    let expected = [(2, "SAVE"), (2, "{n:02} MODS"), (2, "name"), (2, "100%OFF"), (2, "alpha"), (4, "OPEN"), (4, "ROW"), (9, "LAST")];
    let expected = expected.into_iter().chain([(10, "on"), (10, "off"), (11, "REAL"), (12, "MORE")]);
    assert_eq!(shown, expected.map(|(line, text)| (line, text.to_owned())).collect::<Vec<_>>());
}

/// Attributes the user reads or hears.
const SPOKEN: [&str; 5] = [" aria-label=\"", " title=\"", " placeholder=\"", " alt=\"", " label=\""];

/// The panel's literal captions: JSX text, and spoken attributes written as strings. Text between a `>` and a
/// `<` or `{` with code marks in it, or that goes on from code (`: null}`), is code, not JSX; a lone `&` and an
/// apostrophe inside a word are prose.
fn panel_captions(source: &str) -> Vec<(usize, String)> {
    let line = |at: usize| source[..at].matches('\n').count() + 1;
    let worded = |text: &str| text.chars().any(char::is_alphabetic) && !NAMES.contains(&text);
    let mut found = Vec::new();
    for (at, _) in source.match_indices('>').filter(|(at, _)| !source[..*at].ends_with(['=', '-'])) {
        let rest = &source[at + 1..];
        let text = rest[..rest.find(['<', '{']).unwrap_or(rest.len())].trim_end();
        let start = at + 1 + text.len() - text.trim_start().len();
        let text = text.trim_start();
        let quoted = text
            .char_indices()
            .any(|(i, c)| c == '\'' && !(text[..i].ends_with(char::is_alphabetic) && text[i + 1..].starts_with(char::is_alphabetic)));
        let code = text.contains(['(', ')', ';', '=', '|', '[', ']', '}', '"', '`'])
            || text.contains("&&")
            || quoted
            || text.starts_with([',', ':', '?', '.']);
        if worded(text) && !code {
            found.push((line(start), text.to_owned()));
        }
    }
    for spoken in SPOKEN {
        for (at, _) in source.match_indices(spoken) {
            let value = source[at + spoken.len()..].split('"').next().unwrap_or_default();
            if worded(value) {
                found.push((line(at), value.to_owned()));
            }
        }
    }
    found
}

#[test]
fn the_panel_has_no_literal_caption() {
    let mut complaints = Vec::new();
    let ours = |path: &&String| path.starts_with("web/panel/src/") && path.ends_with(".tsx") && !path.ends_with(".test.tsx");
    for path in files().iter().filter(ours) {
        let source = fs::read_to_string(root().join(path)).unwrap();
        complaints.extend(panel_captions(&source).into_iter().map(|(line, text)| format!("{path}:{line}: {text:?}")));
    }
    verdict(complaints);
}

#[test]
fn panel_captions_are_told_from_code() {
    let source = r#"const a = useState<string | null>(null);
const f = (x: number) => x < 2 && y > 1;
<p className="[&>svg]:size-4">{t('ok')}</p>
<button aria-label="Close" title={t('close')}>
    Save mods
</button>
<span>IMPULSE</span><i alt="" />
<p>Save & close</p><p>Don't close</p>{open ? 'a' : 'b'}"#;
    let expected = [(5, "Save mods"), (8, "Save & close"), (8, "Don't close"), (4, "Close")];
    assert_eq!(panel_captions(source), expected.map(|(line, text)| (line, text.to_owned())));
}
