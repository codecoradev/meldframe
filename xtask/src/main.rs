//! Repository checks: layering rules and, later, build and size gates.

mod metadata;
mod rules;

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("deps-check") => deps_check(),
        other => {
            eprintln!("usage: cargo xtask deps-check (got {other:?})");
            ExitCode::from(2)
        }
    }
}

/// Read `cargo metadata`, apply the layering rules, print every violation.
fn deps_check() -> ExitCode {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = match Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            eprintln!(
                "cargo metadata failed:\n{}",
                String::from_utf8_lossy(&o.stderr)
            );
            return ExitCode::from(2);
        }
        Err(e) => {
            eprintln!("cannot run cargo metadata: {e}");
            return ExitCode::from(2);
        }
    };
    let packages = match metadata::parse(&String::from_utf8_lossy(&output.stdout)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let violations = rules::check(&packages);
    for v in &violations {
        eprintln!("{v}");
    }
    if violations.is_empty() {
        println!("deps-check: {} crates, no violations", packages.len());
        ExitCode::SUCCESS
    } else {
        eprintln!("deps-check: {} violation(s)", violations.len());
        ExitCode::FAILURE
    }
}
