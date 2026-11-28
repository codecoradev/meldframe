//! C10 Dependencies (section 11 of the spike note): the exact pins, the git sources of the lock, the patches that remain, and
//! with `--checks` the licence checks and with `--build` the clean and the incremental release build and the size of the binary.
//! The pins are read from the files of the Rune workspace; a command that cannot run here is said to be skipped.

use crate::files::workspace;
use crate::report::Report;
use meldframe_rune_pin::{RUNE_REVISION, RUNE_URL};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// The repository root.
fn repository() -> PathBuf {
    workspace().join("../..")
}

/// The names in `[patch.crates-io]` of the manifest text and every `rev = "..."` of the non-comment lines.
pub fn pins(manifest: &str) -> (Vec<String>, Vec<String>) {
    let (mut patches, mut revs) = (Vec::new(), Vec::new());
    let mut in_patch = false;
    for line in manifest
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
    {
        if line.starts_with('[') {
            in_patch = line == "[patch.crates-io]";
        } else if in_patch && let Some((name, _)) = line.split_once('=') {
            patches.push(name.trim().to_owned());
        }
        if let Some(at) = line.find("rev = \"") {
            let rest = &line[at + 7..];
            revs.push(rest.split('"').next().unwrap_or_default().to_owned());
        }
    }
    (patches, revs)
}

/// The distinct git sources of a lock file, as `url rev` (the revision after `?rev=`).
pub fn git_sources(lock: &str) -> Vec<String> {
    let mut found: Vec<String> = lock
        .lines()
        .filter_map(|l| l.trim().strip_prefix("source = \"git+"))
        .filter_map(|s| s.split('"').next())
        .map(|s| s.split('#').next().unwrap_or(s).replace("?rev=", " "))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// Runs `program args` in `dir` with the variables `set` and without the variables `unset`; the exit status word and the
/// seconds it took.
fn timed(
    dir: &Path,
    program: &str,
    args: &[&str],
    set: &[(&str, &Path)],
    unset: &[&str],
) -> (String, f64) {
    let t = Instant::now();
    let mut c = Command::new(program);
    c.args(args).current_dir(dir);
    for (k, v) in set {
        c.env(k, v);
    }
    for k in unset {
        c.env_remove(k);
    }
    let status = match c.status() {
        Ok(s) if s.success() => "ok".to_owned(),
        Ok(s) => format!("exit_{}", s.code().unwrap_or(-1)),
        Err(e) => format!("cannot_run_{}", e.kind().to_string().replace(' ', "_")),
    };
    (status, t.elapsed().as_secs_f64())
}

/// Runs `program args` in `dir` and keeps the last line it printed (the summary a check ends with): the status word, that line
/// and the seconds.
fn captured(dir: &Path, program: &str, args: &[&str]) -> (String, String, f64) {
    let t = Instant::now();
    match Command::new(program).args(args).current_dir(dir).output() {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).into_owned()
                + &String::from_utf8_lossy(&o.stderr);
            let last = text
                .lines()
                .rev()
                .find(|l| l.contains(':') && !l.trim().is_empty())
                .or_else(|| text.lines().rev().find(|l| !l.trim().is_empty()))
                .unwrap_or("-")
                .to_owned();
            let status = if o.status.success() {
                "ok".to_owned()
            } else {
                format!("exit_{}", o.status.code().unwrap_or(-1))
            };
            (status, last, t.elapsed().as_secs_f64())
        }
        Err(e) => (
            format!("cannot_run_{}", e.kind().to_string().replace(' ', "_")),
            "-".into(),
            t.elapsed().as_secs_f64(),
        ),
    }
}

/// Runs C10. `checks` runs the licence commands and `build` the release builds, in a target directory of their own under `out`;
/// `cold` switches the compiler cache off for them.
pub fn run(checks: bool, build: bool, cold: bool, out: &Path) -> Report {
    let mut r = Report::new("C10");
    let manifest = std::fs::read_to_string(workspace().join("Cargo.toml")).unwrap_or_default();
    let lock = std::fs::read_to_string(workspace().join("Cargo.lock")).unwrap_or_default();
    let (patches, revs) = pins(&manifest);
    let all_pinned = !revs.is_empty() && revs.iter().all(|v| v == RUNE_REVISION);
    r.word("c10.rune_revision", RUNE_REVISION, &[("url", RUNE_URL)])
        .int(
            "c10.manifest_rev_lines",
            revs.len() as i64,
            "count",
            &[("expected", "9")],
        )
        .word(
            "c10.manifest_revs_equal_the_pin",
            if all_pinned { "yes" } else { "no" },
            &[],
        );
    for p in &patches {
        r.word(
            "c10.patch",
            p,
            &[("rev", if all_pinned { "the_pin" } else { "differs" })],
        );
    }
    r.int(
        "c10.patches",
        patches.len() as i64,
        "count",
        &[("expected", "8")],
    )
    .int(
        "c10.patches_beyond_the_8",
        patches.len() as i64 - 8,
        "count",
        &[],
    );
    let sources = git_sources(&lock);
    for s in &sources {
        r.word("c10.lock_git_source", s, &[]);
    }
    r.int("c10.lock_git_sources", sources.len() as i64, "count", &[]);
    let mut deny_missing = false;
    let mut ok = all_pinned && patches.len() == 8 && revs.len() == 9 && sources.len() == 1;
    if checks {
        for (name, args) in [
            ("c10.cargo_xtask_deps_check", &["xtask", "deps-check"][..]),
            (
                "c10.cargo_xtask_licences_check",
                &["xtask", "licences", "--check"][..],
            ),
        ] {
            let (status, says, secs) = captured(&repository(), "cargo", args);
            ok &= status == "ok";
            r.word(
                name,
                &status,
                &[("seconds", &format!("{secs:.1}")), ("says", &says)],
            );
        }
        let have_deny = Command::new("cargo")
            .args(["deny", "--version"])
            .current_dir(workspace())
            .output()
            .is_ok_and(|o| o.status.success());
        if have_deny {
            let (deny, secs) = timed(&workspace(), "cargo", &["deny", "check"], &[], &[]);
            ok &= deny == "ok";
            r.word(
                "c10.cargo_deny_check",
                &deny,
                &[("seconds", &format!("{secs:.1}"))],
            );
        } else {
            deny_missing = true;
            r.word(
                "c10.cargo_deny_check",
                "not_run",
                &[("reason", "cargo-deny_is_not_installed_cargo_install_cargo-deny_--version_0.22.2_--locked")],
            );
        }
    } else {
        r.word(
            "c10.licence_commands",
            "not_run",
            &[("reason", "pass_--checks")],
        );
    }
    if build {
        ok &= builds(&mut r, out, cold);
    } else {
        r.word("c10.build", "not_run", &[("reason", "pass_--build")]);
    }
    let complete = checks && build && !deny_missing;
    r.status(
        if complete { "ran" } else { "partial" },
        if complete {
            ""
        } else {
            "pass --checks and --build for the licence commands and the builds"
        },
    );
    r.verdict(if complete && ok {
        "green"
    } else if complete {
        "gap"
    } else {
        "open"
    });
    r
}

/// The compiler cache the environment names (`RUSTC_WRAPPER`), if any: a build with one is not a cold build.
fn compiler_cache() -> Option<String> {
    let wrapper = std::env::var("RUSTC_WRAPPER")
        .ok()
        .filter(|w| !w.is_empty())?;
    Some(
        Path::new(&wrapper)
            .file_name()
            .map_or(wrapper.clone(), |n| n.to_string_lossy().into_owned()),
    )
}

/// The size of `path` in bytes, or none.
fn size_of(path: &Path) -> Option<i64> {
    std::fs::metadata(path).ok().map(|m| m.len() as i64)
}

/// A number, or the word `unavailable` and no number when it could not be read.
fn size_line(r: &mut Report, name: &str, size: Option<i64>, labels: &[(&str, &str)]) {
    match size {
        Some(n) => r.int(name, n, "bytes", labels),
        None => r.word(name, "unavailable", labels),
    };
}

/// The release builds, made in a copy of the committed tree (`git archive HEAD` extracted under `out`), so that the checkout is
/// never edited and a build that is killed leaves nothing behind in it: the clean build in an empty target directory, the
/// build after a one-line change in a `feature-ui` crate of the copy, and the size of the binary. A run with no git, or with
/// nothing committed, says `unavailable` and prints no number. Without `cold` a compiler cache the environment names stays on
/// and the label says so; with `cold` it is switched off for these builds, which then take minutes. The number of jobs is
/// the `CARGO_BUILD_JOBS` of the environment, or all cores when it is not set, and is printed.
fn builds(r: &mut Report, out: &Path, cold: bool) -> bool {
    let jobs = std::env::var("CARGO_BUILD_JOBS").unwrap_or_else(|_| "default_all_cores".into());
    r.word("c10.build_jobs", &jobs, &[("source", "CARGO_BUILD_JOBS")]);
    let tree = out.join("c10-tree");
    let target = out.join("c10-target");
    let _ = std::fs::remove_dir_all(&tree);
    let _ = std::fs::remove_dir_all(&target);
    let unavailable = |r: &mut Report, why: &str| {
        for name in [
            "c10.build_clean_release",
            "c10.build_after_one_line_change",
            "c10.binary_size_unstripped",
            "c10.binary_size_stripped",
        ] {
            r.word(name, "unavailable", &[("reason", why)]);
        }
        false
    };
    if std::fs::create_dir_all(&tree).is_err() {
        return unavailable(r, "the_copy_directory_cannot_be_made");
    }
    let extract = Command::new("sh")
        .arg("-c")
        .arg("git archive HEAD | tar -x -C \"$0\"")
        .arg(&tree)
        .current_dir(repository())
        .status();
    if !extract.is_ok_and(|s| s.success()) {
        return unavailable(r, "git_archive_of_HEAD_failed_there_is_no_commit_to_build");
    }
    let work = tree.join("frontends/rune");
    let set = [("CARGO_TARGET_DIR", target.as_path())];
    let unset: &[&str] = if cold {
        &["RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER"]
    } else {
        &[]
    };
    let cache = match (cold, compiler_cache()) {
        (true, _) => "none_compiler_cache_switched_off".to_owned(),
        (false, Some(w)) => format!("compiler_cache_{w}_on_so_not_a_cold_time"),
        (false, None) => "no_compiler_cache".to_owned(),
    };
    let args = ["build", "--release", "--locked", "-p", "meldframe-desktop"];
    let (clean, secs) = timed(&work, "cargo", &args, &set, unset);
    r.word(
        "c10.build_clean_release",
        &clean,
        &[
            ("seconds", &format!("{secs:.1}")),
            ("target_dir", "empty"),
            ("tree", "git_archive_of_HEAD"),
            ("cache", &cache),
        ],
    );
    let edited = work.join("crates/features/canvas-ui-rune/src/lib.rs");
    let changed = std::fs::read_to_string(&edited)
        .and_then(|text| std::fs::write(&edited, format!("{text}\n// measured change\n")))
        .is_ok();
    let (again, secs) = if changed {
        timed(&work, "cargo", &args, &set, unset)
    } else {
        ("not_run".to_owned(), 0.0)
    };
    r.word(
        "c10.build_after_one_line_change",
        &again,
        &[
            ("seconds", &format!("{secs:.1}")),
            ("change", "a_comment_line_in_canvas-ui-rune_of_the_copy"),
            ("cache", &cache),
        ],
    );
    let binary = target.join("release/meldframe-desktop");
    size_line(r, "c10.binary_size_unstripped", size_of(&binary), &[]);
    let stripped = out.join("meldframe-desktop.stripped");
    let copied = std::fs::copy(&binary, &stripped).is_ok();
    let (strip, _) = timed(
        out,
        "strip",
        &[stripped.to_string_lossy().as_ref()],
        &[],
        &[],
    );
    let stripped_size = if copied && strip == "ok" {
        size_of(&stripped)
    } else {
        None
    };
    size_line(
        r,
        "c10.binary_size_stripped",
        stripped_size,
        &[("strip", &strip)],
    );
    let _ = std::fs::remove_dir_all(&tree);
    clean == "ok" && again == "ok"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pins_of_the_manifest_are_the_nine_revisions_and_the_eight_patches() {
        let text = std::fs::read_to_string(workspace().join("Cargo.toml")).expect("the manifest");
        let (patches, revs) = pins(&text);
        assert_eq!(patches.len(), 8, "{patches:?}");
        assert_eq!(revs.len(), 9);
        assert!(revs.iter().all(|r| r == RUNE_REVISION));
    }

    #[test]
    fn the_lock_has_one_git_source_and_it_is_rune() {
        let lock = std::fs::read_to_string(workspace().join("Cargo.lock")).expect("the lock");
        assert_eq!(
            git_sources(&lock),
            [format!("https://github.com/anomalyco/rune {RUNE_REVISION}")]
        );
    }

    #[test]
    fn a_comment_is_not_a_pin() {
        let (patches, revs) =
            pins("# rev = \"aaaa\"\n[patch.crates-io]\nfoo = { git = \"u\", rev = \"bbbb\" }\n");
        assert_eq!(
            (patches, revs),
            (vec!["foo".to_owned()], vec!["bbbb".to_owned()])
        );
    }

    #[test]
    fn a_check_prints_the_line_it_ends_with_and_a_failure_is_not_ok() {
        let dir = std::env::temp_dir();
        let (status, says, _) = captured(
            &dir,
            "sh",
            &[
                "-c",
                "echo noise; echo 'deps-check: 130 crates, no violations'",
            ],
        );
        assert_eq!(
            (status.as_str(), says.as_str()),
            ("ok", "deps-check: 130 crates, no violations")
        );
        let (status, _, _) = captured(&dir, "sh", &["-c", "echo 'bad: 1'; exit 3"]);
        assert_eq!(status, "exit_3");
        let (status, _, _) = captured(&dir, "meldframe-no-such-program", &[]);
        assert!(status.starts_with("cannot_run"));
    }

    #[test]
    fn a_size_that_could_not_be_read_prints_unavailable_and_no_number() {
        let mut r = Report::new("C10");
        size_line(&mut r, "c10.binary_size_stripped", None, &[]);
        size_line(&mut r, "c10.binary_size_unstripped", Some(7), &[]);
        let text = r.text();
        assert!(text.contains("c10.binary_size_stripped unavailable -"));
        assert!(!text.contains("stripped -1"));
        assert!(text.contains("c10.binary_size_unstripped 7 bytes"));
    }
}
