//! Property tests over seeded random HTML in and out of the supported subset: nested containers, rows, columns, texts,
//! images and controls, with inline styles and `<style>` rules, units and values of every kind. For every seed the plan
//! is made, is the same when made again, writes only what the checks of a write accept, has unique ids and parents
//! before children, and a property nobody reads changes none of what is written (`docs/import-html.md` section 5, item
//! 5). Seeds per run: 600 natively, 20 under WASM; `MELDFRAME_SEEDS` sets the count and `MELDFRAME_SEED=n` replays one.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::html::ParseError;
use html_import::map::{Options, plan};
use meldframe_protocol::Command;
use std::collections::HashSet;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
    fn pick<'a>(&mut self, of: &[&'a str]) -> &'a str {
        of[self.below(of.len() as u64) as usize]
    }
}

const LENGTHS: [&str; 14] = [
    "0",
    "1px",
    "12px",
    "37.5px",
    "2em",
    "1.5rem",
    "9pt",
    "50%",
    "100%",
    "auto",
    "fit-content",
    "calc(1px + 2px)",
    "-3px",
    "1e9px",
];
const DECLS: [&str; 30] = [
    "display:flex",
    "display:block",
    "display:none",
    "display:grid",
    "display:inline",
    "display:contents",
    "flex-direction:column",
    "flex-direction:row-reverse",
    "gap:6px",
    "gap:2px 9px",
    "align-items:center",
    "align-items:baseline",
    "justify-content:space-between",
    "justify-content:space-around",
    "flex:1",
    "flex:0 0 40px",
    "flex-wrap:wrap",
    "position:absolute",
    "position:relative",
    "overflow:hidden",
    "overflow:auto",
    "white-space:pre",
    "white-space:nowrap",
    "text-align:center",
    "font-weight:700",
    "text-transform:uppercase",
    "visibility:hidden",
    "background:#336699",
    "border:2px solid #000",
    "border-radius:50%",
];
const SIZED: [&str; 12] = [
    "width",
    "height",
    "padding",
    "margin",
    "min-width",
    "max-height",
    "font-size",
    "left",
    "top",
    "padding-left",
    "margin-top",
    "flex-basis",
];
const WORDS: [&str; 6] = [
    "Hello",
    "a  b\n c",
    "https://x.test",
    "日本語",
    "tab\there",
    "&lt;&amp;",
];
const TAGS: [&str; 15] = [
    "div", "section", "p", "span", "b", "ul", "li", "h1", "button", "a", "label", "img", "table",
    "tr", "td",
];

fn style(r: &mut Rng, extra: &str) -> String {
    let mut s = String::new();
    for _ in 0..r.below(5) {
        if r.chance(2) {
            s.push_str(r.pick(&DECLS));
        } else {
            s.push_str(&format!("{}:{}", r.pick(&SIZED), r.pick(&LENGTHS)));
        }
        s.push(';');
    }
    s.push_str(extra);
    s
}

fn element(r: &mut Rng, depth: usize, extra: &str, out: &mut String) {
    let tag = r.pick(&TAGS);
    let void = tag == "img";
    out.push_str(&format!("<{tag}"));
    if r.chance(3) {
        out.push_str(&format!(" data-name='n{}'", r.below(50)));
    }
    if r.chance(3) {
        out.push_str(&format!(" class='c{}'", r.below(4)));
    }
    if tag == "img" {
        out.push_str(" src='https://x.test/a.png'");
    }
    out.push_str(&format!(" style='{}'>", style(r, extra)));
    if void {
        return;
    }
    for _ in 0..r.below(4) {
        if depth < 5 && r.chance(2) {
            element(r, depth + 1, extra, out);
        } else if r.chance(5) {
            out.push_str("<br>");
        } else {
            out.push_str(r.pick(&WORDS));
        }
    }
    out.push_str(&format!("</{tag}>"));
}

fn html(seed: u64, extra: &str) -> (String, String) {
    let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let mut out = String::new();
    for _ in 0..1 + r.below(3) {
        element(&mut r, 0, extra, &mut out);
    }
    let css = format!(
        "body{{margin:{}}}.c0{{display:flex;gap:4px}}.c1{{padding:3px 5px;background:#eee}}.c2 p{{color:#f00}}.c3{{flex:1}}{extra_css}",
        r.pick(&["0", "8px", "2rem"]),
        extra_css = if extra.is_empty() {
            ""
        } else {
            ".c0,.c1,.c2,.c3{zoom:2}"
        }
    );
    (out, css)
}

fn seeds() -> Vec<u64> {
    if let Some(s) = std::env::var("MELDFRAME_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        return vec![s];
    }
    let default = if cfg!(target_arch = "wasm32") {
        20
    } else {
        600
    };
    let n = std::env::var("MELDFRAME_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default);
    (0..n).collect()
}

fn run(seed: u64, extra: &str) -> Result<html_import::map::Plan, ParseError> {
    let (h, c) = html(seed, extra);
    plan(&h, &c, &Options::default())
}

#[test]
fn what_is_planned_passes_every_check_and_is_well_formed() {
    let mut ok = 0;
    for seed in seeds() {
        let p = match run(seed, "") {
            Ok(p) => p,
            Err(e) => {
                assert_eq!(e, ParseError::Empty, "seed {seed}");
                continue;
            }
        };
        ok += 1;
        // Depth and count are the file's to refuse (E_TOO_DEEP, the batch limit), nothing else is.
        if let Err(e) = check_all(&p) {
            assert!(
                e.contains("TooDeep"),
                "seed {seed} (MELDFRAME_SEED={seed} replays it): {e}\n{:?}",
                html(seed, "")
            );
        }
        let list = nodes(&p);
        let ids: HashSet<&str> = list.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(
            (ids.len(), p.nodes.len(), p.commands.len()),
            (list.len(), list.len(), list.len()),
            "seed {seed}"
        );
        let mut seen = HashSet::new();
        for n in &list {
            assert!(
                n.parent_id.as_deref().is_none_or(|p| seen.contains(p)),
                "seed {seed}: {} before its parent",
                n.id
            );
            seen.insert(n.id.as_str());
        }
        assert!(
            p.commands
                .iter()
                .all(|c| matches!(c, Command::AddNode { .. }))
        );
        assert_eq!(
            list.iter().filter(|n| n.parent_id.is_none()).count(),
            1,
            "seed {seed}: one root"
        );
    }
    assert!(ok > 0);
}

#[test]
fn the_same_text_gives_the_same_plan() {
    for seed in seeds().into_iter().take(150) {
        let (a, b) = (run(seed, ""), run(seed, ""));
        match (a, b) {
            (Ok(a), Ok(b)) => same(seed, &a, &b),
            (Err(a), Err(b)) => assert_eq!(a, b),
            _ => panic!("seed {seed}: one plan and one refusal"),
        }
    }
}

/// The first command that differs, shown short.
fn same(seed: u64, a: &html_import::map::Plan, b: &html_import::map::Plan) {
    assert_eq!(a.commands.len(), b.commands.len(), "seed {seed}");
    for (i, (x, y)) in a.commands.iter().zip(&b.commands).enumerate() {
        let (x, y) = (format!("{x:?}"), format!("{y:?}"));
        assert!(x == y, "seed {seed}, command {i}:\n{x}\n{y}");
    }
}

#[test]
fn a_property_nobody_reads_changes_nothing_that_is_written() {
    for seed in seeds().into_iter().take(300) {
        let (plain, with) = (run(seed, ""), run(seed, "zoom:2;"));
        match (plain, with) {
            (Ok(a), Ok(b)) => same(seed, &a, &b),
            (Err(a), Err(b)) => assert_eq!(a, b),
            (a, b) => panic!(
                "seed {seed}: {:?} against {:?}",
                a.map(|_| ()),
                b.map(|_| ())
            ),
        }
    }
}
