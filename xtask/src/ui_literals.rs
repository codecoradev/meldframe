//! `ui-literals` (U4): no colour, font-size or duration literals in UI code. Values come from
//! the tokens module (`tokens.rs` or `tokens.css`, exempt from this lint).

use std::fmt;
use std::path::Path;

use crate::rules::Package;
use crate::scan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Literal {
    Colour,
    FontSize,
    Duration,
}

/// One literal found at a line of a file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub path: String,
    pub line: usize,
    pub literal: Literal,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.literal {
            Literal::Colour => "colour",
            Literal::FontSize => "font-size",
            Literal::Duration => "duration",
        };
        write!(
            f,
            "{}:{}: {what} literal; read it from the tokens module",
            self.path, self.line
        )
    }
}

/// True for the tokens module itself, which is where literals live.
pub fn is_tokens_file(path: &str) -> bool {
    path.split('/')
        .any(|part| part == "tokens" || part.split('.').next() == Some("tokens"))
}

const COLOUR_FNS: &[&str] = &[
    "rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklab(", "oklch(",
];
const RS_DURATIONS: &[&str] = &[
    "from_millis(",
    "from_secs(",
    "from_micros(",
    "from_nanos(",
    "from_secs_f32(",
    "from_secs_f64(",
];

/// Byte offsets where `needle` occurs in `line` and is not glued to a preceding word.
fn finds<'a>(line: &'a str, needle: &'a str) -> impl Iterator<Item = usize> + 'a {
    line.match_indices(needle).map(|(i, _)| i).filter(|&i| {
        line[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric() && c != '-')
    })
}

/// True when `rest`, after separators, starts a numeric literal.
fn starts_number(rest: &str) -> bool {
    let v = rest.trim_start_matches([' ', ':', '=', '(']);
    v.starts_with(|c: char| c.is_ascii_digit() || c == '.')
}

fn has_hex_colour(line: &str) -> bool {
    line.match_indices('#').any(|(i, _)| {
        let rest = &line[i + 1..];
        let n = rest.chars().take_while(char::is_ascii_hexdigit).count();
        let after = rest[n..].chars().next();
        matches!(n, 3 | 4 | 6 | 8) && after.is_none_or(|c| !c.is_alphanumeric())
    })
}

/// A CSS time such as `200ms` or `.3s`.
fn has_css_time(line: &str) -> bool {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let glued = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if (b[i].is_ascii_digit() || b[i] == b'.') && !glued {
            let end = i + b[i..]
                .iter()
                .take_while(|c| c.is_ascii_digit() || **c == b'.')
                .count();
            let has_digit = b[i..end].iter().any(u8::is_ascii_digit);
            let unit = if line[end..].starts_with("ms") {
                2
            } else {
                usize::from(line[end..].starts_with('s'))
            };
            let tail_ok = b.get(end + unit).is_none_or(|c| !c.is_ascii_alphanumeric());
            if has_digit && unit > 0 && tail_ok {
                return true;
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    false
}

/// Literals on one line of a `.rs` or `.css` file (comments excluded by the caller).
pub fn scan_line(ext: &str, line: &str) -> Vec<Literal> {
    let mut out = Vec::new();
    let colour = has_hex_colour(line)
        || COLOUR_FNS.iter().any(|n| finds(line, n).next().is_some())
        || (ext == "rs" && ["from_rgb(", "from_rgba("].iter().any(|n| line.contains(n)));
    if colour {
        out.push(Literal::Colour);
    }
    let size = ["font-size", "font_size"].iter().any(|n| {
        line.match_indices(n)
            .any(|(i, _)| starts_number(&line[i + n.len()..]))
    });
    if size {
        out.push(Literal::FontSize);
    }
    let time = if ext == "rs" {
        RS_DURATIONS.iter().any(|n| {
            line.match_indices(n)
                .any(|(i, _)| starts_number(&line[i + n.len()..]))
        })
    } else {
        has_css_time(line)
    };
    if time {
        out.push(Literal::Duration);
    }
    out
}

/// Scan a whole file; skips the tokens module and comment lines.
pub fn check_file(path: &str, content: &str) -> Vec<Finding> {
    if is_tokens_file(path) {
        return Vec::new();
    }
    let ext = path.rsplit('.').next().unwrap_or_default();
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') {
            continue;
        }
        for literal in scan_line(ext, line) {
            out.push(Finding {
                path: path.to_string(),
                line: i + 1,
                literal,
            });
        }
    }
    out
}

/// File extensions to lint for a package: UI crates (kind `ui`, `feature-ui`) in Rust and CSS,
/// and the web app's styles (`apps/web`, CSS and SCSS). Anything else is out of scope.
pub fn scope(p: &Package) -> &'static [&'static str] {
    match (p.kind.as_deref(), p.dir.as_str()) {
        (Some("ui" | "feature-ui"), _) => &["rs", "css"],
        (_, "apps/web") => &["css", "scss"],
        _ => &[],
    }
}

/// Lint every in-scope file of the workspace.
pub fn collect(root: &Path, packages: &[Package]) -> Vec<Finding> {
    let mut out = Vec::new();
    for p in packages {
        for path in scan::files(root, &p.dir, scope(p)) {
            out.extend(check_file(&path, &scan::read(root, &path)));
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use Literal::*;

    fn css(line: &str) -> Vec<Literal> {
        scan_line("css", line)
    }
    fn rs(line: &str) -> Vec<Literal> {
        scan_line("rs", line)
    }

    #[test]
    fn colours_hex_and_functions() {
        assert_eq!(css("color: #fff;"), [Colour]);
        assert_eq!(css("color: #1A2b3C;"), [Colour]);
        assert_eq!(css("background: rgba(0, 0, 0, .5);"), [Colour]);
        assert_eq!(css("fill: oklch(0.7 0.1 200);"), [Colour]);
        assert_eq!(rs("let c = Color32::from_rgb(1, 2, 3);"), [Colour]);
        assert_eq!(rs("const C: &str = \"#ff00ff\";"), [Colour]);
    }

    #[test]
    fn token_references_and_unrelated_hashes_pass() {
        assert!(css("color: var(--color-text);").is_empty());
        assert!(rs("let c = tokens::COLOR_TEXT;").is_empty());
        assert!(rs("#[derive(Debug)]").is_empty());
        assert!(rs("let s = r#\"plain\"#;").is_empty());
        assert!(css("#header { margin: 0; }").is_empty());
        assert!(css("content: \"#12\";").is_empty());
    }

    #[test]
    fn font_sizes() {
        assert_eq!(css("font-size: 14px;"), [FontSize]);
        assert_eq!(css("font-size:.875rem;"), [FontSize]);
        assert_eq!(rs("let s = Text::new(x).font_size(14.0);"), [FontSize]);
        assert_eq!(rs("font_size: 12.0,"), [FontSize]);
        assert!(css("font-size: var(--font-size-md);").is_empty());
        assert!(rs("text.font_size(tokens::FONT_BODY);").is_empty());
    }

    #[test]
    fn durations() {
        assert_eq!(css("transition: opacity 200ms ease;"), [Duration]);
        assert_eq!(css("animation-duration: 0.3s;"), [Duration]);
        assert_eq!(rs("sleep(Duration::from_millis(250));"), [Duration]);
        assert_eq!(rs("let d = Duration::from_secs(2);"), [Duration]);
        assert!(css("transition: opacity var(--duration-fast);").is_empty());
        assert!(rs("Duration::from_millis(tokens::FAST_MS)").is_empty());
        assert!(css(".items { width: 100%; }").is_empty());
    }

    #[test]
    fn one_line_can_hold_several_kinds() {
        assert_eq!(
            css("a { color: #000; transition: color 1s; }"),
            [Colour, Duration]
        );
    }

    #[test]
    fn tokens_files_are_exempt() {
        assert!(is_tokens_file("crates/ui/meldframe-ui/src/tokens.rs"));
        assert!(is_tokens_file("apps/web/src/tokens.css"));
        assert!(is_tokens_file("crates/ui/x/src/tokens/colour.rs"));
        assert!(!is_tokens_file("crates/ui/x/src/token_like.rs"));
        assert!(check_file("a/tokens.rs", "const C: &str = \"#fff\";").is_empty());
    }

    #[test]
    fn check_file_reports_path_and_line_and_skips_comments() {
        let src = "// colour #fff in a comment\nfn f() {}\nlet c = \"#abc\";\n/* 10ms */\n";
        let f = check_file("crates/ui/x/src/lib.rs", src);
        assert_eq!(
            f,
            vec![Finding {
                path: "crates/ui/x/src/lib.rs".into(),
                line: 3,
                literal: Colour
            }]
        );
        assert!(f[0].to_string().starts_with("crates/ui/x/src/lib.rs:3:"));
    }

    #[test]
    fn scope_covers_ui_crates_and_web_styles_only() {
        let p = |kind: &str, dir: &str| Package {
            kind: Some(kind.into()),
            dir: dir.into(),
            ..Package::default()
        };
        assert_eq!(scope(&p("ui", "crates/ui/a")), ["rs", "css"]);
        assert_eq!(
            scope(&p("feature-ui", "crates/features/g/g-ui")),
            ["rs", "css"]
        );
        assert_eq!(scope(&p("bin", "apps/web")), ["css", "scss"]);
        assert!(scope(&p("bin", "apps/server")).is_empty());
        assert!(scope(&p("feature", "crates/features/g/g")).is_empty());
    }
}
