//! The measurements of the Rune desktop spike (E8.2.3, `docs/spikes/rune-desktop-spike.md` section 3): one subcommand for each
//! criterion C1 to C10, from a clean checkout, printing `name value unit` lines and one `Cn verdict:` line (the format is in
//! `report.rs`), and writing the same numbers as JSON to `<out>/cN.json`. A criterion that cannot run here says so and prints no
//! number; nothing is estimated.
//!
//! ```text
//! meldframe-measure <meta|c1|c2|c3|c4|c5|c6|c7|c8|c9|c10|all> [options]
//!   --out DIR        where the JSON and the probe tables go (default: <target dir>/measure)
//!   --table FILE     the Chromium text table of C2 (default: tools/measure/data/chromium_text_table.json)
//!   --sizes a,b,c    the node counts of C4 and C5 (defaults: C4 5000,19999; C5 1000,5000,19999)
//!   --calls N        calls for each row of C5 (default 4000, the default of tools/hit-perf.mjs)
//!   --ticks N        ticks of C4 (default 300)
//!   --seconds N      length of C6 (default 180)
//!   --checks         C10: run `cargo xtask deps-check`, `cargo xtask licences --check` and `cargo deny check`
//!   --build          C10: a clean release build, one after a one-line change and the size of the binary (minutes)
//!   --cold           C10: switch the compiler cache (RUSTC_WRAPPER) off for those builds, so that the time is a cold one
//!   --system-fonts   C2: also use the machine's fonts (an experiment that depends on the machine, never the record)
//!   --quick          shorter runs (400 calls, 60 ticks, 20 s), labelled `quick=yes`, for a look and never for the record
//! ```
//!
//! Build it in release mode for the record: `cargo run --release --locked -p meldframe-measure -- all`.

mod c1;
mod c10;
mod c2;
mod c2pop;
mod c4;
mod c5;
mod c6;
mod env;
mod files;
mod headless;
mod report;
mod skipped;
mod stats;

use report::Report;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Default)]
struct Options {
    out: Option<PathBuf>,
    table: Option<PathBuf>,
    sizes: Option<Vec<usize>>,
    calls: Option<usize>,
    ticks: Option<usize>,
    seconds: Option<u64>,
    checks: bool,
    build: bool,
    cold: bool,
    system_fonts: bool,
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
            "--table" => o.table = Some(PathBuf::from(it.next().ok_or("--table takes a file")?)),
            "--sizes" => {
                let list = it.next().ok_or("--sizes takes a comma separated list")?;
                let sizes: Result<Vec<usize>, _> = list.split(',').map(str::parse).collect();
                o.sizes = Some(sizes.map_err(|_| "--sizes takes whole numbers".to_owned())?);
            }
            "--calls" => o.calls = Some(number("--calls", it.next())?),
            "--ticks" => o.ticks = Some(number("--ticks", it.next())?),
            "--seconds" => o.seconds = Some(number("--seconds", it.next())?),
            "--checks" => o.checks = true,
            "--build" => o.build = true,
            "--cold" => o.cold = true,
            "--system-fonts" => o.system_fonts = true,
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
    let out = out_dir(o);
    let table = files::read_table(o.table.as_deref().unwrap_or(&files::default_table_path())).ok();
    let calls = o.calls.unwrap_or(if o.quick { 400 } else { 4000 });
    let ticks = o.ticks.unwrap_or(if o.quick { 60 } else { 300 });
    let seconds = o.seconds.unwrap_or(if o.quick { 20 } else { 180 });
    let mut report = match which {
        "meta" => env::run_record(),
        "c1" => c1::run(table.as_ref()),
        "c2" if o.system_fonts => c2::run_with(
            o.table.as_deref(),
            &out,
            &meldframe_text_rune::RuneMeasurer::with_system_fonts(),
            "rune_with_system_fonts_experiment",
        ),
        "c2" => c2::run(o.table.as_deref(), &out),
        "c3" => skipped::c3(),
        "c4" => c4::run(o.sizes.as_deref().unwrap_or(&[5000, 19_999]), ticks, &out),
        "c5" => c5::run(o.sizes.as_deref().unwrap_or(&[1000, 5000, 19_999]), calls),
        "c6" => c6::run(5000, seconds),
        "c7" => skipped::c7(),
        "c8" => skipped::c8(),
        "c9" => skipped::c9(),
        "c10" => c10::run(o.checks, o.build, o.cold, &out),
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
            "usage: meldframe-measure <meta|c1|c2|c3|c4|c5|c6|c7|c8|c9|c10|all> [options] (see the top of src/main.rs)"
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
        vec![
            "meta", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "c9", "c10",
        ]
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
