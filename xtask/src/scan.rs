//! File-system side of the source checks: walk crate directories and read source files.

use std::fs;
use std::path::{Path, PathBuf};

/// The workspace root (the parent of this crate's directory). Cargo sets `CARGO_MANIFEST_DIR` when it
/// runs `xtask`, so the tree under test is the one the command is run from. The value compiled in is
/// only a fallback: with several worktrees sharing one target directory a cached binary may have been
/// built in another tree.
pub fn workspace_root() -> PathBuf {
    let run = std::env::var("CARGO_MANIFEST_DIR").ok();
    root_of(&pick_manifest_dir(run, env!("CARGO_MANIFEST_DIR")))
}

/// The run-time value counts only if it names an existing directory called `xtask`: a stale value left
/// by another package (a `cargo test -p other` that starts xtask) must not move the root.
fn pick_manifest_dir(run_time: Option<String>, compiled_in: &str) -> String {
    run_time
        .filter(|d| Path::new(d).file_name().is_some_and(|n| n == "xtask") && Path::new(d).is_dir())
        .unwrap_or_else(|| compiled_in.to_string())
}

fn root_of(manifest_dir: &str) -> PathBuf {
    Path::new(manifest_dir)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

/// Build outputs that exist at a known path, relative to the workspace root. Only these exact
/// directories are skipped (a `dist` or `target` anywhere else is source and is scanned).
const SKIPPED_DIRS: &[&str] = &[
    "target",
    "frontends/rune/target",
    "apps/web/dist",
    "tools/fidelity-oracle/out",
];

/// Every file under `dir` whose extension is in `exts`, as `/`-separated paths relative to
/// `root`, sorted. Skips the directories in `SKIPPED_DIRS`, `node_modules` at any depth and hidden
/// directories, and does not follow symbolic links.
pub fn files(root: &Path, dir: &str, exts: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    walk(root, &root.join(dir), exts, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, exts: &[&str], out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        // `DirEntry::file_type` does not follow links: a symlink is neither a dir nor a file here.
        let Ok(kind) = e.file_type() else {
            continue;
        };
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if kind.is_dir() {
            if name != "node_modules"
                && !name.starts_with('.')
                && !SKIPPED_DIRS.contains(&rel.as_str())
            {
                walk(root, &path, exts, out);
            }
        } else if kind.is_file()
            && path
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| exts.contains(&x))
        {
            out.push(rel);
        }
    }
}

/// The text of `rel` on the git ref `base`: an error when the ref does not resolve, `None` when
/// the file is not on it (a list that is being introduced).
pub fn base_file(root: &Path, base: &str, rel: &str) -> Result<Option<String>, String> {
    let git = |spec: &str, show: bool| {
        let mut cmd = std::process::Command::new("git");
        if show {
            cmd.args(["show", spec]);
        } else {
            cmd.args(["rev-parse", "--verify", "--quiet", spec]);
        }
        cmd.current_dir(root)
            .output()
            .ok()
            .filter(|o| o.status.success())
    };
    if git(&format!("{base}^{{commit}}"), false).is_none() {
        return Err(format!("--base `{base}` is not a git ref that resolves"));
    }
    Ok(
        git(&format!("{base}:{rel}"), true)
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned()),
    )
}

/// Read a file relative to `root`; unreadable files read as empty (a skipped file is not a
/// finding for the checks that call this).
pub fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(name: &str, files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for f in files {
            let p = dir.join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, "x\n").unwrap();
        }
        dir
    }

    #[test]
    fn only_the_top_level_target_and_the_known_build_outputs_are_skipped() {
        let root = tree(
            "skip",
            &[
                "target/a.rs",
                "frontends/rune/target/debug/r.rs",
                "frontends/rune/crates/p/target/q.rs",
                "crates/x/target/b.rs",
                "crates/x/src/dist/c.rs",
                "apps/web/dist/d.js",
                "apps/web/src/e.js",
                "apps/web/node_modules/m/f.js",
                "crates/x/node_modules/g.js",
                ".hidden/h.rs",
            ],
        );
        let got = files(&root, "", &["rs", "js"]);
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            got,
            [
                "apps/web/src/e.js",
                "crates/x/src/dist/c.rs",
                "crates/x/target/b.rs",
                "frontends/rune/crates/p/target/q.rs"
            ]
        );
    }

    #[test]
    fn what_the_fidelity_oracle_writes_is_skipped_and_nothing_else_called_out() {
        // The oracle writes pages and a stylesheet for each fixture to `out/` (git-ignored).
        let root = tree(
            "oracle-out",
            &[
                "tools/fidelity-oracle/out/f1/page.html",
                "tools/fidelity-oracle/out/f1/tw.css",
                "tools/fidelity-oracle/oracle.mjs",
                "tools/fidelity-oracle/test/out/keep.mjs",
                "tools/other/out/keep.mjs",
            ],
        );
        let got = files(&root, "tools", &["html", "css", "mjs"]);
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            got,
            [
                "tools/fidelity-oracle/oracle.mjs",
                "tools/fidelity-oracle/test/out/keep.mjs",
                "tools/other/out/keep.mjs"
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        let root = tree("link", &["a/real.rs", "b/other.rs"]);
        std::os::unix::fs::symlink(root.join("a"), root.join("a/loop")).unwrap();
        std::os::unix::fs::symlink(root.join("b/other.rs"), root.join("a/file.rs")).unwrap();
        let got = files(&root, "", &["rs"]);
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(got, ["a/real.rs", "b/other.rs"]);
    }

    #[test]
    fn the_root_is_the_parent_of_the_xtask_directory() {
        assert_eq!(root_of("/a/b/xtask"), PathBuf::from("/a/b"));
    }

    #[test]
    fn the_directory_cargo_gives_at_run_time_wins_over_the_one_compiled_in() {
        // A cached binary built in one worktree must measure the worktree it is run from.
        let dir = std::env::temp_dir().join(format!("xtask-scan-{}", std::process::id()));
        let other = dir.join("xtask");
        fs::create_dir_all(&other).unwrap();
        let run = other.to_string_lossy().into_owned();
        assert_eq!(
            pick_manifest_dir(Some(run.clone()), "/built/here/xtask"),
            run
        );
        assert_eq!(
            pick_manifest_dir(None, "/built/here/xtask"),
            "/built/here/xtask"
        );
        assert_eq!(
            pick_manifest_dir(Some(String::new()), "/built/here/xtask"),
            "/built/here/xtask"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_directory_of_another_package_is_not_taken_for_the_xtask_directory() {
        // `cargo test -p other` sets CARGO_MANIFEST_DIR to that package, and may start xtask.
        let foreign = Some("/x/crates/core/meldframe-check".to_string());
        assert_eq!(
            pick_manifest_dir(foreign, "/built/here/xtask"),
            "/built/here/xtask"
        );
        let nowhere = Some("/does/not/exist".to_string());
        assert_eq!(
            pick_manifest_dir(nowhere, "/built/here/xtask"),
            "/built/here/xtask"
        );
    }
}
