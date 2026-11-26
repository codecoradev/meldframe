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

/// Limits so tight that every one of them is reached by a few pieces, and limits of nothing at all.
fn tight() -> Limits {
    Limits {
        max_rules: 3,
        max_decls: 5,
        max_value_bytes: 10,
        max_selector_bytes: 16,
        max_depth: 2,
        max_selectors: 4,
        max_work: 50,
        max_style_bytes: 500,
    }
}

fn zero() -> Limits {
    Limits {
        max_rules: 0,
        max_decls: 0,
        max_value_bytes: 0,
        max_selector_bytes: 0,
        max_depth: 0,
        max_selectors: 0,
        max_work: 0,
        max_style_bytes: 0,
    }
}

/// Read `css` with `l` and check what must hold of any text and any limits.
fn within(css: &str, l: &Limits) {
    let (s, r) = sheet_with(css, l);
    assert!(r.work <= 6 * css.len() + 64, "work for {css:?}");
    assert!(r.entries.len() <= MAX_ENTRIES);
    let listed: usize = r
        .entries
        .iter()
        .map(|e| e.name.len() + e.message.len() + e.code.len())
        .sum();
    assert!(listed <= MAX_REPORT_BYTES);
    assert!(s.rules.len() <= l.max_rules, "{css:?}");
    let selectors: usize = s.rules.iter().map(|r| r.selectors.len()).sum();
    let decls: usize = s.rules.iter().map(|r| r.decls.len()).sum();
    assert!(
        selectors <= l.max_selectors && decls <= l.max_decls,
        "{css:?}"
    );
    for rule in &s.rules {
        assert!(!rule.selectors.is_empty() && !rule.decls.is_empty());
        assert!(
            rule.decls
                .iter()
                .all(|d| d.value.len() <= l.max_value_bytes && !d.name.is_empty())
        );
    }
    // The same text gives the same answer.
    assert_eq!(sheet_with(css, l), (s, r));
    // A style attribute is read with the same limits.
    let mut rep = html_import::css::Report::default();
    let d = read_declarations(css, 0, l, &mut rep);
    assert!(d.len() <= l.max_decls && d.iter().all(|d| d.value.len() <= l.max_value_bytes));
    assert!(rep.work <= 6 * css.len() + 64);
}

fn bytes_ok(css: &str) {
    for l in [Limits::default(), tight(), zero()] {
        within(css, &l);
    }
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
            let d = read_declarations(&attr[..i], 0, &Limits::default(), &mut r);
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

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// The pieces of the random text, cut at `|`: selectors, at-rules, brackets, strings, escapes, comments, values,
/// the characters that end a piece, and what the reader reads in a special way.
const PIECES: &str = "a|div|.x|#y|*|[d]|[d=e]|[d=\"e\"]|:root|:hover|::after|::before|::x|>|+|~| |,|{|}|;|:|(|)|[|]|\"|'|\\|\\3c |/*|*/|@media|@layer a{|@layer|@import|@font-face|!important|!|url(|var(--a)|--a|color|red|#fff|1px|\n|\u{e9}|\u{1f600}|\0|calc(|= i|^=|<!--|-->|a{x:1}|@layer a{b{c:d}}|.a/**/.b|p{a:1;b:2;c:3;d:4;e:5;f:6;g:7}|q{v:abcdefghijklmnopqrstuvwxyz}";

/// Whole rules, which make most of the text that has any rule in it.
const RULES: &str = "a{x:1}|.b,.c{y:2;z:3}|p>q{a:b !important}|@media x{a{b:c}}|@layer a{b{c:d}}|:hover{a:b}|a{b:url(x;y)}|@import 'x';";

/// A text of `n` pieces, now and then a long run of one thing (a name, white space, a list of selectors, a
/// comment, a value) so that the caps and the limits are reached by text that is not random.
fn text(r: &mut Rng) -> String {
    let pieces: Vec<&str> = PIECES.split('|').collect();
    let rules: Vec<&str> = RULES.split('|').collect();
    let n = 1 + r.below(300);
    let mut css = String::new();
    for _ in 0..n {
        match r.below(60) {
            0 => css.push_str(&"x".repeat(200 + r.below(1_500))),
            1 => css.push_str(&" ".repeat(300 + r.below(1_500))),
            2 => css.push_str(&"a,".repeat(50 + r.below(800))),
            3 => css.push_str(&"/**/".repeat(100 + r.below(600))),
            4 => css.push_str(&"@layer a{".repeat(1 + r.below(20))),
            5..=24 => css.push_str(rules[r.below(rules.len())]),
            _ => css.push_str(pieces[r.below(pieces.len())]),
        }
    }
    css
}

fn run_seed(seed: u64) {
    let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let css = text(&mut r);
    bytes_ok(&css);
    // Every limit is reached in a tenth of the seeds or more: count what the tight limits cut.
    let (_, rep) = sheet_with(&css, &tight());
    assert!(rep.counts.values().sum::<usize>() >= rep.entries.len());
}

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn seeds() -> std::ops::Range<u64> {
    let default = if cfg!(target_arch = "wasm32") {
        20
    } else {
        600
    };
    match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => 0..env("MELDFRAME_SEEDS").unwrap_or(default),
    }
}

#[test]
fn random_text_never_panics_and_stays_within_every_limit() {
    for seed in seeds() {
        run_seed(seed);
    }
}

#[test]
fn the_tight_limits_are_reached_by_the_random_text() {
    // Each limit is hit by some seeds, so the test above is not only reading text that stays under them.
    let mut hit = std::collections::BTreeMap::new();
    for seed in seeds().take(200) {
        let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
        let (_, rep) = sheet_with(&text(&mut r), &tight());
        for code in [
            "rule_limit",
            "declaration_limit",
            "value_too_long",
            "selector_too_long",
            "depth_cut",
            "selector_limit",
        ] {
            *hit.entry(code).or_insert(0) += count(&rep, code).min(1);
        }
    }
    for (code, n) in hit {
        assert!(
            n > 0 || seeds().end - seeds().start < 60,
            "no seed reached {code}"
        );
    }
}
