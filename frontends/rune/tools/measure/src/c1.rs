//! C1 Fidelity of boxes (section 4 of the spike note): the boxes the painter draws from the core's scene against the rects of
//! the core's layout.
//!
//! The stage read here is the highest one: the shape the painter hands to Rune (`canvas_ui_rune::paint`), read back as the
//! corners of each fill, in `f32`. It proves that the scene to shape step (the world rectangles, the corner paths) does not move,
//! round or resize a box. It does NOT read the geometry after Rune's own transform and scale factor, and the painter has no
//! camera, so the scale 2 run and the run with a fractional pan offset are not run. The verdict of C1 stays `open` for that.

use crate::files::{TableFile, layout_trees, table_measurer, text_fixtures};
use crate::report::Report;
use meldframe_app::App;
use meldframe_layout::{StubMeasurer, TextMeasurer, compute_rects};
use meldframe_protocol::Node;
use rune::shape::Op;
use std::collections::HashMap;

/// The limit of the criterion [E]: 0.05 px.
pub const LIMIT: f64 = 0.05;

/// The corners `[x0, y0, x1, y1]` of what one fill of the shape covers.
fn corners(op: &Op) -> Option<[f32; 4]> {
    let points: Vec<[f32; 2]> = match op {
        Op::Fill { points, .. } => points.clone(),
        Op::FillCompound { contours, .. } => contours.concat(),
        _ => return None,
    };
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for p in &points {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    Some([lo[0], lo[1], hi[0], hi[1]])
}

/// What one file came to: the rects of the core, the boxes painted and the largest difference, with the node it was at.
#[derive(Debug, Default)]
pub struct FileResult {
    pub rects: usize,
    pub painted: usize,
    pub largest: f64,
    pub at: String,
    /// Boxes the core gives a negative width or height (rule G3 of the contract: outside what it promises), and the largest
    /// difference among them, kept apart.
    pub negative: usize,
    pub negative_largest: f64,
    pub error: Option<String>,
}

/// The oracle fixtures write a transparent fill as `transparent`, which the layout core never reads and the app layer refuses as a
/// colour; the file is loaded with `#00000000`, which changes no box.
fn loadable(mut nodes: Vec<Node>) -> Vec<Node> {
    for n in &mut nodes {
        if matches!(&n.fill, meldframe_protocol::Bind::Lit(v) if v == "transparent") {
            n.fill = meldframe_protocol::Bind::Lit("#00000000".into());
        }
    }
    nodes
}

/// Paints `nodes` through the app layer with the layout on the canvas and the measurer `make` gives, and compares each filled
/// box with the core's rect for it.
pub fn compare(nodes: Vec<Node>, make: &dyn Fn() -> Box<dyn TextMeasurer + Send>) -> FileResult {
    let mut out = FileResult::default();
    let mut app = match App::new("user:measure", "measure", 1, loadable(nodes), 0) {
        Ok(app) => app,
        Err(e) => {
            out.error = Some(format!("the file does not load: {e:?}"));
            return out;
        }
    };
    app.set_layout_on_canvas(true);
    app.set_measurer(make());
    let core: HashMap<String, _> = compute_rects(app.nodes(), make().as_ref())
        .into_iter()
        .collect();
    out.rects = core.len();
    let (shape, filled) = app.with_scene(|_, boxes| {
        let placed = meldframe_render::world_rects(boxes);
        let filled: Vec<String> = boxes
            .iter()
            .zip(&placed)
            .filter(|(b, p)| p.is_some() && b.painted_fill().is_some())
            .map(|(b, _)| b.id.clone())
            .collect();
        (canvas_ui_rune::paint(boxes), filled)
    });
    let fills: Vec<[f32; 4]> = shape.ops.iter().filter_map(corners).collect();
    if fills.len() != filled.len() {
        out.error = Some(format!(
            "{} fills in the shape for {} filled boxes",
            fills.len(),
            filled.len()
        ));
        return out;
    }
    for (id, c) in filled.iter().zip(&fills) {
        let Some(want) = core.get(id) else { continue };
        let got = [
            f64::from(c[0]),
            f64::from(c[1]),
            f64::from(c[2] - c[0]),
            f64::from(c[3] - c[1]),
        ];
        let want = [want.x, want.y, want.w, want.h];
        let worst = got
            .iter()
            .zip(want)
            .map(|(g, w)| (g - w).abs())
            .fold(0.0, f64::max);
        out.painted += 1;
        if want[2] < 0.0 || want[3] < 0.0 {
            out.negative += 1;
            out.negative_largest = out.negative_largest.max(worst);
            continue;
        }
        if out.at.is_empty() || worst > out.largest {
            out.largest = worst;
            out.at.clone_from(id);
        }
    }
    out
}

/// Runs C1 over the 15 layout golden trees (the stub measurer) and the oracle text fixtures (the recorded table, when it can be
/// read).
pub fn run(table: Option<&TableFile>) -> Report {
    let mut r = Report::new("C1");
    r.label("stage", "shape_ops_before_rune_transform");
    let (mut nodes, mut painted, mut worst) = (0usize, 0usize, (0.0f64, String::new()));
    let mut negative = (0usize, 0.0f64);
    let mut errors = 0;
    let mut note = |r: &mut Report, name: String, res: FileResult| {
        let l = [("fixture", name.as_str())];
        r.int("c1.rects", res.rects as i64, "count", &l)
            .int("c1.painted_fills", res.painted as i64, "count", &l)
            .num("c1.diff_max", res.largest, "px", &l)
            .int("c1.negative_size_rects", res.negative as i64, "count", &l)
            .num("c1.diff_max_negative_size", res.negative_largest, "px", &l);
        if let Some(e) = &res.error {
            r.word("c1.error", e, &l);
            errors += 1;
        }
        nodes += res.rects;
        painted += res.painted;
        negative.0 += res.negative;
        negative.1 = negative.1.max(res.negative_largest);
        if res.largest > worst.0 || worst.1.is_empty() {
            worst = (res.largest, format!("{name}/{}", res.at));
        }
    };
    for (i, tree) in layout_trees().into_iter().enumerate() {
        let make = || -> Box<dyn TextMeasurer + Send> { Box::new(StubMeasurer) };
        note(
            &mut r,
            format!("layout-golden-{i:02}"),
            compare(tree.nodes, &make),
        );
    }
    match table {
        Some(t) => {
            for f in text_fixtures() {
                let make = || -> Box<dyn TextMeasurer + Send> { Box::new(table_measurer(t)) };
                note(&mut r, f.name.clone(), compare(f.nodes, &make));
            }
        }
        None => {
            r.word(
                "c1.oracle_fixtures",
                "skipped",
                &[("reason", "the_recorded_table_cannot_be_read")],
            );
        }
    }
    r.int("c1.rects_total", nodes as i64, "count", &[])
        .int("c1.painted_fills_total", painted as i64, "count", &[])
        .num(
            "c1.diff_max_total",
            worst.0,
            "px",
            &[("at", &worst.1), ("negative_size_rects", "left_out")],
        )
        .int(
            "c1.negative_size_rects_total",
            negative.0 as i64,
            "count",
            &[("outside_contract", "rule_G3")],
        )
        .num(
            "c1.diff_max_negative_size_total",
            negative.1,
            "px",
            &[("outside_contract", "rule_G3")],
        )
        .word(
            "c1.scale2",
            "skipped",
            &[("reason", "the_stage_read_has_no_scale_factor")],
        )
        .word(
            "c1.fractional_pan",
            "skipped",
            &[("reason", "the_painter_has_no_camera")],
        )
        .word(
            "c1.fixtures_without_text",
            "skipped",
            &[(
                "reason",
                "only_oracle_fixtures_with_text_are_exported_as_json",
            )],
        );
    let shape_ops = if errors > 0 {
        "open"
    } else if worst.0 <= LIMIT {
        "green"
    } else {
        "gap"
    };
    r.word("c1.shape_ops_verdict", shape_ops, &[("limit_px", "0.05")])
        .status("partial", "only the shape the painter hands to Rune was read, not Rune's geometry after its transform and scale")
        .verdict("open");
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_painted_box_is_where_the_core_put_it() {
        let nodes: Vec<Node> = serde_json::from_str(
            r##"[{"id":"p","type":"frame","x":10,"y":20,"w":100,"h":80,"layout":"column","padding":7,"gap":3,"fill":"#ffffff"},
                 {"id":"a","type":"rect","parentId":"p","w":40,"h":10.5,"fill":"#ff0000"},
                 {"id":"b","type":"rect","parentId":"p","w":30,"h":12.25,"fill":"#00ff00","radius":4}]"##,
        )
        .expect("nodes");
        let res = compare(nodes, &|| Box::new(StubMeasurer));
        assert_eq!(res.error, None);
        assert_eq!((res.rects, res.painted), (3, 3));
        assert!(res.largest <= LIMIT, "{} at {}", res.largest, res.at);
    }
}
