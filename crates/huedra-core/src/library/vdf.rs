//! Valve KeyValues text, the format of Steam's `libraryfolders.vdf` and
//! `appmanifest_*.acf`.

/// A KeyValues node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A quoted or bare string.
    Text(String),
    /// A block of key-value pairs, in file order.
    Map(Vec<(String, Value)>),
}

const MAX_DEPTH: usize = 32;

impl Value {
    /// The first entry with this key, compared without regard to case.
    pub fn get(&self, key: &str) -> Option<&Self> {
        self.entries()
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    }

    /// The text of the first entry with this key.
    pub fn text(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Self::Text(text) => Some(text),
            Self::Map(_) => None,
        }
    }

    /// The entries of a block; empty for text.
    pub fn entries(&self) -> &[(String, Self)] {
        match self {
            Self::Map(entries) => entries,
            Self::Text(_) => &[],
        }
    }
}

/// Parses a whole file into a block of its top-level entries. Returns `None`
/// for malformed text.
pub fn parse(text: &str) -> Option<Value> {
    let mut tokens = Tokens {
        chars: text.chars().peekable(),
    };
    let entries = parse_block(&mut tokens, 0, false)?;
    Some(Value::Map(entries))
}

fn parse_block(
    tokens: &mut Tokens<'_>,
    depth: usize,
    nested: bool,
) -> Option<Vec<(String, Value)>> {
    if depth > MAX_DEPTH {
        return None;
    }
    let mut entries = Vec::new();
    loop {
        let key = match tokens.next() {
            None if !nested => return Some(entries),
            Some(Token::Close) if nested => return Some(entries),
            Some(Token::Text(key)) => key,
            _ => return None,
        };
        let value = match tokens.next()? {
            Token::Open => Value::Map(parse_block(tokens, depth + 1, true)?),
            Token::Text(text) => Value::Text(text),
            Token::Close => return None,
        };
        entries.push((key, value));
        tokens.skip_condition();
    }
}

enum Token {
    Open,
    Close,
    Text(String),
}

struct Tokens<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl Tokens<'_> {
    fn skip_space(&mut self) {
        loop {
            match self.chars.peek() {
                Some(c) if c.is_whitespace() => {
                    self.chars.next();
                }
                Some('/') => {
                    let mut ahead = self.chars.clone();
                    ahead.next();
                    if ahead.peek() != Some(&'/') {
                        return;
                    }
                    for c in self.chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                _ => return,
            }
        }
    }

    /// Skips a platform condition such as `[$WIN32]` after a value.
    fn skip_condition(&mut self) {
        self.skip_space();
        if self.chars.peek() == Some(&'[') {
            for c in self.chars.by_ref() {
                if c == ']' {
                    break;
                }
            }
        }
    }

    fn next(&mut self) -> Option<Token> {
        self.skip_space();
        match self.chars.next()? {
            '{' => Some(Token::Open),
            '}' => Some(Token::Close),
            '"' => {
                let mut text = String::new();
                while let Some(c) = self.chars.next() {
                    match c {
                        '"' => return Some(Token::Text(text)),
                        '\\' => match self.chars.next()? {
                            'n' => text.push('\n'),
                            't' => text.push('\t'),
                            other => text.push(other),
                        },
                        other => text.push(other),
                    }
                }
                None
            }
            first => {
                let mut text = String::from(first);
                while let Some(&c) = self.chars.peek() {
                    if c.is_whitespace() || matches!(c, '{' | '}' | '"') {
                        break;
                    }
                    text.push(c);
                    self.chars.next();
                }
                Some(Token::Text(text))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_blocks_and_escapes() {
        let text = r#"
            // comment
            "libraryfolders"
            {
                "0"
                {
                    "path"    "C:\\Program Files (x86)\\Steam"
                    "apps" { "228980" "1" }
                }
                bare value [$WIN32]
            }
        "#;
        let root = parse(text).expect("parse");
        let folders = root.get("LibraryFolders").expect("case-insensitive");
        let first = folders.get("0").expect("first library");
        assert_eq!(
            first.text("path"),
            Some(r"C:\Program Files (x86)\Steam"),
            "escapes"
        );
        assert_eq!(folders.text("bare"), Some("value"), "bare tokens");
        assert!(first.get("apps").is_some(), "nested block");
    }

    #[test]
    fn rejects_malformed() {
        assert!(parse(r#""a" { "b" "c" "#).is_none(), "unclosed block");
        assert!(parse(r#"}"#).is_none(), "stray brace");
        let deep = "\"a\" {".repeat(100);
        assert!(parse(&deep).is_none(), "depth limit");
    }
}
