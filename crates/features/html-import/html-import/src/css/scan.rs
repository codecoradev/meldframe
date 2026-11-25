//! The byte reader under the CSS reader: it cuts text into the pieces between `;`, `{` and `}` that sit outside
//! strings, comments, escapes and brackets. Every call moves forward, so the whole read is one pass; nothing here
//! recurses, and `skip_block` is a loop. A piece is kept only up to a cap (the rest is read and not stored), so a
//! hostile megabyte of text in one piece costs time, not memory.

/// What ended a piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum End {
    Semi,
    Open,
    Close,
    /// The text ended.
    Eof,
}

/// A piece of text up to its end byte.
pub(super) struct Piece {
    /// The text, comments made one space, kept up to the cap (cut on a character boundary).
    pub text: String,
    pub end: End,
    /// The piece was longer than the cap, and `text` holds only its first bytes.
    pub too_long: bool,
    /// The byte of the text where the piece starts (the white space before it included).
    pub at: usize,
    /// The piece ran into the end of the text inside a string, a comment or a bracket.
    pub unterminated: bool,
    /// A string in the piece ended at its line, not at its quote: CSS makes the piece invalid.
    pub bad_string: bool,
}

pub(super) struct Cursor<'a> {
    text: &'a str,
    pub at: usize,
    /// Bytes looked at, for [`crate::html::Report::work`].
    pub work: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(text: &'a str) -> Self {
        Cursor {
            text,
            at: 0,
            work: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    /// The character at `at`, which is on a boundary.
    fn ch(&self) -> char {
        self.text[self.at..].chars().next().unwrap_or(' ')
    }

    /// Past a comment that starts here; false when none does. An unfinished comment runs to the end.
    fn comment(&mut self) -> Option<bool> {
        if !self.text[self.at..].starts_with("/*") {
            return None;
        }
        let from = self.at + 2;
        match self.text[from..].find("*/") {
            Some(k) => {
                self.work += k + 4;
                self.at = from + k + 2;
                Some(true)
            }
            None => {
                self.work += self.text.len() - self.at;
                self.at = self.text.len();
                Some(false)
            }
        }
    }

    /// Read a piece. `cap` is the most bytes of it that are kept.
    pub fn piece(&mut self, cap: usize) -> Piece {
        let mut p = Piece {
            text: String::new(),
            end: End::Eof,
            too_long: false,
            at: self.at,
            unterminated: false,
            bad_string: false,
        };
        let mut depth = 0usize;
        // Push what was read, until the cap.
        macro_rules! keep {
            ($s:expr) => {{
                let s: &str = $s;
                if p.text.len() + s.len() <= cap {
                    p.text.push_str(s);
                } else {
                    p.too_long = true;
                }
            }};
        }
        while let Some(c) = self.peek() {
            self.work += 1;
            match self.comment() {
                Some(true) => {
                    keep!(" ");
                    continue;
                }
                Some(false) => {
                    p.unterminated = true;
                    keep!(" ");
                    break;
                }
                None => {}
            }
            match c {
                b'"' | b'\'' => {
                    let start = self.at;
                    self.at += 1;
                    loop {
                        match self.peek() {
                            // An unfinished string ends at the line, as CSS says, or at the end of the text.
                            None => {
                                p.unterminated = true;
                                break;
                            }
                            Some(b'\n' | b'\r' | 0x0c) => {
                                p.bad_string = true;
                                break;
                            }
                            Some(b'\\') => {
                                self.at += 1;
                                if self.peek().is_some() {
                                    self.at += self.ch().len_utf8();
                                }
                            }
                            Some(q) if q == c => {
                                self.at += 1;
                                break;
                            }
                            Some(_) => self.at += 1,
                        }
                    }
                    // A skip over a multi-byte character may land past the end of a short string.
                    let end = self.at.min(self.text.len());
                    self.work += end - start;
                    keep!(&self.text[start..end]);
                }
                b'\\' => {
                    let start = self.at;
                    self.at += 1;
                    if self.peek().is_some() {
                        self.at += self.ch().len_utf8();
                    }
                    keep!(&self.text[start..self.at]);
                }
                b'(' | b'[' => {
                    depth += 1;
                    keep!(if c == b'(' { "(" } else { "[" });
                    self.at += 1;
                }
                b')' | b']' => {
                    depth = depth.saturating_sub(1);
                    keep!(if c == b')' { ")" } else { "]" });
                    self.at += 1;
                }
                b';' | b'{' | b'}' if depth == 0 => {
                    p.end = match c {
                        b';' => End::Semi,
                        b'{' => End::Open,
                        _ => End::Close,
                    };
                    return p;
                }
                _ => {
                    let ch = self.ch();
                    let mut buf = [0u8; 4];
                    keep!(ch.encode_utf8(&mut buf));
                    self.at += ch.len_utf8();
                }
            }
        }
        p.unterminated |= depth > 0;
        p
    }

    /// Step over the end byte that the last piece stopped at.
    pub fn consume(&mut self) {
        self.at += 1;
    }

    /// Read past the block that just opened and keep nothing. A loop, so that nesting cannot grow the stack.
    /// Returns false when the text ended inside it.
    pub fn skip_block(&mut self) -> bool {
        let mut open = 1usize;
        while open > 0 {
            match self.piece(0).end {
                End::Eof => return false,
                End::Open => open += 1,
                End::Close => open -= 1,
                End::Semi => {}
            }
            self.consume();
        }
        true
    }
}
