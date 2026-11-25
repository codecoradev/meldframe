//! The text of an element: the pieces of its content, joined as the browser would show them.

use super::pre::is_space;

/// A piece of the content of a line of text.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Piece<'a> {
    Text(&'a str),
    /// A `<br>`.
    Br,
}

/// The pieces as one string with the white space collapsed: a run of spaces, tabs and line ends is one space, a space
/// at the start or the end of a line goes, and a `<br>` is a line end.
pub(crate) fn collapse(pieces: &[Piece]) -> String {
    let mut out = String::new();
    let (mut space, mut start) = (false, true);
    for p in pieces {
        match p {
            Piece::Br => {
                space = false;
                start = true;
                out.push('\n');
            }
            Piece::Text(t) => {
                for c in t.chars() {
                    if is_space(c) {
                        space = !start;
                    } else {
                        if space {
                            out.push(' ');
                        }
                        space = false;
                        start = false;
                        out.push(c);
                    }
                }
            }
        }
    }
    // A `<br>` at the end of a block does not make another line.
    out.truncate(out.trim_end_matches('\n').len());
    out
}

/// One text with the white space collapsed.
pub(crate) fn collapse_plain(t: &str) -> String {
    collapse(&[Piece::Text(t)])
}
