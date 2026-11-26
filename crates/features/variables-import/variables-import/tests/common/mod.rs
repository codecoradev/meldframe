//! Helpers of the import tests: run an import, apply its commands to an empty file with the real checks, and look
//! at what came out.
#![allow(dead_code)]

use meldframe_core::apply_doc;
use meldframe_protocol::{Bind, Command, Doc, Value, Variables};
use variables_import::{Entry, EntryKind, Import, ImportOptions, import_css};

pub fn run(text: &str) -> Import {
    import_css(text, &ImportOptions::default()).unwrap()
}

/// The file after the commands, each one applied through the checks of the file.
pub fn applied(commands: &[Command]) -> Doc {
    let mut doc = Doc::default();
    for (i, c) in commands.iter().enumerate() {
        apply_doc(&mut doc, c, &Default::default())
            .unwrap_or_else(|e| panic!("command {i} ({c:?}) is refused: {e:?}"));
    }
    doc
}

/// Import `text` and apply it; panics when a command is refused.
pub fn vars(text: &str) -> Variables {
    applied(&run(text).commands).vars
}

pub fn entries<'a>(i: &'a Import, code: &str) -> Vec<&'a Entry> {
    i.report.entries.iter().filter(|e| e.code == code).collect()
}

pub fn skipped<'a>(i: &'a Import, name: &str) -> Option<&'a Entry> {
    i.report
        .entries
        .iter()
        .find(|e| e.kind == EntryKind::Skipped && e.name == name)
}

/// The literal of variable `name` of collection `collection` in mode `mode`.
pub fn lit(v: &Variables, collection: &str, name: &str, mode: &str) -> Value {
    let var = v
        .variables
        .iter()
        .find(|x| x.collection == format!("col-{collection}") && x.name == name)
        .unwrap_or_else(|| panic!("no variable {collection}/{name}"));
    match var
        .values
        .get(mode)
        .unwrap_or_else(|| panic!("no mode {mode}"))
    {
        Bind::Lit(v) => v.clone(),
        Bind::Var(r) => panic!("{name} is an alias of {}", r.var),
    }
}

/// The name of the variable that variable `name` of `collection` aliases in `mode`.
pub fn alias_of(v: &Variables, collection: &str, name: &str, mode: &str) -> String {
    let var = v
        .variables
        .iter()
        .find(|x| x.collection == format!("col-{collection}") && x.name == name)
        .unwrap_or_else(|| panic!("no variable {collection}/{name}"));
    match var.values.get(mode).unwrap() {
        Bind::Var(r) => v.variable(&r.var).unwrap().name.clone(),
        Bind::Lit(_) => panic!("{name} is a literal"),
    }
}
