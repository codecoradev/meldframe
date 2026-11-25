//! Seeded random declarations: whatever the reader makes of them, the Node it is put on passes the checks of the
//! protocol (`E_INVALID_*` is never what the importer writes), the answer is the same twice, and what was refused
//! left the default. Seeds per run: 300 natively, 20 under WASM; `MELDFRAME_SEEDS` sets the count and
//! `MELDFRAME_SEED=n` replays one.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::paint::Ctx;
use meldframe_check::{Limits, check_node};
use meldframe_protocol::Node;
use serde_json::json;

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

    fn pick<'a>(&mut self, of: &[&'a str]) -> &'a str {
        of[self.below(of.len() as u64) as usize]
    }
}

const COLOURS: [&str; 10] = [
    "#ff0000",
    "#0f0",
    "#00000080",
    "rgba(0,0,0,.5)",
    "hsl(10 50% 50%)",
    "red",
    "transparent",
    "currentcolor",
    "oklch(0.7 0.4 30)",
    "banana",
];
const ID: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

/// A colour that is mostly one the importer reads, and now and then one it does not.
fn colour(r: &mut Rng) -> &'static str {
    if r.below(8) == 0 {
        r.pick(&COLOURS)
    } else {
        r.pick(&["#ff0000", "#0f0", "#00000080", "rgba(0,0,0,.5)", "red"])
    }
}

fn gradient(r: &mut Rng) -> String {
    let kind = match r.below(20) {
        0..=11 => "linear-gradient",
        12..=16 => "radial-gradient",
        17 => "conic-gradient",
        _ => "repeating-linear-gradient",
    };
    let mut parts: Vec<String> = Vec::new();
    match (kind, r.below(4)) {
        ("radial-gradient", 0..=2) => parts.push(
            r.pick(&[
                "circle",
                "circle",
                "circle farthest-corner",
                "ellipse",
                "circle at top",
            ])
            .into(),
        ),
        ("linear-gradient", 0..=2) => parts.push(
            r.pick(&[
                "to right",
                "to top left",
                "45deg",
                "-90deg",
                "90deg",
                "1turn",
                "9000deg",
                "banana",
            ])
            .into(),
        ),
        _ => {}
    }
    for _ in 0..1 + r.below(10) {
        let pos = match r.below(10) {
            0..=5 => String::new(),
            6..=7 => format!(" {}%", r.below(101)),
            8 => format!(" {}% {}%", r.below(60), r.below(100)),
            _ => format!(" {}px", r.below(50)),
        };
        parts.push(format!("{}{pos}", colour(r)));
    }
    format!("{kind}({})", parts.join(","))
}

fn layer(r: &mut Rng) -> String {
    match r.below(5) {
        0 => format!("url(assets/{ID})"),
        1 => format!(
            "url({})",
            r.pick(&[
                "photo.png",
                "data:image/png;base64,AAAA",
                "logo.svg",
                "",
                "javascript:x"
            ])
        ),
        2 => "none".into(),
        _ => gradient(r),
    }
}

fn decls(r: &mut Rng) -> Vec<(String, String)> {
    let mut d: Vec<(String, String)> = Vec::new();
    let mut put = |n: &str, v: String| d.push((n.into(), v));
    if r.below(2) == 0 {
        put("background-color", r.pick(&COLOURS).into());
    }
    if r.below(2) == 0 {
        let list: Vec<String> = (0..1 + r.below(3)).map(|_| layer(r)).collect();
        put("background-image", list.join(","));
        if r.below(2) == 0 {
            // The three values of the export.
            put(
                "background-size",
                r.pick(&["cover", "contain", "100% 100%", "cover,100% 100%"])
                    .into(),
            );
            put("background-position", "center".into());
            put("background-repeat", "no-repeat".into());
        } else {
            for (n, of) in [
                ("background-size", ["cover", "auto", "20px", "100% 100%"]),
                (
                    "background-position",
                    ["center", "top left", "50% 50%", "center"],
                ),
                (
                    "background-repeat",
                    ["no-repeat", "repeat", "no-repeat", "repeat"],
                ),
            ] {
                if r.below(2) == 0 {
                    put(n, r.pick(&of).into());
                }
            }
        }
    }
    if r.below(2) == 0 {
        for side in ["top", "right", "bottom", "left"] {
            let same = r.below(3) != 0;
            let w = if same {
                "3px".into()
            } else {
                format!("{}px", r.below(9))
            };
            put(&format!("border-{side}-width"), w);
            put(
                &format!("border-{side}-style"),
                r.pick(&["solid", "solid", "dashed", "none"]).into(),
            );
            put(&format!("border-{side}-color"), r.pick(&COLOURS).into());
        }
    }
    if r.below(2) == 0 {
        let one = r.pick(&["8px", "8px", "0", "50%", "9999px", "1em", "9000000px"]);
        let same = r.below(3) != 0;
        for c in ["top-left", "top-right", "bottom-right", "bottom-left"] {
            let v = if same {
                one
            } else {
                r.pick(&["0", "50%", "banana", "10px 20px", "8px"])
            };
            put(&format!("border-{c}-radius"), v.into());
        }
    }
    if r.below(2) == 0 {
        let list: Vec<String> = (0..r.below(7))
            .map(|_| {
                format!(
                    "{}{}px {}px {}px {}px {}",
                    if r.below(5) == 0 { "inset " } else { "" },
                    r.below(9),
                    r.below(9),
                    r.below(20),
                    r.below(5),
                    r.pick(&COLOURS)
                )
            })
            .collect();
        put(
            "box-shadow",
            if list.is_empty() {
                "none".into()
            } else {
                list.join(",")
            },
        );
    }
    if r.below(2) == 0 {
        put(
            "opacity",
            r.pick(&["0", "0.5", "1", "1.5", "-1", "50%", "banana", "200%"])
                .into(),
        );
    }
    d
}

fn after(r: &mut Rng) -> Vec<(String, String)> {
    let mut a: Vec<(String, String)> = Vec::new();
    let mut put = |n: &str, v: &str| a.push((n.into(), v.into()));
    put("content", r.pick(&["\"\"", "''", "\"x\""]));
    put("position", r.pick(&["absolute", "absolute", "relative"]));
    let w = 1 + r.below(4);
    let inset = r.pick(&["0", "-0.5px", "-1px", "-2px", "-1.5px", "3px"]);
    for s in ["top", "right", "bottom", "left"] {
        put(s, inset);
    }
    put("border-top-width", &format!("{w}px"));
    put("border-top-style", "solid");
    put("border-top-color", r.pick(&COLOURS));
    for s in ["right", "bottom", "left"] {
        put(&format!("border-{s}-width"), &format!("{w}px"));
        put(&format!("border-{s}-style"), "solid");
        put(&format!("border-{s}-color"), "#ff0000");
    }
    if r.below(2) == 0 {
        let v = r.pick(&[
            "0px",
            "8px",
            "9px",
            "calc(min(6 * 1px, 50px, 20px) + 1px)",
            "50%",
            "banana",
            "calc(min(1px, 2px) + 1px)",
        ]);
        for c in ["top-left", "top-right", "bottom-right", "bottom-left"] {
            put(&format!("border-{c}-radius"), v);
        }
    }
    a
}

/// What a run made, so that a run that never made a border or an image says so.
#[derive(Default)]
struct Seen {
    flags: [bool; 9],
}

const NAMES: [&str; 9] = [
    "gradient",
    "image",
    "border",
    "border from the box",
    "radius",
    "ellipse",
    "shadows",
    "opacity",
    "assets",
];

fn run(seed: u64, seen: &mut Seen) {
    let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let (d, a) = (
        decls(&mut r),
        if r.below(2) == 0 {
            after(&mut r)
        } else {
            Vec::new()
        },
    );
    let ctx = Ctx {
        width: [None, Some(100.0), Some(10.0)][r.below(3) as usize],
        height: [None, Some(60.0), Some(10.0)][r.below(3) as usize],
        has_children: r.below(2) == 0,
    };
    let d2: Vec<(&str, &str)> = d.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
    let a2: Vec<(&str, &str)> = a.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
    let s = if a.is_empty() {
        style_of(&d2)
    } else {
        style_with_after(&d2, &a2)
    };
    let say = |what: &str| {
        format!("seed {seed}: {what}; replay with MELDFRAME_SEED={seed}\n{d:?}\n{a:?}")
    };
    let (p, rep) = paint_of(&s, &ctx);
    let (p2, rep2) = paint_of(&s, &ctx);
    assert_eq!(
        (&p, &rep.entries),
        (&p2, &rep2.entries),
        "{}",
        say("not the same twice")
    );
    let mut node: Node = serde_json::from_value(json!({"id": "n", "type": "rect"})).unwrap();
    p.apply(&mut node);
    if let Err(e) = check_node(&node, &Limits::default()) {
        panic!("{}: {e:?}\n{p:?}", say("the Node is refused by the checks"));
    }
    // What is read is within what the model holds, and an ellipse leaves the radius alone.
    assert!(
        p.shadows.len() <= 4
            && p.gradient
                .as_ref()
                .is_none_or(|g| (2..=8).contains(&g.stops.len())),
        "{}",
        say("a count")
    );
    assert!(
        !p.ellipse || p.radius == meldframe_protocol::Bind::Lit(0.0),
        "{}",
        say("an ellipse with a radius")
    );
    assert!(
        p.border_room == 0.0 || p.border_room == p.border.width,
        "{}",
        say("room")
    );
    assert!(
        rep.entries
            .iter()
            .all(|e| !e.message.chars().any(|c| c.is_control())),
        "{}",
        say("a control character in the report")
    );
    let made = [
        p.gradient.is_some(),
        p.image.is_some(),
        p.border.width > 0.0,
        p.border.width > 0.0 && p.border_room == 0.0,
        p.radius != meldframe_protocol::Bind::Lit(0.0),
        p.ellipse,
        !p.shadows.is_empty(),
        p.opacity != meldframe_protocol::Bind::Lit(1.0),
        !p.assets.is_empty(),
    ];
    for (flag, m) in seen.flags.iter_mut().zip(made) {
        *flag |= m;
    }
}

fn seeds() -> std::ops::Range<u64> {
    let env = |n: &str| std::env::var(n).ok().and_then(|v| v.parse::<u64>().ok());
    let default = if cfg!(target_arch = "wasm32") {
        20
    } else {
        300
    };
    match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => 0..env("MELDFRAME_SEEDS").unwrap_or(default),
    }
}

#[test]
fn random_declarations_give_a_paint_the_checks_accept() {
    let mut seen = Seen::default();
    for seed in seeds() {
        run(seed, &mut seen);
    }
    if seeds().end - seeds().start >= 100 {
        for (flag, name) in seen.flags.iter().zip(NAMES) {
            assert!(flag, "no run made: {name}");
        }
    }
}
