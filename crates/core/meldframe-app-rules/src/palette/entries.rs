//! The catalogue: every entry the editor has, in the order the palette lists them when nothing is typed.
//!
//! The words are the palette's own and fixed (U5), so no label carries text from a file (U9): a frame's name never reaches one.
//! The keys shown are read from the key table through the sheet's writer, never written here, and a thing with no key has none.

use crate::keys::sheet::combo_label;
use crate::keys::table::TABLE;
use meldframe_app_model::{AppState, FramePreset, Panel, Tool};

/// A change to the zoom: the commands of the zoom menu, which the camera runs (`Action::Camera`).
pub use meldframe_app_model::ZoomCommand as Zoom;

/// What running an entry makes, as data: `plan` turns it into what the app does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    Tool(Tool),
    Undo,
    Redo,
    /// Delete the selection.
    DeleteSelection,
    /// A new top-level frame of the preset, centred on the middle of the view.
    NewFrame(FramePreset),
    Zoom(Zoom),
    /// Turn "Auto layout on canvas" on or off.
    AutoLayout,
    TogglePanel(Panel),
    /// Open the Export dialog on the selected frame.
    Export,
    /// Copy the selected frame beside it as a variant.
    Variant,
    /// Open the shortcut sheet.
    Help,
}

/// One row of the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub label: String,
    pub group: &'static str,
    /// The first key the sheet lists for it, written for the platform.
    pub shortcut: Option<String>,
    /// More text for the row, such as the size of a preset; it is searched too.
    pub detail: Option<String>,
    /// False while the editor cannot do it (nothing to undo, nothing selected).
    pub enabled: bool,
    /// Words that find it besides its label.
    pub keywords: &'static [&'static str],
    /// It opens a dialog, which takes the focus: the palette does not give it back.
    pub opens_dialog: bool,
    pub run: Run,
}

/// The first key of a row of the sheet, written for the platform.
fn key(row: &str, mac: bool) -> Option<String> {
    let row = TABLE.iter().flat_map(|g| g.rows).find(|r| r.id == row)?;
    row.bindings.first().map(|b| combo_label(&b.combo, mac))
}

fn preset_id(p: FramePreset) -> &'static str {
    match p {
        FramePreset::Desktop => "desktop",
        FramePreset::DesktopHd => "desktopHd",
        FramePreset::Tablet => "tablet",
        FramePreset::Mobile => "mobile",
        FramePreset::PhoneSmall => "phoneSmall",
        FramePreset::A4 => "a4",
        FramePreset::Letter => "letter",
    }
}

/// A zoom entry: id, label, what it makes, the row of the sheet that gives its key, and its keywords.
type ZoomRow = (
    &'static str,
    &'static str,
    Zoom,
    Option<&'static str>,
    &'static [&'static str],
);

/// The entries for the state of the editor. `auto_layout` is `Some(on)` when the host has the switch "Auto layout on canvas" (the
/// entry says what is on) and `None` for a host without it, which leaves the entry out.
pub fn entries(state: &AppState, mac: bool, auto_layout: Option<bool>) -> Vec<Entry> {
    let entry = |group, id: &str, label: &str, enabled, run| Entry {
        id: id.into(),
        label: label.into(),
        group,
        shortcut: None,
        detail: None,
        enabled,
        keywords: &[],
        opens_dialog: false,
        run,
    };
    let keyed = |mut e: Entry, row: &str, words: &'static [&'static str]| {
        e.shortcut = key(row, mac);
        e.keywords = words;
        e
    };
    let has_target = state.export.target.is_some();
    let mut out = Vec::new();
    let tools: [(&str, &str, Tool, &[&str]); 5] = [
        ("tool-select", "Select tool", Tool::Select, &[]),
        ("tool-frame", "Frame tool", Tool::Frame, &[]),
        ("tool-rectangle", "Rectangle tool", Tool::Rectangle, &[]),
        (
            "tool-ellipse",
            "Ellipse tool",
            Tool::Ellipse,
            &["circle", "oval"],
        ),
        ("tool-text", "Text tool", Tool::Text, &[]),
    ];
    for (id, label, tool, words) in tools {
        out.push(keyed(
            entry("Tools", id, label, true, Run::Tool(tool)),
            id,
            words,
        ));
    }
    let h = &state.history;
    out.push(keyed(
        entry("Edit", "undo", "Undo", h.can_undo, Run::Undo),
        "undo",
        &[],
    ));
    out.push(keyed(
        entry("Edit", "redo", "Redo", h.can_redo, Run::Redo),
        "redo",
        &[],
    ));
    let delete = entry(
        "Edit",
        "delete",
        "Delete the selection",
        !state.selection.is_empty(),
        Run::DeleteSelection,
    );
    out.push(keyed(delete, "delete", &["remove"]));
    for p in FramePreset::ALL {
        let (w, h) = p.size();
        let id = format!("preset-{}", preset_id(p));
        let mut e = entry(
            "Frames",
            &id,
            &format!("New frame: {}", p.name()),
            true,
            Run::NewFrame(p),
        );
        e.detail = Some(format!("{w} \u{d7} {h}"));
        e.keywords = &["preset", "create", "add"];
        out.push(e);
    }
    let zooms: [ZoomRow; 5] = [
        ("zoom-in", "Zoom in", Zoom::In, Some("zoom-in"), &["closer"]),
        (
            "zoom-out",
            "Zoom out",
            Zoom::Out,
            Some("zoom-out"),
            &["further"],
        ),
        (
            "zoom-reset",
            "Zoom to 100%",
            Zoom::Reset,
            None,
            &["reset", "actual size"],
        ),
        (
            "zoom-fit",
            "Fit all",
            Zoom::Fit,
            Some("fit-all"),
            &["fit", "everything"],
        ),
        (
            "zoom-fit-selection",
            "Zoom to selection",
            Zoom::FitSelection,
            Some("fit-selection"),
            &["fit", "focus"],
        ),
    ];
    for (id, label, zoom, row, words) in zooms {
        let mut e = entry("View", id, label, true, Run::Zoom(zoom));
        e.shortcut = row.and_then(|r| key(r, mac));
        e.keywords = words;
        out.push(e);
    }
    if let Some(on) = auto_layout {
        let mut e = entry(
            "View",
            "auto-layout",
            "Auto layout on canvas",
            true,
            Run::AutoLayout,
        );
        e.detail = Some(
            if on {
                "On: choose to turn off"
            } else {
                "Off: choose to turn on"
            }
            .into(),
        );
        e.keywords = &["layout", "flow", "hug", "fill", "toggle", "switch"];
        out.push(e);
    }
    let p = &state.panels;
    let shown = |open| if open { "Hide" } else { "Show" };
    let layers = entry(
        "Panels",
        "toggle-layers",
        &format!("{} layers panel", shown(p.layers)),
        true,
        Run::TogglePanel(Panel::Layers),
    );
    out.push(keyed(layers, "layers-panel", &["pages", "toggle"]));
    let right = entry(
        "Panels",
        "toggle-inspector",
        &format!("{} right panel", shown(p.properties)),
        true,
        Run::TogglePanel(Panel::Properties),
    );
    out.push(keyed(
        right,
        "right-panel",
        &["design", "inspector", "properties", "toggle"],
    ));
    let mut export = entry("File", "export", "Export\u{2026}", has_target, Run::Export);
    export.keywords = &["png", "html", "tailwind", "image"];
    export.opens_dialog = true;
    out.push(export);
    let mut variant = entry(
        "File",
        "variant",
        "Make a variant",
        has_target,
        Run::Variant,
    );
    variant.keywords = &["copy", "duplicate"];
    out.push(variant);
    let mut help = keyed(
        entry(
            "Help",
            "shortcuts",
            "Show keyboard shortcuts",
            true,
            Run::Help,
        ),
        "shortcuts",
        &["keys", "help"],
    );
    help.opens_dialog = true;
    out.push(help);
    out
}
