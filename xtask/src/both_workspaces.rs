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
}
