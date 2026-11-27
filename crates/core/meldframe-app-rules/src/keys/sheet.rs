//! The table written as the groups of the shortcut sheet, for a platform: the same words, the same ids and the same keys as
//! `shortcutGroups` and `sheetGroups` of `packages/web-shell/src/shortcuts.ts`, so that a front end can read the sheet from here
//! instead of keeping a table of its own.

use super::table::{Combo, Held, TABLE};

/// One row of the sheet, with every key written for the platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetRow {
    pub id: &'static str,
    pub name: &'static str,
    pub keys: Vec<String>,
}

/// A titled group of rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetGroup {
    pub id: &'static str,
    pub title: &'static str,
    pub rows: Vec<SheetRow>,
}

/// The word or sign a key is written as.
fn word(key: &str) -> &str {
    match key {
        "Escape" => "Esc",
        "ArrowUp" => "\u{2191}",
        "ArrowDown" => "\u{2193}",
        "ArrowLeft" => "\u{2190}",
        "ArrowRight" => "\u{2192}",
        other => other,
    }
}

/// A combo the way the platform writes it: symbols in front of the key on a Mac (Shift and Command before the Z), words joined
/// by `+` elsewhere (`Ctrl+Shift+Z`).
pub fn combo_label(combo: &Combo, mac: bool) -> String {
    let key = word(combo.key);
    let (alt, shift) = (combo.alt == Held::Yes, combo.shift == Held::Yes);
    if mac {
        let marks = [
            (alt, "\u{2325}"),
            (shift, "\u{21E7}"),
            (combo.command, "\u{2318}"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, mark)| *mark)
        .collect::<String>();
        let gap = if !marks.is_empty() && key.chars().count() != 1 {
            " "
        } else {
            ""
        };
        return format!("{marks}{gap}{key}");
    }
    let parts = [(combo.command, "Ctrl"), (alt, "Alt"), (shift, "Shift")];
    let mut out: Vec<&str> = parts
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, w)| *w)
        .collect();
    out.push(key);
    out.join("+")
}

/// The groups of the sheet written for the platform. A row that needs "Auto layout on canvas" is there only while it is on.
pub fn sheet(mac: bool, auto_layout: bool) -> Vec<SheetGroup> {
    TABLE
        .iter()
        .map(|g| SheetGroup {
            id: g.id,
            title: g.title,
            rows: g
                .rows
                .iter()
                .filter(|r| auto_layout || !r.needs_auto_layout)
                .map(|r| SheetRow {
                    id: r.id,
                    name: r.name,
                    keys: r
                        .bindings
                        .iter()
                        .map(|b| combo_label(&b.combo, mac))
                        .collect(),
                })
                .collect(),
        })
        .collect()
}
