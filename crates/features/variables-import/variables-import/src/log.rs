//! The report of an import while it is being made: entries with a bound, and the lines about conversions and notes.

use crate::plan::Prop;
use crate::scan::{Scan, Section};
use crate::text::{clean, short};
use crate::value::{Conversion, Val};
use crate::{Entry, EntryKind, MAX_ENTRIES, MAX_REPORT_BYTES};

#[derive(Default)]
pub(crate) struct Log {
    pub entries: Vec<Entry>,
    pub omitted: usize,
    pub skipped: usize,
    /// The bytes of the entries pushed so far (names, messages and codes).
    bytes: usize,
}

/// The longest message of an entry, in characters.
const MAX_MESSAGE: usize = 240;

fn entry(kind: EntryKind, code: &'static str, name: &str, message: impl Into<String>) -> Entry {
    Entry {
        kind,
        code,
        name: short(name),
        message: clean(&message.into(), MAX_MESSAGE),
    }
}

impl Log {
    /// A note about the whole import goes first and is never dropped: a full list gives up its last line for it, so the
    /// list stays within [`MAX_ENTRIES`].
    pub fn front(
        &mut self,
        kind: EntryKind,
        code: &'static str,
        name: &str,
        message: impl Into<String>,
    ) {
        self.entries.insert(0, entry(kind, code, name, message));
        if self.entries.len() > MAX_ENTRIES {
            self.entries.pop();
            self.omitted += 1;
        }
    }

    /// An entry, unless the list is full in lines or in bytes (`MAX_REPORT_BYTES`): then it is counted as omitted.
    pub fn push(
        &mut self,
        kind: EntryKind,
        code: &'static str,
        name: &str,
        message: impl Into<String>,
    ) {
        if self.entries.len() >= MAX_ENTRIES || self.bytes >= MAX_REPORT_BYTES {
            self.omitted += 1;
            return;
        }
        let e = entry(kind, code, name, message);
        let size = e.name.len() + e.message.len() + e.code.len();
        if self.entries.len() < MAX_ENTRIES && self.bytes + size <= MAX_REPORT_BYTES {
            self.bytes += size;
            self.entries.push(e);
        } else {
            self.omitted += 1;
        }
    }
}

pub(crate) fn section_name(s: Section) -> &'static str {
    match s {
        Section::Theme => "theme",
        Section::Root => "root",
    }
}

pub(crate) fn kill(p: &mut Prop, log: &mut Log, code: &'static str, message: impl Into<String>) {
    if !p.dead {
        p.dead = true;
        log.skipped += 1;
        log.push(EntryKind::Skipped, code, &p.label, message);
    }
}

pub(crate) fn conversions(props: &[Prop], log: &mut Log) {
    for p in props.iter().filter(|p| !p.dead) {
        for (mode, s) in [("", &p.light), ("dark: ", &p.dark)]
            .into_iter()
            .filter_map(|(m, s)| s.as_ref().map(|s| (m, s)))
        {
            match &s.val {
                Val::Color {
                    hex,
                    how: Some(Conversion::Colour),
                } => log.push(
                    EntryKind::Converted,
                    "converted",
                    &p.raw,
                    format!("{mode}{} became {hex}", short(&s.text)),
                ),
                Val::Color {
                    hex,
                    how: Some(Conversion::Clipped),
                } => log.push(
                    EntryKind::Converted,
                    "clipped",
                    &p.raw,
                    format!(
                        "{mode}{} is outside sRGB and was clamped to {hex}",
                        short(&s.text)
                    ),
                ),
                Val::Number {
                    value,
                    how: Some(Conversion::Rem),
                } => log.push(
                    EntryKind::Converted,
                    "rem",
                    &p.raw,
                    format!("{mode}{} became {value} (1rem is 16)", short(&s.text)),
                ),
                Val::Alias {
                    target,
                    fallback: true,
                } => log.push(
                    EntryKind::Converted,
                    "fallback_ignored",
                    &p.raw,
                    format!("{mode}the fallback of var({target}, ...) is not kept"),
                ),
                _ => {}
            }
        }
    }
}

pub(crate) fn notes(scan: &Scan, props: &[Prop], inherited: usize, log: &mut Log) {
    let mut sources: Vec<&str> = Vec::new();
    for p in props.iter().filter(|p| !p.dead && p.dark.is_some()) {
        let label = scan.sources[p.dark_source as usize].as_str();
        if !sources.contains(&label) {
            sources.push(label);
        }
    }
    if !sources.is_empty() {
        let list = sources
            .iter()
            .take(5)
            .copied()
            .collect::<Vec<_>>()
            .join(" | ");
        log.front(
            EntryKind::Note,
            "dark_mode_source",
            "dark",
            format!("the dark mode is read from: {}; the model keeps no selector, the export picks its own", short(&list)),
        );
    }
    if inherited > 0 {
        log.front(
            EntryKind::Note,
            "inherited_dark",
            "dark",
            format!("{inherited} variable(s) have no dark value and take their light value in the dark mode"),
        );
    }
    if scan.other_properties > 0 {
        log.front(
            EntryKind::Note,
            "other_declarations",
            "",
            format!(
                "{} declaration(s) of properties that are not custom properties were not read",
                scan.other_properties
            ),
        );
    }
    if scan.too_many {
        log.front(
            EntryKind::Skipped,
            "limit_declarations",
            "",
            "more custom properties than an import reads; the rest were left out",
        );
    }
    if scan.unterminated {
        log.front(
            EntryKind::Note,
            "unterminated",
            "",
            "the text ends inside a string, a comment or a block; what follows was not read",
        );
    }
}
