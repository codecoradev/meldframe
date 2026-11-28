//! The measurements of the Rune desktop spike (E8.2.3, `docs/spikes/rune-desktop-spike.md` section 3): one subcommand for each
//! criterion that can run here (this slice: C5 and the ones that say what they need, C3, C7, C8 and C9; C1 and C2, then C4, C6 and C10 follow), from a clean checkout, printing `name value unit` lines and one `Cn verdict:` line (the format is in
//! `report.rs`), and writing the same numbers as JSON to `<out>/cN.json`. A criterion that cannot run here says so and prints no
//! number; nothing is estimated.
//!
//! ```text
//! meldframe-measure <meta|c3|c5|c7|c8|c9|all> [options]
//!   --out DIR        where the JSON and the probe tables go (default: <target dir>/measure)
//!   --sizes a,b,c    the node counts of C5 (default 1000,5000,19999)
//!   --calls N        calls for each row of C5 (default 4000, the default of tools/hit-perf.mjs)
//!   --quick          shorter runs (400 calls), labelled `quick=yes`, for a look and never for the record
//! ```
//!
//! Build it in release mode for the record: `cargo run --release --locked -p meldframe-measure -- all`.

mod c5;
mod env;
mod files;
mod report;
mod skipped;
mod stats;

use report::Report;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Default)]
struct Options {
    out: Option<PathBuf>,
    sizes: Option<Vec<usize>>,
    calls: Option<usize>,
    quick: bool,
}

fn number<T: std::str::FromStr>(name: &str, value: Option<&String>) -> Result<T, String> {
    value
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| format!("{name} takes a number"))
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => o.out = Some(PathBuf::from(it.next().ok_or("--out takes a directory")?)),
            "--sizes" => {
                let list = it.next().ok_or("--sizes takes a comma separated list")?;
                let sizes: Result<Vec<usize>, _> = list.split(',').map(str::parse).collect();
                o.sizes = Some(sizes.map_err(|_| "--sizes takes whole numbers".to_owned())?);
            }
            "--calls" => o.calls = Some(number("--calls", it.next())?),
            "--quick" => o.quick = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(o)
}

fn out_dir(o: &Options) -> PathBuf {
    o.out.clone().unwrap_or_else(|| {
        std::env::var_os("CARGO_TARGET_DIR")
            .map_or_else(|| files::workspace().join("target"), PathBuf::from)
            .join("measure")
    })
}

fn run_one(which: &str, o: &Options) -> Option<Report> {
    let calls = o.calls.unwrap_or(if o.quick { 400 } else { 4000 });
    let mut report = match which {
        "meta" => env::run_record(),
        "c3" => skipped::c3(),
        "c5" => c5::run(o.sizes.as_deref().unwrap_or(&[1000, 5000, 19_999]), calls),
        "c7" => skipped::c7(),
        "c8" => skipped::c8(),
        "c9" => skipped::c9(),
        _ => return None,
    };
    if o.quick {
        report.label("quick", "yes");
    }
    Some(report)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((which, rest)) = args.split_first() else {
        eprintln!(
            "usage: meldframe-measure <meta|c3|c5|c7|c8|c9|all> [options] (see the top of src/main.rs)"
        );
        return ExitCode::from(2);
    };
    let options = match parse(rest) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("meldframe-measure: {e}");
            return ExitCode::from(2);
        }
    };
    let list: Vec<&str> = if which == "all" {
        vec!["meta", "c3", "c5", "c7", "c8", "c9"]
    } else {
        vec![which.as_str()]
    };
    let out = out_dir(&options);
    for name in list {
        let Some(report) = run_one(name, &options) else {
            eprintln!("meldframe-measure: unknown criterion {name}");
            return ExitCode::from(2);
        };
        if let Err(e) = report.emit(&out) {
            eprintln!("meldframe-measure: cannot write to {}: {e}", out.display());
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}
