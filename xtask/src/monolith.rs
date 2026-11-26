//! `mm-check`: the modular monolith rule of D1 (`docs/standards/modular-thin-ui.md`) as a check.
//! The rules are pure functions over crate sources; this module holds their shared types, reads
//! the workspace and runs them. The limits are in `xtask/monolith.toml` and are listed here.
//!
//! - MM3 no copies: two files with a function of the same body (25 tokens or more, tests left
//!   out) fail, in any crate, unless `[[allow_copy]]` lists the group with a reason.
//! - MM4 shared helpers: a banned pattern of a `[shared.<name>]` rule outside its home fails with
//!   "use <home>", unless `[allow_shared.<name>]` lists the file with a reason. A pattern is Rust
//!   source compared token by token: hex digits (`0x200b`, `\u{feff}`) without regard to case, and
//!   an `_` stands for any one identifier. A file is reported once per rule.
//!
//! Every allow-list entry carries a reason and only goes down: an entry that no longer matches
//! fails as not needed, and `--base <ref>` rejects an entry that is not on the base branch. An
//! entry may shrink and a file may move: a group in `[[allow_copy]]` is accepted when each of its
//! members matches a member of one base entry, by `path::fn` or, for a file that moved, by the
//! function name (so deleting one copy of three is progress, and a group that gains a member is
//! not); a file in `[allow_shared.<name>]` matches a base entry by path or else by file name; each
//! base entry covers one group or file. Two groups with the same member ids need two entries.
//!
//! What it does not do. MM3 finds exact copies only: the same rule written twice with another
//! name for a variable, another error type or another loop is not found. MM4 is only as good as
//! the pattern list somebody keeps up to date. None of it checks that a shared helper is a good
//! one, only that there is one. A crate is read as text: macros are not expanded.

mod config;
mod copies;
mod shared;
mod tokens;

use std::fmt;
use std::path::Path;
use std::process::ExitCode;

use crate::rules::Package;
use crate::{scan, size};

pub use config::{CONFIG_FILE, Config, check_not_raised, parse};

/// One crate's Rust sources.
#[derive(Debug, Clone, Default)]
pub struct CrateSrc {
    pub name: String,
    pub files: Vec<SrcFile>,
}

/// A `.rs` file: workspace-relative path with `/` separators, and its text.
#[derive(Debug, Clone)]
pub struct SrcFile {
    pub path: String,
    pub text: String,
}

/// One broken rule. `path` is empty and `line` 0 when the finding is about a crate or the list.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Violation {
    pub krate: String,
    pub path: String,
    pub line: usize,
    pub rule: &'static str,
    pub message: String,
}

impl Violation {
    pub fn new(krate: &str, path: &str, line: usize, rule: &'static str, msg: String) -> Self {
        Violation {
            krate: krate.into(),
            path: path.into(),
            line,
            rule,
            message: msg,
        }
    }

    /// A finding about an allow-list entry, not about a crate.
    pub fn entry(rule: &'static str, msg: String) -> Self {
        Violation::new(CONFIG_FILE, "", 0, rule, msg)
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.krate)?;
        match (self.path.as_str(), self.line) {
            ("", _) => {}
            (p, 0) => write!(f, "{p}: ")?,
            (p, l) => write!(f, "{p}:{l}: ")?,
        }
        write!(f, "{}: {}", self.rule, self.message)
    }
}

/// True for a file that only exists for tests: under a `tests`, `benches`, `examples` or
/// `scripts` directory, or named `tests.rs`.
pub fn is_test_path(path: &str) -> bool {
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        let last = parts.peek().is_none();
        if !last && matches!(part, "tests" | "benches" | "examples" | "scripts") {
            return true;
        }
        if last && part == "tests.rs" {
            return true;
        }
    }
    false
}

/// The tokens of a file without its test code.
pub(crate) fn product_tokens(file: &SrcFile) -> Vec<tokens::Tok> {
    tokens::strip_tests(tokens::tokenize(&file.text))
}

/// Run the source rules over every crate and sort the findings.
pub fn check(crates: &[CrateSrc], cfg: &Config) -> Vec<Violation> {
    let mut out = copies::check(crates, cfg);
    out.extend(shared::check(crates, cfg));
    out.sort();
    out
}

/// Read the Rust sources of every package.
fn load(root: &Path, packages: &[Package]) -> Vec<CrateSrc> {
    let files = |p: &Package| {
        scan::files(root, &p.dir, &["rs"])
            .into_iter()
            .map(|path| SrcFile {
                text: scan::read(root, &path),
                path,
            })
            .collect()
    };
    packages
        .iter()
        .map(|p| CrateSrc {
            name: p.name.clone(),
            files: files(p),
        })
        .collect()
}

/// `mm-check [--base <git-ref>]`: the rules over the workspace, and with `--base` that the
/// allow-lists did not grow. A configuration that is not on the base yet is being introduced.
pub fn run(packages: &[Package], args: &[String]) -> Result<ExitCode, String> {
    crate::args::check_flags("mm-check", args, &["--base"])?;
    let root = scan::workspace_root();
    let cfg = parse(&scan::read(&root, CONFIG_FILE))?;
    let crates = load(&root, packages);
    let mut found = check(&crates, &cfg);
    if let Some(base) = size::base_arg(args).map_err(|e| e.replace("size-check", "mm-check"))? {
        match scan::base_file(&root, base, CONFIG_FILE)? {
            Some(text) => found.extend(check_not_raised(&parse(&text)?, &cfg)),
            None => println!(
                "mm-check: note: {CONFIG_FILE} is not on {base}; the lists are being introduced"
            ),
        }
    }
    Ok(crate::report(
        "mm-check",
        &format!("{} crates", crates.len()),
        &found,
    ))
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    /// A crate at `crates/<name>` with files given as (path under the crate, text).
    pub fn krate(name: &str, files: &[(&str, &str)]) -> CrateSrc {
        let dir = format!("crates/{name}");
        CrateSrc {
            name: name.into(),
            files: files
                .iter()
                .map(|(p, t)| SrcFile {
                    path: format!("{dir}/{p}"),
                    text: (*t).into(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_are_tests_benches_examples_scripts_and_tests_rs() {
        for p in [
            "crates/a/tests/x.rs",
            "crates/a/src/tests.rs",
            "crates/a/benches/b.rs",
            "crates/a/examples/e.rs",
            "crates/a/src/x/tests/y.rs",
        ] {
            assert!(is_test_path(p), "{p}");
        }
        for p in [
            "crates/a/src/lib.rs",
            "crates/core/meldframe-app-tests/src/lib.rs",
            "crates/a/src/tests",
            "crates/a/src/my_tests.rs",
        ] {
            assert!(!is_test_path(p), "{p}");
        }
    }

    #[test]
    fn a_violation_prints_crate_path_line_rule_and_message() {
        let v = Violation::new("c", "a/b.rs", 7, "MM4", "msg".into());
        assert_eq!(v.to_string(), "c: a/b.rs:7: MM4: msg");
        let v = Violation::new("c", "a/b.rs", 0, "MM4", "msg".into());
        assert_eq!(v.to_string(), "c: a/b.rs: MM4: msg");
        let v = Violation::entry("MM4", "msg".into());
        assert_eq!(v.to_string(), format!("{CONFIG_FILE}: MM4: msg"));
    }
}
