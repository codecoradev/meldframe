//! What a criterion prints and writes: `name value unit` lines in one stable format, a status and a verdict, and the same
//! numbers as JSON. A number is only ever added by code that measured it; a criterion that cannot run says so in its status.
//!
//! The format, one record to a line, fields separated by one space:
//!
//! ```text
//! C2 status: ran|partial|skipped reason=<words joined by _>
//! <name> <value> <unit> [key=value ...]        the name is lower case, `c2a.width_diff_max`; labels hold no space
//! C2 verdict: green|gap|tripwire|open
//! ```
//!
//! `open` is the word of the note for "not measured, or not all of it" (`docs/spikes/rune-desktop-spike.md` section 3).

use serde_json::{Value, json};
use std::fmt::Write as _;
use std::path::Path;

/// One criterion's output: lines for stdout and the JSON to write.
#[derive(Debug)]
pub struct Report {
    id: &'static str,
    labels: Vec<(String, String)>,
    lines: Vec<Line>,
    status: Option<String>,
    verdict: Option<String>,
}

#[derive(Debug)]
struct Line {
    name: String,
    value: String,
    unit: String,
    labels: Vec<(String, String)>,
}

/// A text with its spaces made `_`, so that it stays one field of a line.
pub fn field(text: &str) -> String {
    let s: String = text
        .trim()
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect();
    if s.is_empty() { "-".into() } else { s }
}

impl Report {
    /// A report of criterion `id`, such as `"C2"`.
    pub fn new(id: &'static str) -> Report {
        Report {
            id,
            labels: Vec::new(),
            lines: Vec::new(),
            status: None,
            verdict: None,
        }
    }

    /// A label every following number carries, such as `adapter=software`.
    pub fn label(&mut self, key: &str, value: &str) -> &mut Report {
        self.labels.push((key.into(), field(value)));
        self
    }

    /// A measured number with its unit and the labels of this one line.
    pub fn num(
        &mut self,
        name: &str,
        value: f64,
        unit: &str,
        labels: &[(&str, &str)],
    ) -> &mut Report {
        let shown = if value.is_finite() {
            format!("{value:.4}")
        } else {
            "nan".into()
        };
        self.push(name, shown, unit, labels)
    }

    /// A whole number (a count, a size in bytes).
    pub fn int(
        &mut self,
        name: &str,
        value: i64,
        unit: &str,
        labels: &[(&str, &str)],
    ) -> &mut Report {
        self.push(name, value.to_string(), unit, labels)
    }

    /// A word as a value (a revision, a name), unit `-`.
    pub fn word(&mut self, name: &str, value: &str, labels: &[(&str, &str)]) -> &mut Report {
        self.push(name, field(value), "-", labels)
    }

    fn push(
        &mut self,
        name: &str,
        value: String,
        unit: &str,
        labels: &[(&str, &str)],
    ) -> &mut Report {
        self.lines.push(Line {
            name: name.into(),
            value,
            unit: field(unit),
            labels: labels
                .iter()
                .map(|(k, v)| ((*k).into(), field(v)))
                .collect(),
        });
        self
    }

    /// The criterion ran (all of it), ran in part, or did not run, and why.
    pub fn status(&mut self, status: &str, reason: &str) -> &mut Report {
        let reason = if reason.is_empty() {
            String::new()
        } else {
            format!(" reason={}", field(reason))
        };
        self.status = Some(format!("{status}{reason}"));
        self
    }

    /// The verdict word: `green`, `gap`, `tripwire` or `open`.
    pub fn verdict(&mut self, verdict: &str) -> &mut Report {
        self.verdict = Some(verdict.into());
        self
    }

    /// The text to print.
    pub fn text(&self) -> String {
        let mut out = String::new();
        let status = self
            .status
            .as_deref()
            .unwrap_or("skipped reason=nothing_ran");
        let _ = writeln!(out, "{} status: {status}", self.id);
        for l in &self.lines {
            let _ = write!(out, "{} {} {}", l.name, l.value, l.unit);
            for (k, v) in self.labels.iter().chain(&l.labels) {
                let _ = write!(out, " {k}={v}");
            }
            out.push('\n');
        }
        let _ = writeln!(
            out,
            "{} verdict: {}",
            self.id,
            self.verdict.as_deref().unwrap_or("open")
        );
        out
    }

    /// The same numbers as JSON.
    pub fn json(&self) -> Value {
        let labels = |l: &[(String, String)]| -> Value {
            l.iter()
                .map(|(k, v)| (k.clone(), json!(v)))
                .collect::<serde_json::Map<_, _>>()
                .into()
        };
        json!({
            "criterion": self.id,
            "status": self.status,
            "verdict": self.verdict.as_deref().unwrap_or("open"),
            "labels": labels(&self.labels),
            "numbers": self.lines.iter().map(|l| json!({
                "name": l.name, "value": l.value, "unit": l.unit, "labels": labels(&l.labels),
            })).collect::<Vec<_>>(),
        })
    }

    /// Prints the lines and writes `<dir>/<id>.json` (the id in lower case).
    pub fn emit(&self, dir: &Path) -> std::io::Result<()> {
        print!("{}", self.text());
        std::fs::create_dir_all(dir)?;
        let file = dir.join(format!("{}.json", self.id.to_lowercase()));
        std::fs::write(file, serde_json::to_string_pretty(&self.json())? + "\n")
    }
}

/// A criterion that cannot run here, said plainly: its status and the verdict `open`, with no number.
pub fn skipped(id: &'static str, reason: &str, needs: &[&str]) -> Report {
    let mut r = Report::new(id);
    r.status("skipped", reason);
    for (i, need) in needs.iter().enumerate() {
        r.word(&format!("{}.needs.{}", id.to_lowercase(), i + 1), need, &[]);
    }
    r.verdict("open");
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_prints_the_status_the_numbers_and_the_verdict_in_the_stable_format() {
        let mut r = Report::new("C9");
        r.label("adapter", "software adapter")
            .status("ran", "")
            .num("c9.first_wheel_ms", 12.5, "ms", &[("rest", "5s")])
            .int("c9.cycles", 10, "count", &[])
            .verdict("gap");
        assert_eq!(
            r.text(),
            "C9 status: ran\n\
             c9.first_wheel_ms 12.5000 ms adapter=software_adapter rest=5s\n\
             c9.cycles 10 count adapter=software_adapter\n\
             C9 verdict: gap\n"
        );
    }

    #[test]
    fn a_skipped_criterion_has_a_reason_no_number_and_the_verdict_open() {
        let text = skipped("C7", "needs a person and macOS input methods", &[]).text();
        assert_eq!(
            text,
            "C7 status: skipped reason=needs_a_person_and_macOS_input_methods\nC7 verdict: open\n"
        );
    }

    #[test]
    fn the_json_holds_the_same_numbers() {
        let mut r = Report::new("C5");
        r.status("partial", "click latency needs a window").num(
            "c5a.hit_p99_ms",
            0.25,
            "ms",
            &[("nodes", "1000")],
        );
        let json = r.json();
        assert_eq!(json["criterion"], "C5");
        assert_eq!(json["verdict"], "open");
        assert_eq!(json["numbers"][0]["name"], "c5a.hit_p99_ms");
        assert_eq!(json["numbers"][0]["labels"]["nodes"], "1000");
    }

    #[test]
    fn a_value_that_is_not_a_number_prints_as_nan_and_never_as_zero() {
        let mut r = Report::new("C1");
        r.num("c1.max_diff", f64::NAN, "px", &[]);
        assert!(r.text().contains("c1.max_diff nan px"));
    }
}
