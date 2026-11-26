//! A small Rust tokenizer for the source rules of `mm-check`: comments dropped, a literal or a
//! lifetime kept as one token, every other character its own token. It is not a parser: it finds
//! functions and item boundaries by counting brackets, which is enough for exact-copy detection
//! and pattern search.

/// One token with the line (from 1) where it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tok {
    pub text: String,
    pub line: usize,
}

/// A function found in a token stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Func {
    pub name: String,
    pub line: usize,
    /// Every token from `fn` to the closing brace of the body, without the name.
    pub tokens: Vec<String>,
}

/// Split `src` into tokens. Comments (line, block and doc) are dropped.
pub fn tokenize(src: &str) -> Vec<Tok> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    while i < c.len() {
        let at = |k: usize| c.get(k).copied().unwrap_or('\0');
        let (start, start_line) = (i, line);
        if c[i] == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c[i].is_whitespace() {
            i += 1;
            continue;
        }
        if c[i] == '/' && at(i + 1) == '/' {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c[i] == '/' && at(i + 1) == '*' {
            let mut depth = 0;
            while i < c.len() {
                if c[i] == '/' && at(i + 1) == '*' {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && at(i + 1) == '/' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    line += usize::from(c[i] == '\n');
                    i += 1;
                }
            }
            continue;
        }
        if is_word(c[i]) && !c[i].is_ascii_digit() {
            i = word_end(&c, i);
            let word: String = c[start..i].iter().collect();
            let quote = at(i);
            let hashes = c[i..].iter().take_while(|h| **h == '#').count();
            let raw = matches!(word.as_str(), "r" | "br" | "cr") && at(i + hashes) == '"';
            if raw || (matches!(word.as_str(), "b" | "c") && quote == '"') {
                i = string_end(&c, i, raw, &mut line);
            } else if word == "b" && quote == '\'' {
                i = char_end(&c, i);
            }
        } else if c[i].is_ascii_digit() {
            i = word_end(&c, i);
            if at(i) == '.' && at(i + 1).is_ascii_digit() {
                i = word_end(&c, i + 1);
            }
        } else if c[i] == '"' {
            i = string_end(&c, i, false, &mut line);
        } else if c[i] == '\'' {
            // `'x'` and `'\n'` are characters; `'a` is a lifetime.
            i = if at(i + 1) == '\\' || at(i + 2) == '\'' {
                char_end(&c, i)
            } else {
                word_end(&c, i + 1)
            };
        } else {
            i += 1;
        }
        out.push(Tok {
            text: c[start..i].iter().collect(),
            line: start_line,
        });
    }
    out
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn word_end(c: &[char], mut i: usize) -> usize {
    while i < c.len() && is_word(c[i]) {
        i += 1;
    }
    i
}

/// The index after the string whose prefix ends at `i` (`i` is at the quote or the first `#`).
fn string_end(c: &[char], mut i: usize, raw: bool, line: &mut usize) -> usize {
    let mut hashes = 0;
    while raw && c.get(i) == Some(&'#') {
        hashes += 1;
        i += 1;
    }
    i += 1; // the opening quote
    while i < c.len() {
        match c[i] {
            '\n' => *line += 1,
            '\\' if !raw => i += 1,
            '"' if c[i + 1..].iter().take_while(|h| **h == '#').count() >= hashes => {
                return i + 1 + hashes;
            }
            _ => {}
        }
        i += 1;
    }
    c.len()
}

/// The index after the character literal that starts at `i` (the opening quote).
fn char_end(c: &[char], mut i: usize) -> usize {
    i += 1;
    while i < c.len() {
        match c[i] {
            '\\' => i += 1,
            '\'' => return i + 1,
            _ => {}
        }
        i += 1;
    }
    c.len()
}

fn is(t: &Tok, s: &str) -> bool {
    t.text == s
}

/// The index of the bracket that closes the one at `open`, or the last token.
pub(super) fn close_of(toks: &[Tok], open: usize) -> usize {
    let mut depth = 0;
    for (k, t) in toks.iter().enumerate().skip(open) {
        match t.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth -= 1;
                if depth == 0 {
                    return k;
                }
            }
            _ => {}
        }
    }
    toks.len().saturating_sub(1)
}

/// The index after the item that starts at `from`: up to a `;` or to the closing brace of its block.
pub(super) fn item_end(toks: &[Tok], from: usize) -> usize {
    let mut k = from;
    while k < toks.len() {
        match toks[k].text.as_str() {
            ";" => return k + 1,
            "(" | "[" => k = close_of(toks, k),
            "{" => return close_of(toks, k) + 1,
            _ => {}
        }
        k += 1;
    }
    toks.len()
}

/// Whether the tokens inside `#[..]` mark test code: `test`, `cfg(test)` or `cfg(all(.., test, ..))`.
/// `cfg(any(test, ..))` and `cfg(not(test))` build outside tests too, so they are not.
fn is_test_attribute(body: &[&str]) -> bool {
    if body == ["test"] {
        return true;
    }
    let ["cfg", "(", inner @ .., ")"] = body else {
        return false;
    };
    if inner == ["test"] {
        return true;
    }
    let ["all", "(", args @ .., ")"] = inner else {
        return false;
    };
    let mut depth = 0;
    args.split(|t| {
        match *t {
            "(" => depth += 1,
            ")" => depth -= 1,
            _ => {}
        }
        depth == 0 && *t == ","
    })
    .any(|arg| arg == ["test"])
}

/// Remove what only exists for tests: items under `#[cfg(test)]` (or `cfg(all(test, ..))`) and
/// `#[test]` functions (a file that starts with `#![cfg(test)]` becomes empty).
pub fn strip_tests(toks: Vec<Tok>) -> Vec<Tok> {
    let attr_at = |i: usize| is(&toks[i], "#") && toks.get(i + 1).is_some_and(|t| is(t, "["));
    let mut out = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        let inner = toks.get(i + 1).is_some_and(|t| is(t, "!"));
        let open = i + 1 + usize::from(inner);
        if is(&toks[i], "#") && toks.get(open).is_some_and(|t| is(t, "[")) {
            let close = close_of(&toks, open);
            // A file that ends inside the attribute has no closing bracket: nothing to read.
            let body: Vec<&str> = toks
                .get(open + 1..close)
                .unwrap_or_default()
                .iter()
                .map(|t| t.text.as_str())
                .collect();
            if is_test_attribute(&body) {
                if inner {
                    return Vec::new();
                }
                i = close + 1;
                // Attributes that follow belong to the same item.
                while i < toks.len() && attr_at(i) {
                    i = close_of(&toks, i + 1) + 1;
                }
                i = item_end(&toks, i);
                continue;
            }
        }
        out.push(toks[i].clone());
        i += 1;
    }
    out
}

/// Every function with a body, nested functions inside another body not counted separately.
pub fn functions(toks: &[Tok]) -> Vec<Func> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < toks.len() {
        let named = toks[i + 1]
            .text
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_');
        if !(is(&toks[i], "fn") && named) {
            i += 1;
            continue;
        }
        // The body is the first `{` outside parentheses; a `;` first means a declaration.
        let mut k = i + 2;
        let open = loop {
            match toks.get(k).map(|t| t.text.as_str()) {
                None | Some(";") => break None,
                Some("(" | "[") => k = close_of(toks, k),
                Some("{") => break Some(k),
                _ => {}
            }
            k += 1;
        };
        let Some(open) = open else {
            i += 2;
            continue;
        };
        let close = close_of(toks, open);
        let tokens = toks[i..=close]
            .iter()
            .enumerate()
            .filter(|(n, _)| *n != 1)
            .map(|(_, t)| t.text.clone())
            .collect();
        out.push(Func {
            name: toks[i + 1].text.clone(),
            line: toks[i].line,
            tokens,
        });
        i = close + 1;
    }
    out
}

/// The lines where the token sequence `pattern` starts in `toks`.
///
/// Tokens are equal when their text is, except that the digits of a hex literal (`0x200b`) and of a
/// unicode escape (`'\u{FEFF}'`) are compared without regard to case, and that a `_` in the
/// pattern stands for any one identifier (`matches!(_.len(), 3 | 4)` reads `hex.len()` and
/// `s.len()`).
pub fn find(toks: &[Tok], pattern: &[Tok]) -> Vec<usize> {
    if pattern.is_empty() || pattern.len() > toks.len() {
        return Vec::new();
    }
    let pat: Vec<String> = pattern.iter().map(|t| fold(&t.text)).collect();
    let src: Vec<String> = toks.iter().map(|t| fold(&t.text)).collect();
    let same = |s: &String, p: &String| {
        s == p
            || (p == "_"
                && s.chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic() || c == '_'))
    };
    (0..=src.len() - pat.len())
        .filter(|&i| src[i..].iter().zip(&pat).all(|(s, p)| same(s, p)))
        .map(|i| toks[i].line)
        .collect()
}

/// A token with the case of hex digits taken out: a hex literal, and what is inside `\u{..}`.
fn fold(text: &str) -> String {
    if text.starts_with("0x") || text.starts_with("0X") {
        return text.to_lowercase();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("\\u{") {
        let (head, tail) = rest.split_at(at + 3);
        out.push_str(head);
        let end = tail.find('}').unwrap_or(tail.len());
        out.push_str(&tail[..end].to_lowercase());
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests;
