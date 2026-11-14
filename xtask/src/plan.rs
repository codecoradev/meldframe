//! A planned `cargo` invocation. Commands build a list of steps with a pure function, then
//! `run` executes them, so the planning is unit-testable without running a process.

use std::process::Command;

/// One `cargo` invocation: a label for the log and the arguments after `cargo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub label: String,
    pub args: Vec<String>,
}

impl Step {
    pub fn new(label: impl Into<String>, args: &[&str]) -> Step {
        Step {
            label: label.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Run every step, keep going after a failure, and return the labels of the steps that failed.
pub fn run(steps: &[Step]) -> Vec<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut failed = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        eprintln!(
            "[{}/{}] {}: cargo {}",
            i + 1,
            steps.len(),
            step.label,
            step.args.join(" ")
        );
        match Command::new(&cargo).args(&step.args).status() {
            Ok(s) if s.success() => {}
            Ok(_) => failed.push(step.label.clone()),
            Err(e) => {
                eprintln!("cannot run cargo: {e}");
                failed.push(step.label.clone());
            }
        }
    }
    failed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failing_step_is_reported_and_later_steps_still_run() {
        let steps = [
            Step::new("bad", &["no-such-subcommand-xyz"]),
            Step::new("good", &["--version"]),
        ];
        assert_eq!(run(&steps), vec!["bad".to_string()]);
    }
}
