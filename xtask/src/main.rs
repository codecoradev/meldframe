//! Repository checks: layering rules, build matrix, portability and other gates.

mod args;
mod bin_homes;
mod both_workspaces;
mod gitsrc;
mod groups;
mod licence_scopes;
mod matrix;
mod metadata;
mod monolith;
mod packages;
mod plan;
mod portable;
mod rules;
mod scan;
mod size;
mod toolkit_swap;
mod ui_literals;
mod units;
mod workspaces;

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("deps-check") => with_packages("deps-check", |p| {
            let found = deps_check(&scan::workspace_root(), p);
            report("deps-check", &format!("{} crates", p.len()), &found)
        }),
        Some("build-matrix") => with_packages("build-matrix", |p| {
            run_steps("build-matrix", &matrix::plan(p), None)
        }),
        Some("portable-check") => with_packages("portable-check", |p| {
            let hint = "if a target is missing: rustup target add wasm32-unknown-unknown \
                        (rust-toolchain.toml installs it on a fresh checkout)";
            run_steps("portable-check", &portable::plan(p), Some(hint))
        }),
        Some("size-check") => with_packages("size-check", |p| finish(size_check(p))),
        Some("mm-check") => with_packages("mm-check", |p| {
            finish(monolith::run(p, &std::env::args().collect::<Vec<_>>()))
        }),
        Some("toolkit-swap") => with_packages("toolkit-swap", |p| {
            finish(toolkit_swap::run(p, &std::env::args().collect::<Vec<_>>()))
        }),
        Some("licences") => finish(licences_check()),
        Some("ui-literals") => with_packages("ui-literals", |p| {
            let found = ui_literals::collect(&scan::workspace_root(), p);
            report("ui-literals", "UI code", &found)
        }),
        other => {
            eprintln!(
                "usage: cargo xtask <deps-check|build-matrix|portable-check|size-check|mm-check|toolkit-swap|ui-literals|licences> (got {other:?})"
            );
            ExitCode::from(2)
        }
    }
}

/// Print every violation; success only when there are none.
fn report(name: &str, scope: &str, violations: &[impl std::fmt::Display]) -> ExitCode {
    violations.iter().for_each(|v| eprintln!("{v}"));
    if violations.is_empty() {
        println!("{name}: {scope}, no violations");
        ExitCode::SUCCESS
    } else {
        eprintln!("{name}: {} violation(s)", violations.len());
        ExitCode::FAILURE
    }
}

/// A command that could not even run (bad input) exits 2; a finished check keeps its code.
fn finish(result: Result<ExitCode, String>) -> ExitCode {
    result.unwrap_or_else(|e| {
        eprintln!("{e}");
        ExitCode::from(2)
    })
}

/// Run planned steps; fail when any step fails.
fn run_steps(name: &str, steps: &[plan::Step], hint: Option<&str>) -> ExitCode {
    let failed = plan::run(steps);
    if failed.is_empty() {
        println!("{name}: {} step(s) passed", steps.len());
        return ExitCode::SUCCESS;
    }
    for f in &failed {
        eprintln!("{name}: FAILED {f}");
    }
    if let Some(h) = hint {
        eprintln!("{name}: {h}");
    }
    ExitCode::FAILURE
}

/// Read the packages of the workspaces `command` reads (`packages::scope`) and hand them to `f`.
fn with_packages(command: &str, f: impl FnOnce(&[rules::Package]) -> ExitCode) -> ExitCode {
    match packages::read(&scan::workspace_root(), packages::scope(command)) {
        Ok(p) => f(&p),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

/// The rules of `deps-check`: kinds and the matrix, where a `bin` crate may live, the line between
/// the two workspaces, and the manifests that name a git source.
fn deps_check(root: &std::path::Path, packages: &[rules::Package]) -> Vec<String> {
    let manifests = gitsrc::collect(root);
    fn shown(v: &[impl std::fmt::Display]) -> Vec<String> {
        v.iter().map(ToString::to_string).collect()
    }
    [
        shown(&rules::check(packages)),
        shown(&bin_homes::check(packages)),
        shown(&workspaces::check(packages)),
        shown(&gitsrc::check(&manifests)),
    ]
    .concat()
}

/// `licences [--check] [--list] [--main-only]`: third-party licences of each workspace against the
/// allow list of its own `deny.toml` (D2; the Rune workspace has its own, E8.0.4). The Rune graph
/// is read with `--locked` and needs the registry and the Rune repository, so `--main-only` leaves
/// it out for a job that has no reason to fetch them. `--list` prints every crate with its licence.
fn licences_check() -> Result<ExitCode, String> {
    let args: Vec<String> = std::env::args().collect();
    args::check_flags("licences", &args, &["--check", "--list", "--main-only"])?;
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let root = scan::workspace_root();
    let main = packages::cargo_metadata(&root, &root.join("Cargo.toml"), &[])?;
    let main_deny = scan::read(&root, "deny.toml");
    let rune_deny = scan::read(&root, &format!("{}/deny.toml", workspaces::RUNE_DIR));
    let rune = match packages::rune_manifest(&root) {
        Some(manifest) if !has("--main-only") => {
            Some(packages::cargo_metadata(&root, &manifest, &["--locked"])?)
        }
        _ => None,
    };
    let mut scopes = vec![licence_scopes::Scope {
        name: "main",
        metadata: &main,
        deny_toml: &main_deny,
    }];
    if let Some(rune) = &rune {
        scopes.push(licence_scopes::Scope {
            name: workspaces::RUNE_DIR,
            metadata: rune,
            deny_toml: &rune_deny,
        });
    }
    let outcome = licence_scopes::check(&scopes)?;
    if has("--list") {
        outcome.listing.iter().for_each(|l| println!("{l}"));
    }
    let counts: Vec<String> = scopes
        .iter()
        .zip(&outcome.counts)
        .map(|(s, n)| format!("{n} ({})", s.name))
        .collect();
    let scope = format!("{} third-party crates", counts.join(" + "));
    Ok(report("licences", &scope, &outcome.violations))
}

/// What `size-check` measures: every package and the source that no package owns (`tools/*`, a
/// `packages/*` without a manifest, the non-crate source of `frontends/rune`).
fn measured(root: &std::path::Path, packages: &[rules::Package]) -> Vec<size::Sized> {
    let mut sizes = size::measure(root, packages);
    sizes.extend(units::unowned(root, packages));
    sizes
}

/// `size-check [--base <git-ref>]`: ceilings, and with `--base` that the allow-list did not grow.
/// After the result line it notes the crates above the soft value (2,500); that never fails.
fn size_check(packages: &[rules::Package]) -> Result<ExitCode, String> {
    let root = scan::workspace_root();
    let allow = size::parse_allow(&scan::read(&root, size::ALLOW_FILE))?;
    let sizes = measured(&root, packages);
    let mut violations = size::check(&sizes, &allow);
    let args: Vec<String> = std::env::args().collect();
    args::check_flags("size-check", &args, &["--base"])?;
    if let Some(base) = size::base_arg(&args)? {
        // The ref must resolve: a bad ref is an error, never an empty base.
        let resolves = |spec: &str| {
            Command::new("git")
                .args(["rev-parse", "--verify", "--quiet", spec])
                .current_dir(&root)
                .output()
                .is_ok_and(|o| o.status.success())
        };
        if !resolves(&format!("{base}^{{commit}}")) {
            return Err(format!(
                "size-check: --base `{base}` is not a git ref that resolves"
            ));
        }
        // A file missing on a resolved base means the list is being introduced: start from empty.
        let out = Command::new("git")
            .args(["show", &format!("{base}:{}", size::ALLOW_FILE)])
            .current_dir(&root)
            .output();
        let text = out
            .ok()
            .filter(|o| o.status.success())
            .map(|o| o.stdout)
            .unwrap_or_default();
        let base_allow = size::parse_allow(&String::from_utf8_lossy(&text))?;
        violations.extend(size::check_not_raised(&base_allow, &allow));
    }
    let code = report(
        "size-check",
        &format!("{} crates and units", sizes.len()),
        &violations,
    );
    if let Some(note) = size::split_note(&size::split_candidates(&sizes)) {
        println!("{note}");
    }
    Ok(code)
}
