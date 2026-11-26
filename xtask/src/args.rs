//! The arguments after a subcommand: a flag that a command does not know is an error, so a typo
//! such as `--bsae` does not silently skip the check the flag stands for.

/// Check the arguments of `command` (`args` is the whole command line: the program, the
/// subcommand, then these). `known` lists the flags the command takes; `--base` also takes a value,
/// as `--base <ref>` or `--base=<ref>`, and no other argument may be a bare word.
pub fn check_flags(command: &str, args: &[String], known: &[&str]) -> Result<(), String> {
    let mut rest = args.iter().skip(2);
    while let Some(arg) = rest.next() {
        let name = arg.split('=').next().unwrap_or(arg);
        if !known.contains(&name) {
            return Err(format!(
                "{command}: unknown argument `{arg}` (it takes: {})",
                known.join(", ")
            ));
        }
        if arg == "--base" {
            rest.next();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(args: &[&str]) -> Vec<String> {
        ["xtask", "cmd"]
            .iter()
            .chain(args)
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn known_flags_and_the_value_of_base_pass() {
        let known = ["--base", "--list"];
        for args in [
            &[][..],
            &["--list"],
            &["--base", "origin/develop"],
            &["--base=origin/develop"],
            &["--list", "--base", "origin/develop"],
        ] {
            assert_eq!(check_flags("cmd", &line(args), &known), Ok(()), "{args:?}");
        }
    }

    #[test]
    fn a_misspelt_flag_a_bare_word_and_a_flag_of_another_command_fail() {
        let known = ["--base"];
        for args in [
            &["--bsae", "x"][..],
            &["--bsae=x"],
            &["x"],
            &["--base", "origin/develop", "extra"],
            &["--list"],
        ] {
            let err = check_flags("cmd", &line(args), &known).unwrap_err();
            assert!(err.starts_with("cmd: unknown argument"), "{args:?}: {err}");
        }
    }
}
