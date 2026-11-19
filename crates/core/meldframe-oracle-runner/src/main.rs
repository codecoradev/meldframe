//! `oracle-runner`: reads the input JSON on stdin, writes the output JSON on stdout.

use meldframe_oracle_runner::{Input, run};
use std::io::{Read, Write};

fn main() {
    let mut text = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut text) {
        eprintln!("oracle-runner: cannot read stdin: {e}");
        std::process::exit(2);
    }
    let input: Input = match serde_json::from_str(&text) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("oracle-runner: bad input: {e}");
            std::process::exit(2);
        }
    };
    let out = serde_json::to_vec(&run(&input)).expect("the output is plain data");
    if std::io::stdout().write_all(&out).is_err() {
        std::process::exit(2);
    }
}
