//! The file name of an export, worked out once for the PNG and the HTML (rule U9: the front end draws and saves under
//! the name it is given and cleans nothing).

use meldframe_text::{Profile, is_control_char, sanitize, sanitize_clip};

/// The longest stem, in characters, and in bytes (80 four-byte characters would be 320 bytes, more than a name on a file
/// system that counts bytes holds once the scale and the extension are added).
const MAX_STEM: usize = 80;
const MAX_STEM_BYTES: usize = 200;

/// The names Windows refuses whatever follows the first dot (`NUL`, `NUL.txt`, `NUL.tar.gz`).
const RESERVED: [&str; 4] = ["con", "prn", "aux", "nul"];

/// The stem of the file an export is saved under: the frame's name, with the characters a file system refuses and the
/// control characters turned into `-`, every hidden character left out (the strict profile of `meldframe-text`, so a
/// name cannot hide a second extension or a lookalike), every run of white space of any kind one plain space, cut to
/// [`MAX_STEM`] characters and [`MAX_STEM_BYTES`] bytes and with the dots and spaces at the start dropped. A name that
/// is nothing after that falls back to the node's id, and then to `export`. The scale (`@2x`) and the extension are the
/// front end's.
pub fn file_stem(name: &str, id: &str) -> String {
    let mut base = clean(name);
    if base.is_empty() {
        base = clean(id);
    }
    if base.is_empty() {
        base = "export".to_owned();
    }
    // Windows refuses a device name before the first dot: a hyphen after it makes it an ordinary name.
    // The spaces before the dot are dropped by Windows too: `CON .txt` is the device.
    let head = base.split('.').next().unwrap_or_default().trim_end();
    if is_device(head) {
        base.insert(head.len(), '-');
    }
    base
}

fn is_device(head: &str) -> bool {
    let lower = head.to_ascii_lowercase();
    RESERVED.contains(&lower.as_str())
        || ["com", "lpt"].iter().any(|p| {
            lower
                .strip_prefix(p)
                .is_some_and(|n| matches!(n.as_bytes(), [b'1'..=b'9']))
        })
}

fn clean(text: &str) -> String {
    // A character a file system refuses, or a control character, becomes a hyphen; the rest of the hidden ones go.
    let refused = |c: char| {
        matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || is_control_char(c)
    };
    let hyphened: String = text
        .chars()
        .map(|c| if refused(c) { '-' } else { c })
        .collect();
    let safe = sanitize(&hyphened, Profile::Strict);
    let folded = safe.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut = sanitize_clip(&folded, Profile::Strict, MAX_STEM, MAX_STEM_BYTES);
    cut.trim_start_matches(|c: char| c == '.' || c.is_whitespace())
        .trim_end()
        .to_owned()
}
