//! Property test of the tree builder: random tag soup, made of the pieces that have a rule (tags that imply an end,
//! void and raw-text elements, comments, quotes, references, stray end tags, unfinished constructs) under limits that
//! change with the seed, some of them small enough that every bound is met.
//!
//! Checked for every text, whatever it makes:
//! - it does not panic, and it is refused only for a reason the text gives (too large, too many elements, no element);
//! - the tree is well formed: each element is the child of exactly one element, its parent is that element, and
//!   nothing is out of reach of the root;
//! - the tree is inside the bounds: elements, depth, attributes of an element, the bytes of a value, of a name and of
//!   a text; names are lower case, an attribute name is not repeated and is not an event; no NUL or carriage return
//!   is kept; no two texts are side by side;
//! - the report is inside its bounds, and the work is within the window of the limits for each byte of text;
//! - the same text gives the same tree and report.
//!
//! Seeds per run: 200 by default; set `MELDFRAME_SEEDS` for more. `MELDFRAME_SEED=n` replays one seed. Under WASM the
//! default is 20 seeds (`just wasm-test`; the environment does not reach a WASM test, so the count cannot be raised
//! there). A failure prints the seed and the text.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use html_import::html::{
    Child, Limits, MAX_ENTRIES, MAX_NAME_BYTES, MAX_REPORT_BYTES, ParseError, Parsed, parse,
};

const DEFAULT_SEEDS: u64 = if cfg!(target_arch = "wasm32") {
    20
} else {
    200
};

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn seeds() -> std::ops::Range<u64> {
    match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => 0..env("MELDFRAME_SEEDS").unwrap_or(DEFAULT_SEEDS),
    }
}

/// Small deterministic generator, so a failing seed can be replayed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const PIECES: [&str; 67] = [
    "<div>",
    "</div>",
    "<p>",
    "</p>",
    "<span>",
    "</span>",
    "<b>",
    "</i>",
    "<i>",
    "</b>",
    "<ul>",
    "</ul>",
    "<li>",
    "<ol>",
    "<dl>",
    "<dt>",
    "<dd>",
    "<table>",
    "</table>",
    "<tr>",
    "<td>",
    "<th>",
    "<tbody>",
    "<select>",
    "<option>",
    "<h1>",
    "<h2>",
    "<br>",
    "<br/>",
    "<img src=x>",
    "<input value=\"a>b\">",
    "<a href=\"u\" class='c d'>",
    "</a>",
    "<DIV CLASS=A>",
    "<p onclick=x title=\"t\">",
    "<x-y data-a=1 data-a=2 e>",
    "<style>",
    "</style>",
    "<script>",
    "</script>",
    "<title>",
    "</title>",
    "<textarea>",
    "</textarea>",
    "<!-- c -->",
    "<!--",
    "-->",
    "<!DOCTYPE html>",
    "<![CDATA[",
    "<?pi",
    "&amp;",
    "&#65;",
    "&#x1F600;",
    "&bogus",
    "&",
    "<",
    "</",
    "\"",
    "'",
    "=",
    "/>",
    "\0",
    "\r\n",
    "\r",
    "nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn",
    "é",
    "text ",
];

fn soup(r: &mut Rng) -> String {
    let n = r.below(120);
    let mut s = String::new();
    for _ in 0..n {
        s.push_str(PIECES[r.below(PIECES.len())]);
        if r.below(9) == 0 {
            s.push_str(&"x".repeat(r.below(40)));
        }
    }
    s
}

fn limits_for(seed: u64) -> Limits {
    match seed % 4 {
        0 => Limits::default(),
        1 => Limits {
            max_input_bytes: 600,
            max_elements: 12,
            max_depth: 4,
            max_attributes: 3,
            max_value_bytes: 8,
            max_class_bytes: 8,
            max_text_bytes: 16,
        },
        2 => Limits {
            max_elements: 25,
            max_depth: 2,
            max_attributes: 1,
            max_value_bytes: 2,
            max_class_bytes: 2,
            max_text_bytes: 5,
            ..Limits::default()
        },
        _ => Limits {
            max_depth: 1_000,
            ..Limits::default()
        },
    }
}

fn check(seed: u64, html: &str, l: &Limits, p: &Parsed) {
    let ctx = |what: &str| {
        format!(
            "seed {seed}: {what}\n{html:?}\nreplay: MELDFRAME_SEED={seed} cargo test -p meldframe-import-html-tests --test props"
        )
    };
    let t = &p.tree;
    assert!(t.len() <= l.max_elements, "{}", ctx("elements"));
    // Shape: walk from the root, each element once.
    let mut seen = vec![false; t.len() + 1];
    let mut stack = vec![(t.root_id(), 0usize)];
    let mut reached = 0;
    while let Some((id, depth)) = stack.pop() {
        let el = t.element(id);
        assert!(depth <= l.max_depth, "{}", ctx("depth"));
        let mut last_text = false;
        for c in &el.children {
            match c {
                Child::Text(s) => {
                    assert!(
                        !last_text && !s.is_empty(),
                        "{}",
                        ctx("adjacent or empty text")
                    );
                    last_text = true;
                    assert!(!s.contains(['\0', '\r']), "{}", ctx("NUL or CR in text"));
                    if el.tag != "style" {
                        assert!(s.len() <= l.max_text_bytes, "{}", ctx("text"));
                    }
                }
                Child::Element(e) => {
                    last_text = false;
                    assert!(!seen[e.index()], "{}", ctx("an element reached twice"));
                    seen[e.index()] = true;
                    reached += 1;
                    assert_eq!(t.element(*e).parent, id, "{}", ctx("parent"));
                    stack.push((*e, depth + 1));
                }
            }
        }
        if id != t.root_id() {
            assert!(
                el.tag.len() <= MAX_NAME_BYTES && !el.tag.is_empty(),
                "{}",
                ctx("tag")
            );
            assert_eq!(el.tag, el.tag.to_ascii_lowercase(), "{}", ctx("case"));
            assert!(el.attrs.len() <= l.max_attributes, "{}", ctx("attributes"));
            for (k, a) in el.attrs.iter().enumerate() {
                assert!(a.value.len() <= l.max_value_bytes, "{}", ctx("value"));
                assert!(a.name.len() <= MAX_NAME_BYTES, "{}", ctx("name"));
                assert!(
                    !a.value.contains(['\0', '\r']) && !a.name.contains('\0'),
                    "{}",
                    ctx("NUL")
                );
                assert!(
                    el.attrs[..k].iter().all(|b| b.name != a.name),
                    "{}",
                    ctx("repeated")
                );
                assert!(
                    !(a.name.starts_with("on") && a.name.len() > 2),
                    "{}",
                    ctx("event")
                );
            }
        }
    }
    assert_eq!(reached, t.len(), "{}", ctx("unreachable elements"));
    // Report.
    let r = &p.report;
    assert!(r.entries.len() <= MAX_ENTRIES, "{}", ctx("entries"));
    let bytes: usize = r
        .entries
        .iter()
        .map(|e| e.name.len() + e.message.len() + e.code.len())
        .sum();
    assert!(bytes <= MAX_REPORT_BYTES, "{}", ctx("report bytes"));
    // Work.
    assert!(
        r.work <= (l.max_depth.min(html.len() + 1) + 4) * html.len() + 64,
        "{}: {}",
        ctx("work"),
        r.work
    );
}

#[test]
fn random_tag_soup_is_read_inside_the_bounds() {
    let mut ok = 0;
    let mut refused = [0usize; 3];
    for seed in seeds() {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let l = limits_for(seed);
        for _ in 0..25 {
            let html = soup(&mut r);
            match parse(&html, &l) {
                Ok(p) => {
                    ok += 1;
                    check(seed, &html, &l, &p);
                    let again = parse(&html, &l).unwrap();
                    assert_eq!(p, again, "seed {seed}: not deterministic\n{html:?}");
                }
                Err(ParseError::TooLarge { bytes, max }) => {
                    refused[0] += 1;
                    assert!(
                        bytes > max && bytes == html.len() && max == l.max_input_bytes,
                        "seed {seed}"
                    );
                }
                Err(ParseError::TooMany { max, at }) => {
                    refused[1] += 1;
                    assert!(
                        max == l.max_elements && at < html.len(),
                        "seed {seed}\n{html:?}"
                    );
                }
                Err(ParseError::Empty) => refused[2] += 1,
            }
        }
    }
    println!("random_tag_soup_is_read_inside_the_bounds: {ok} read, refused {refused:?}");
    if seeds().end - seeds().start >= 100 {
        // The run is not idle: texts are read, and each refusal is met.
        assert!(ok > 1_000, "{ok}");
        assert!(refused.iter().all(|n| *n > 0), "{refused:?}");
    }
}
