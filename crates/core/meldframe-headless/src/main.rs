//! `meldframe-headless [--strict] <script>`: run a script file (see the crate doc) and print what it saw. Exits 1
//! when a step fails (a check, or with `--strict` a refusal), 2 when the script cannot be read. A file named by
//! `open` is read relative to the directory of the script.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let strict = args.iter().any(|a| a == "--strict");
    let paths: Vec<&String> = args.iter().filter(|a| *a != "--strict").collect();
    let [path] = paths.as_slice() else {
        eprintln!("usage: meldframe-headless [--strict] <script>");
        return ExitCode::from(2);
    };
    let script = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let dir = Path::new(path.as_str()).parent().unwrap_or(Path::new("."));
    let read =
        |name: &str| std::fs::read_to_string(dir.join(name)).map_err(|e| format!("{name}: {e}"));
    let report = meldframe_headless::run(&script, &read, strict);
    print!("{}", report.out);
    match report.failure {
        Some(why) => {
            eprintln!("meldframe-headless: {why}");
            ExitCode::FAILURE
        }
        None => ExitCode::SUCCESS,
    }
}
