//! `ui-literals`, `size-check` and `mm-check` read the Rune workspace as well as the main one
//! (E8.0.4): for each, a violation in a crate under `frontends/rune` fails and the clean case passes.
//! The trees are made on disk, with the packages `cargo metadata` would give for them.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::monolith;
    use crate::rules::Package;
    use crate::{size, ui_literals, units};

    const RUNE_CRATE: &str = "frontends/rune/crates/rune-ui";

    fn tree(name: &str, files: &[(&str, String)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-both-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, text) in files {
            let full = dir.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, text).unwrap();
        }
        dir
    }

    fn package(name: &str, kind: &str, dir: &str) -> Package {
        Package {
            name: name.into(),
            kind: Some(kind.into()),
            dir: dir.into(),
            ..Package::default()
        }
    }

    fn lines(n: usize) -> String {
        "// a line\n".repeat(n)
    }

    #[test]
    fn ui_literals_reads_a_rune_ui_crate() {
        let pkgs = [package("rune-ui", "ui", RUNE_CRATE)];
        let files = |src: &str| [(format!("{RUNE_CRATE}/src/lib.rs"), src.to_string())];
        for (src, count) in [
            ("pub const C: &str = \"#ff00ff\";\n", 1),
            ("pub const C: &str = tokens::TEXT;\n", 0),
        ] {
            let list = files(src);
            let refs: Vec<(&str, String)> =
                list.iter().map(|(p, t)| (p.as_str(), t.clone())).collect();
            let root = tree("lit", &refs);
            let found = ui_literals::collect(&root, &pkgs);
            std::fs::remove_dir_all(&root).unwrap();
            assert_eq!(found.len(), count, "{src}: {found:?}");
        }
    }

    fn judged(name: &str, rune_lines: usize, tool_lines: usize) -> Vec<size::Violation> {
        let pkgs = [package("rune-ui", "ui", RUNE_CRATE)];
        let root = tree(
            name,
            &[
                (&format!("{RUNE_CRATE}/src/lib.rs"), lines(rune_lines)),
                ("frontends/rune/tools/measure.mjs", lines(tool_lines)),
            ],
        );
        let mut sizes = size::measure(&root, &pkgs);
        sizes.extend(units::unowned(&root, &pkgs));
        std::fs::remove_dir_all(&root).unwrap();
        size::check(&sizes, &size::Allow::default())
    }

    #[test]
    fn size_check_counts_the_crates_and_the_other_source_of_the_rune_workspace() {
        assert_eq!(judged("size-ok", 500, 500), vec![]);
        let over_crate_file = judged("size-crate", 501, 10);
        assert_eq!(
            over_crate_file,
            vec![size::Violation::OverLimit(
                format!("{RUNE_CRATE}/src/lib.rs"),
                501,
                size::FILE_LIMIT
            )]
        );
        let over_script = judged("size-tool", 10, 501);
        assert_eq!(
            over_script,
            vec![size::Violation::OverLimit(
                "frontends/rune/tools/measure.mjs".into(),
                501,
                size::FILE_LIMIT
            )]
        );
    }

    /// A function body of more than 25 tokens.
    fn body(name: &str) -> String {
        format!(
            "pub fn {name}(x: u32) -> u32 {{ let a = x + 1; let b = a * 2; let c = b - 1; \
             if c > 10 {{ c + a }} else {{ c * b }} }}\n"
        )
    }

    fn mm(name: &str, rune_src: &str) -> Vec<monolith::Violation> {
        let pkgs = [
            package("core-a", "core", "crates/core/core-a"),
            package("rune-ui", "ui", RUNE_CRATE),
        ];
        let root = tree(
            name,
            &[
                ("crates/core/core-a/src/lib.rs", body("main_side")),
                (&format!("{RUNE_CRATE}/src/lib.rs"), rune_src.to_string()),
            ],
        );
        let crates = monolith::load(&root, &pkgs);
        let _ = std::fs::remove_dir_all(&root);
        let cfg = monolith::Config {
            threshold: 4000,
            ..monolith::Config::default()
        };
        monolith::check(&crates, &cfg)
    }

    #[test]
    fn mm_check_finds_a_function_copied_between_the_two_workspaces() {
        let copy = mm("mm-copy", &body("rune_side"));
        assert_eq!(copy.len(), 1, "{copy:?}");
        assert_eq!(copy[0].rule, "MM3");
        assert!(copy[0].message.contains(RUNE_CRATE), "{}", copy[0]);
        assert_eq!(mm("mm-clean", "pub fn other() -> u32 { 7 }\n"), vec![]);
    }

    // The wiring itself: the packages come from `cargo metadata` of two real (tiny) workspaces, the
    // way `cargo xtask` reads them, and each of the four commands judges what `packages::read` gave.

    fn manifest(name: &str, kind: Option<&str>) -> String {
        let meta = kind.map_or(String::new(), |k| {
            format!("\n[package.metadata.meldframe]\nkind = \"{k}\"\n")
        });
        format!("[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n{meta}")
    }

    /// A main workspace with one core crate and, under `frontends/rune`, a workspace with a `ui`
    /// crate and a crate with no kind. With `dirty`, the Rune half has a colour literal, a file of
    /// 501 lines and a copy of the main crate's function; the main half is clean either way.
    fn two_workspaces(name: &str, dirty: bool) -> PathBuf {
        let rune_src = if dirty {
            format!(
                "{}pub const C: &str = \"#ff00ff\";\n{}",
                body("rune_side"),
                lines(499)
            )
        } else {
            "pub fn other() -> u32 {\n    7\n}\n".to_string()
        };
        let mut files = vec![
            (
                "Cargo.toml",
                "[workspace]\nresolver = \"3\"\nmembers = [\"crates/*\"]\nexclude = [\"frontends/rune\"]\n".to_string(),
            ),
            ("crates/core-a/Cargo.toml", manifest("core-a", Some("core"))),
            ("crates/core-a/src/lib.rs", body("main_side")),
            (
                "frontends/rune/Cargo.toml",
                "[workspace]\nresolver = \"3\"\nmembers = [\"crates/*\"]\n".to_string(),
            ),
            ("frontends/rune/crates/rune-ui/Cargo.toml", manifest("rune-ui", Some("ui"))),
            ("frontends/rune/crates/rune-ui/src/lib.rs", rune_src),
        ];
        if dirty {
            files.push((
                "frontends/rune/crates/rune-bare/Cargo.toml",
                manifest("rune-bare", None),
            ));
            files.push(("frontends/rune/crates/rune-bare/src/lib.rs", String::new()));
        }
        tree(name, &files)
    }

    /// What deps-check, ui-literals, size-check and mm-check find, in that order.
    fn findings(root: &std::path::Path, scope: crate::packages::Scope) -> [usize; 4] {
        let pkgs = crate::packages::read(root, scope).expect("cargo metadata");
        let cfg = monolith::Config {
            threshold: 4000,
            ..monolith::Config::default()
        };
        [
            crate::deps_check(root, &pkgs).len(),
            ui_literals::collect(root, &pkgs).len(),
            size::check(&crate::measured(root, &pkgs), &size::Allow::default()).len(),
            monolith::check(&monolith::load(root, &pkgs), &cfg).len(),
        ]
    }

    #[test]
    fn each_command_fails_on_a_violation_that_is_only_in_the_rune_workspace() {
        use crate::packages::Scope;
        let dirty = two_workspaces("wired-dirty", true);
        let both = findings(&dirty, Scope::Both);
        let main_only = findings(&dirty, Scope::Main);
        let names: Vec<String> = crate::packages::read(&dirty, Scope::Both)
            .unwrap()
            .into_iter()
            .map(|p| format!("{}@{}", p.name, p.dir))
            .collect();
        let _ = std::fs::remove_dir_all(&dirty);
        assert_eq!(
            names,
            [
                "core-a@crates/core-a",
                "rune-bare@frontends/rune/crates/rune-bare",
                "rune-ui@frontends/rune/crates/rune-ui"
            ]
        );
        // One finding each, and none when the Rune half is left out. The size count is left out of
        // the second comparison: with no Rune package known, `size-check` would count the files of
        // its crates as source that no package owns, which it never does in a real run.
        assert_eq!(both, [1, 1, 1, 1]);
        assert_eq!([main_only[0], main_only[1], main_only[3]], [0, 0, 0]);
    }

    #[test]
    fn each_command_passes_two_clean_workspaces() {
        let clean = two_workspaces("wired-clean", false);
        let found = findings(&clean, crate::packages::Scope::Both);
        let _ = std::fs::remove_dir_all(&clean);
        assert_eq!(found, [0, 0, 0, 0]);
    }
}
