//! MM1 and MM2: the shape of a monolith. The root (`src/lib.rs`) is a module list, and every
//! top-level module is a small file or a directory of files, with a one-sentence doc.

use super::tokens::{Tok, close_of, item_end};
use super::{Config, CrateSrc, SrcFile, Violation, is_test_path, product_tokens};

/// Longest `src/lib.rs`, and longest root of a directory module, in lines.
pub const ROOT_LINES: usize = 150;
/// Most top-level modules of a crate.
pub const MAX_MODULES: usize = 40;
/// Longest file of a module.
pub const MODULE_FILE_LINES: usize = 500;

/// A top-level module of a crate: the files that belong to it, tests left out.
pub struct Module<'a> {
    pub name: String,
    /// `src/<name>.rs` or `src/<name>/mod.rs`; `None` when neither exists.
    pub root: Option<&'a SrcFile>,
    /// The root first, then every file under `src/<name>/`.
    pub files: Vec<&'a SrcFile>,
    /// True when the module has a directory of its own.
    pub dir: bool,
}

fn lib_path(k: &CrateSrc) -> String {
    format!("{}/src/lib.rs", k.dir)
}

/// The crates above the threshold that have a `src/lib.rs`.
pub fn monoliths<'a>(crates: &'a [CrateSrc], cfg: &Config) -> Vec<&'a CrateSrc> {
    crates
        .iter()
        .filter(|k| k.total_lines > cfg.threshold && k.files.iter().any(|f| f.path == lib_path(k)))
        .collect()
}

/// What the root of a monolith holds: the lines of items that do not belong there, with what
/// they are, and the names of the files it declares with `mod name;`.
fn scan_root(toks: &[Tok]) -> (Vec<(usize, String)>, Vec<String>) {
    let (mut bad, mut mods) = (Vec::new(), Vec::new());
    let is = |k: usize, s: &str| toks.get(k).is_some_and(|t| t.text == s);
    let mut i = 0;
    while i < toks.len() {
        if is(i, "#") {
            let open = i + 1 + usize::from(is(i + 1, "!"));
            if is(open, "[") {
                i = close_of(toks, open) + 1;
                continue;
            }
        }
        let mut k = i;
        if is(k, "pub") {
            k += 1;
            if is(k, "(") {
                k = close_of(toks, k) + 1;
            }
        }
        let kw = toks.get(k).map_or("", |t| t.text.as_str());
        let next = match kw {
            "use" | "extern" => semicolon_after(toks, k),
            "mod" if is(k + 2, ";") => {
                mods.push(toks[k + 1].text.clone());
                k + 3
            }
            _ => {
                let what = if kw == "mod" { "an inline module" } else { kw };
                bad.push((toks[i].line, what.to_string()));
                item_end(toks, k.max(i))
            }
        };
        i = next.max(i + 1);
    }
    (bad, mods)
}

/// The index after the next `;` outside brackets.
fn semicolon_after(toks: &[Tok], from: usize) -> usize {
    let mut k = from;
    while k < toks.len() {
        match toks[k].text.as_str() {
            ";" => return k + 1,
            "(" | "[" | "{" => k = close_of(toks, k),
            _ => {}
        }
        k += 1;
    }
    toks.len()
}

/// The top-level modules declared by `src/lib.rs`.
pub fn modules(k: &CrateSrc) -> Vec<Module<'_>> {
    let Some(lib) = k.files.iter().find(|f| f.path == lib_path(k)) else {
        return Vec::new();
    };
    let src = format!("{}/src", k.dir);
    let find = |path: &str| k.files.iter().find(|f| f.path == path);
    scan_root(&product_tokens(lib))
        .1
        .into_iter()
        .map(|name| {
            let dir_root = find(&format!("{src}/{name}/mod.rs"));
            let root = dir_root.or_else(|| find(&format!("{src}/{name}.rs")));
            let prefix = format!("{src}/{name}/");
            let mut files: Vec<&SrcFile> = root.into_iter().collect();
            files.extend(
                k.files
                    .iter()
                    .filter(|f| f.path.starts_with(&prefix) && !is_test_path(&f.path))
                    .filter(|f| Some(&f.path) != root.map(|r| &r.path)),
            );
            Module {
                dir: dir_root.is_some() || files.len() > usize::from(root.is_some()),
                name,
                root,
                files,
            }
        })
        .collect()
}

/// The first paragraph of the `//!` doc at the top of a file, when it is one sentence: it ends
/// with a full stop and holds no other.
fn doc_sentence(text: &str) -> Option<String> {
    let docs: Vec<String> = text
        .lines()
        .skip_while(|l| l.trim().is_empty())
        .take_while(|l| l.trim_start().starts_with("//!"))
        .map(|l| l.trim_start()[3..].trim().to_string())
        .take_while(|l| !l.is_empty())
        .collect();
    let sentence = docs.join(" ");
    (sentence.ends_with('.') && !sentence.contains(". ")).then_some(sentence)
}

/// Findings about a root file: too long, or an item that is not a module list entry.
fn root_findings(k: &CrateSrc, file: &SrcFile, rule: &'static str, what: &str) -> Vec<Violation> {
    let mut out = Vec::new();
    let lines = file.text.lines().count();
    if lines > ROOT_LINES {
        out.push(Violation::new(
            &k.name,
            &file.path,
            0,
            rule,
            format!("{what} has {lines} lines; at most {ROOT_LINES}"),
        ));
    }
    for (line, item) in scan_root(&product_tokens(file)).0 {
        out.push(Violation::new(
            &k.name,
            &file.path,
            line,
            rule,
            format!(
                "{what} holds {item}; only docs, `use`, `mod`, `pub use` and attributes belong there"
            ),
        ));
    }
    out
}

fn module_findings(k: &CrateSrc, m: &Module) -> Vec<Violation> {
    let at = |path: &str, msg: String| Violation::new(&k.name, path, 0, "MM2", msg);
    let Some(root) = m.root else {
        return vec![at(
            &lib_path(k),
            format!(
                "`mod {};` has no file (src/{0}.rs or src/{0}/mod.rs)",
                m.name
            ),
        )];
    };
    let mut out = Vec::new();
    if m.dir {
        if m.files.len() < 2 {
            out.push(at(
                &root.path,
                format!(
                    "module `{}` is a directory of one file; a directory has at least 2 files",
                    m.name
                ),
            ));
        }
        let what = format!("the root of module `{}`", m.name);
        out.extend(root_findings(k, root, "MM2", &what));
    } else {
        let lines = root.text.lines().count();
        if lines > MODULE_FILE_LINES {
            out.push(at(
                &root.path,
                format!("module `{}` is a file of {lines} lines; at most {MODULE_FILE_LINES}, or make it a directory", m.name),
            ));
        }
    }
    if doc_sentence(&root.text).is_none() {
        out.push(at(
            &root.path,
            format!(
                "module `{}` must start with a one-sentence `//!` doc naming its capability",
                m.name
            ),
        ));
    }
    out
}

pub fn check(crates: &[CrateSrc], cfg: &Config) -> Vec<Violation> {
    let mut out = Vec::new();
    for k in monoliths(crates, cfg) {
        let lib_path = lib_path(k);
        let Some(lib) = k.files.iter().find(|f| f.path == lib_path) else {
            continue;
        };
        out.extend(root_findings(k, lib, "MM1", "src/lib.rs"));
        let mods = modules(k);
        if mods.len() > MAX_MODULES {
            out.push(Violation::new(
                &k.name,
                &lib_path,
                0,
                "MM2",
                format!("{} top-level modules; at most {MAX_MODULES}", mods.len()),
            ));
        }
        out.extend(mods.iter().flat_map(|m| module_findings(k, m)));
    }
    out
}

/// One line for each module of each monolith: `crate::module: sentence (files, lines)`.
pub fn list(crates: &[CrateSrc], cfg: &Config) -> Vec<String> {
    let mut out = Vec::new();
    for k in monoliths(crates, cfg) {
        for m in modules(k) {
            let sentence = m
                .root
                .and_then(|r| doc_sentence(&r.text))
                .unwrap_or_else(|| "(no one-sentence doc)".into());
            let lines: usize = m.files.iter().map(|f| f.text.lines().count()).sum();
            out.push(format!(
                "{}::{}: {sentence} ({} files, {lines} lines)",
                k.name,
                m.name,
                m.files.len()
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests;
