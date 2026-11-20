//! Valve text KeyValues (`.acf`, `libraryfolders.vdf`).

pub enum Node {
    Str(String),
    Map(Vec<(String, Node)>),
}

impl Node {
    /// Case-insensitive child lookup: Valve files mix `AppState` and `appstate`.
    pub fn get(&self, key: &str) -> Option<&Node> {
        self.entries().iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Node::Str(s) => Some(s),
            Node::Map(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Node)] {
        match self {
            Node::Map(entries) => entries,
            Node::Str(_) => &[],
        }
    }
}

pub fn parse(text: &str) -> Option<Node> {
    let mut lexer = Lexer { s: text, i: 0 };
    map(&mut lexer, false).map(Node::Map)
}

enum Token {
    Open,
    Close,
    Str(String),
}

struct Lexer<'a> {
    s: &'a str,
    i: usize,
}

impl Lexer<'_> {
    fn next(&mut self) -> Option<Token> {
        let rest = loop {
            let rest = self.s[self.i..].trim_start();
            self.i = self.s.len() - rest.len();
            if !rest.starts_with("//") {
                break rest;
            }
            self.i += rest.find('\n').unwrap_or(rest.len());
        };
        match rest.chars().next()? {
            '{' => {
                self.i += 1;
                Some(Token::Open)
            }
            '}' => {
                self.i += 1;
                Some(Token::Close)
            }
            '"' => {
                let mut out = String::new();
                let mut chars = rest[1..].char_indices();
                loop {
                    let (at, c) = chars.next()?;
                    match c {
                        '"' => {
                            self.i += at + 2;
                            return Some(Token::Str(out));
                        }
                        '\\' => out.push(match chars.next()?.1 {
                            'n' => '\n',
                            't' => '\t',
                            other => other,
                        }),
                        c => out.push(c),
                    }
                }
            }
            _ => {
                let end = rest.find(|c: char| c.is_whitespace() || matches!(c, '{' | '}' | '"')).unwrap_or(rest.len());
                self.i += end;
                Some(Token::Str(rest[..end].to_owned()))
            }
        }
    }
}

fn map(lexer: &mut Lexer, nested: bool) -> Option<Vec<(String, Node)>> {
    let mut out = Vec::new();
    loop {
        match lexer.next() {
            None => return (!nested).then_some(out),
            Some(Token::Close) => return nested.then_some(out),
            Some(Token::Open) => return None,
            Some(Token::Str(key)) => {
                let value = match lexer.next()? {
                    Token::Str(s) => Node::Str(s),
                    Token::Open => Node::Map(map(lexer, true)?),
                    Token::Close => return None,
                };
                out.push((key, value));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_and_library_folders() {
        let acf =
            "\"AppState\"\n{\n\t\"appid\"\t\t\"1203620\"\n\t\"StateFlags\"\t\"4\"\n\t// note\n\t\"installdir\"\t\t\"Enshrouded\"\n}\n";
        let root = parse(acf).unwrap();
        let state = root.get("appstate").unwrap();
        assert_eq!(state.str("APPID"), Some("1203620"));
        assert_eq!(state.str("installdir"), Some("Enshrouded"));

        let lf =
            r#""libraryfolders" { "0" { "path" "C:\\Program Files (x86)\\Steam" "apps" { "1203620" "123" } } "1" { "path" "D:\\Games" } }"#;
        let folders = parse(lf).unwrap();
        let folders = folders.get("libraryfolders").unwrap();
        let paths: Vec<_> = folders.entries().iter().filter_map(|(_, f)| f.str("path")).collect();
        assert_eq!(paths, [r"C:\Program Files (x86)\Steam", r"D:\Games"]);
    }

    #[test]
    fn rejects_unbalanced_input() {
        assert!(parse("\"a\" { \"b\" \"c\"").is_none());
        assert!(parse("\"a\" }").is_none());
    }
}
