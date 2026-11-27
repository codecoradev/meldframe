//! Shared helpers of the palette tests: the editor state the TypeScript tests gave the page, and the lists built from it.
#![allow(dead_code)]

use meldframe_app::{AppState, ExportTarget};
use meldframe_app_rules::palette::{Entry, Run, entries, filter};

/// What the page told the palette in the TypeScript tests: the panels and what the editor can do now.
#[derive(Clone, Copy)]
pub struct Flags {
    pub layers_open: bool,
    pub inspector_open: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub can_delete: bool,
    /// A frame is selected: the one flag Export and Make a variant both read.
    pub can_export: bool,
}

pub const ON: Flags = Flags {
    layers_open: false,
    inspector_open: true,
    can_undo: true,
    can_redo: false,
    can_delete: true,
    can_export: true,
};

pub fn state(f: Flags) -> AppState {
    let mut s = AppState::default();
    s.panels.layers = f.layers_open;
    s.panels.properties = f.inspector_open;
    s.history.can_undo = f.can_undo;
    s.history.can_redo = f.can_redo;
    if f.can_delete {
        s.selection = vec!["f1".into()];
    }
    if f.can_export {
        s.export.target = Some(ExportTarget {
            id: "f1".into(),
            name: "Home".into(),
            file_stem: "home".into(),
        });
    }
    s
}

pub fn list(f: Flags, mac: bool, switch: Option<bool>) -> Vec<Entry> {
    entries(&state(f), mac, switch)
}

pub fn by<'a>(es: &'a [Entry], id: &str) -> &'a Entry {
    es.iter().find(|e| e.id == id).unwrap()
}

pub fn ids(es: &[Entry]) -> Vec<&str> {
    es.iter().map(|e| e.id.as_str()).collect()
}

pub fn found(es: &[Entry], q: &str) -> Vec<String> {
    filter(es, q).into_iter().map(|e| e.id).collect()
}

pub fn custom(id: &str, label: &str, keywords: &'static [&'static str]) -> Entry {
    Entry {
        id: id.into(),
        label: label.into(),
        group: "G",
        shortcut: None,
        detail: None,
        enabled: true,
        keywords,
        opens_dialog: false,
        run: Run::Undo,
    }
}
