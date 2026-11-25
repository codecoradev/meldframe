//! Hostile and random CSS: the reader never panics, never reads more than its bounds allow, gives the same answer
//! twice, and only makes rules that have a selector and a declaration. Seeds per run: 600 natively, 20 under WASM;
//! `MELDFRAME_SEEDS` sets the count and `MELDFRAME_SEED=n` replays one.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod css_common;
use css_common::*;
use html_import::css::{Limits, MAX_ENTRIES, MAX_REPORT_BYTES, read_declarations};

const DOC: &str = "@charset \"utf-8\";@import url(javascript:alert(1));/* c */:root{--a:#fff;--b:var(--a)}\n\
    @media (min-width:1px){a{x:1}}@layer l{[data-node=\"a\\3c b\"]>.c #d::after{content:\"\\\"\";inset:-1px!important}}\n\
    a:hover,b ~ c,*{background:url('x;y') no-repeat;font:12px/1 \"A;B\"}\u{e9}\u{1f600}{x:\u{e9}}@keyframes k{from{a:b}to{a:c}}\n\
    p{margin:0 auto;padding:calc(1px + 2px);}.a.b.c[d=e]{width:100%}\\{\\}{x:y}";

fn bytes_ok(css: &str) {
    let (s, r) = sheet(css);
    assert!(r.work <= 6 * css.len() + 64, "work for {css:?}");
    assert!(r.entries.len() <= MAX_ENTRIES);
    let listed: usize = r
        .entries
        .iter()
        .map(|e| e.name.len() + e.message.len() + e.code.len())
        .sum();
    assert!(listed <= MAX_REPORT_BYTES);
    for rule in &s.rules {
        assert!(!rule.selectors.is_empty() && !rule.decls.is_empty());
        assert!(
            rule.decls
                .iter()
                .all(|d| d.value.len() <= 1_000 && !d.name.is_empty())
        );
    }
    // The same text gives the same answer.
    assert_eq!(sheet(css), (s, r));
}

#[test]
fn every_prefix_of_a_hostile_document_is_read() {
    let mut cuts = 0;
    for i in 0..=DOC.len() {
        if DOC.is_char_boundary(i) {
            bytes_ok(&DOC[..i]);
            cuts += 1;
        }
    }
    assert!(cuts > 300, "{cuts}");
    // The whole document: what it is read as.
    let (s, r) = sheet(DOC);
    assert_eq!(
        dump(&s),
        [
            ":root{--a:#fff; --b:var(--a)}",
            "[data-node=a<b]>.c #d::after{content:\"\\\"\"; inset:-1px!}",
            "*{background:url('x;y') no-repeat; font:12px/1 \"A;B\"}",
            "\u{e9}\u{1f600}{x:\u{e9}}",
            "p{margin:0 auto; padding:calc(1px + 2px)}",
            ".a.b.c[d=e]{width:100%}",
            // An escaped brace is a name, here a type selector.
            "{}{x:y}",
        ]
    );
    // Nothing was followed: the `url()` is text in a value, and the import is listed.
    assert_eq!(count(&r, "import_skipped"), 1);
}

#[test]
fn every_prefix_of_a_style_attribute_is_read() {
    let attr = "color:red!important;background:url(a;b);content:\"x;y\";--k:v;width:calc(1px + \u{e9});}{;";
    for i in 0..=attr.len() {
        if attr.is_char_boundary(i) {
            let mut r = html_import::css::Report::default();
            let d = read_declarations(&attr[..i], &Limits::default(), &mut r);
            assert!(d.len() <= 5 && r.work <= 6 * i + 64);
        }
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn pick<'a>(&mut self, of: &[&'a str]) -> &'a str {
        of[(self.next() % of.len() as u64) as usize]
    }
}

const PIECES: &[&str] = &[
    "a",
    "div",
    ".x",
    "#y",
    "*",
    "[d]",
    "[d=e]",
    "[d=\"e\"]",
    ":root",
    ":hover",
    "::after",
    "::before",
    "::x",
    ">",
    "+",
    "~",
    " ",
    ",",
    "{",
    "}",
    ";",
    ":",
    "(",
    ")",
    "[",
    "]",
    "\"",
    "'",
    "\\",
    "\\3c ",
    "/*",
    "*/",
    "@media",
    "@layer",
    "@import",
    "@font-face",
    "!important",
    "!",
    "url(",
    "var(--a)",
    "--a",
    "color",
    "red",
    "#fff",
    "1px",
    "\n",
    "\u{e9}",
    "\u{1f600}",
    "\0",
    "calc(",
    "= i",
    "^=",
];

fn run_seed(seed: u64) {
    let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let n = 1 + (r.next() % 300) as usize;
    let css: String = (0..n).map(|_| r.pick(PIECES)).collect();
    bytes_ok(&css);
    let mut rep = html_import::css::Report::default();
    let d = read_declarations(&css, &Limits::default(), &mut rep);
    assert!(d.iter().all(|d| !d.name.is_empty()), "seed {seed}");
}

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

#[test]
fn random_text_never_panics_and_stays_within_bounds() {
    let default = if cfg!(target_arch = "wasm32") {
        20
    } else {
        600
    };
    let range = match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => 0..env("MELDFRAME_SEEDS").unwrap_or(default),
    };
    for seed in range {
        run_seed(seed);
    }
}
