//! Repository checks: layering rules, build matrix, portability and other gates.

mod licences;
mod matrix;
mod metadata;
mod plan;
mod portable;
mod rules;
mod scan;
mod size;
mod ui_literals;

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("deps-check") => with_packages(|p| {
            report(
                "deps-check",
                &format!("{} crates", p.len()),
                &rules::check(p),
            )
        }),
        Some("build-matrix") => {
            with_packages(|p| run_steps("build-matrix", &matrix::plan(p), None))
        }
        Some("portable-check") => with_packages(|p| {
            let hint = "if a target is missing: rustup target add wasm32-unknown-unknown \
                        (rust-toolchain.toml installs it on a fresh checkout)";
            run_steps("portable-check", &portable::plan(p), Some(hint))
        }),
        Some("size-check") => with_packages(|p| finish(size_check(p))),
        Some("licences") => finish(licences_check()),
        Some("ui-literals") => with_packages(|p| {
            let found = ui_literals::collect(&scan::workspace_root(), p);
            report("ui-literals", "UI code", &found)
        }),
        other => {
            eprintln!(
                "usage: cargo xtask <deps-check|build-matrix|portable-check|size-check|ui-literals|licences> (got {other:?})"
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

/// Run `cargo metadata` with extra arguments and return its JSON, or an exit code on failure.
fn metadata_json(extra: &[&str]) -> Result<String, ExitCode> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    match Command::new(cargo)
        .args(["metadata", "--format-version", "1"])
        .args(extra)
        .output()
    {
        Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).into_owned()),
        Ok(o) => {
            eprintln!(
                "cargo metadata failed:\n{}",
                String::from_utf8_lossy(&o.stderr)
            );
            Err(ExitCode::from(2))
        }
        Err(e) => {
            eprintln!("cannot run cargo metadata: {e}");
            Err(ExitCode::from(2))
        }
    }
}

/// Read `cargo metadata` and hand the workspace packages to `f`.
fn with_packages(f: impl FnOnce(&[rules::Package]) -> ExitCode) -> ExitCode {
    let json = match metadata_json(&["--no-deps"]) {
        Ok(j) => j,
        Err(code) => return code,
    };
    match metadata::parse(&json) {
        Ok(p) => f(&p),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

/// `licences [--check]`: third-party licences against the allow-list in `deny.toml`.
fn licences_check() -> Result<ExitCode, String> {
    let root = scan::workspace_root();
    let json = metadata_json(&[]).map_err(|_| "cargo metadata failed".to_string())?;
    let allowed = licences::parse_allow(&scan::read(&root, "deny.toml"))?;
    let deps = licences::parse_deps(&json)?;
    let scope = format!("{} third-party crates", deps.len());
    Ok(report(
        "licences",
        &scope,
        &licences::check(&deps, &allowed),
    ))
}

/// `size-check [--base <git-ref>]`: ceilings, and with `--base` that the allow-list did not grow.
fn size_check(packages: &[rules::Package]) -> Result<ExitCode, String> {
    let root = scan::workspace_root();
    let allow = size::parse_allow(&scan::read(&root, size::ALLOW_FILE))?;
    let mut violations = size::check(&size::measure(&root, packages), &allow);
    let args: Vec<String> = std::env::args().collect();
    if let Some(base) = args
        .iter()
        .position(|a| a == "--base")
        .and_then(|i| args.get(i + 1))
    {
        // A missing file on the base means the list is being introduced: start from empty.
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
    Ok(report(
        "size-check",
        &format!("{} crates", packages.len()),
        &violations,
    ))
}
