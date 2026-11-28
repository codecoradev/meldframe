//! The run record of section 2 of the spike note, read from the machine: what can be read is printed, what cannot is
//! `unknown`, never a guess. The display, the power state and the operator are not readable here and stay in the record by hand.

use crate::report::Report;
use std::process::Command;

/// The first line of what `program args` prints, or `unknown` when it cannot run.
pub fn first_line(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".into())
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// The value after `key` of a `key: value` line of a /proc file.
fn proc_value(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_owned())
}

/// The adapter path as far as the environment says it: a software Vulkan driver is named, anything else is not claimed to be
/// a real GPU. The adapter wgpu picks is reported by Rune on a real window; this reads only what the process was told.
pub fn gpu_path() -> (String, bool) {
    match std::env::var("VK_ICD_FILENAMES") {
        Ok(v) if v.to_lowercase().contains("swiftshader") => {
            (format!("software: SwiftShader Vulkan ICD ({v})"), true)
        }
        Ok(v) => (format!("Vulkan ICD from VK_ICD_FILENAMES ({v})"), false),
        Err(_) => (
            "not set: the adapter wgpu picks on this machine, not read here".into(),
            false,
        ),
    }
}

/// The record, as lines.
pub fn run_record() -> Report {
    let mut r = Report::new("RUN");
    r.status("ran", "");
    let linux = cfg!(target_os = "linux");
    let cpuinfo = read("/proc/cpuinfo");
    let meminfo = read("/proc/meminfo");
    let os = if linux {
        read("/etc/os-release")
            .lines()
            .find_map(|l| l.strip_prefix("PRETTY_NAME="))
            .map(|v| v.trim_matches('"').to_owned())
            .unwrap_or_else(|| "unknown".into())
    } else {
        format!(
            "{} {}",
            first_line("sw_vers", &["-productName"]),
            first_line("sw_vers", &["-productVersion"])
        )
    };
    let cpu = if linux {
        proc_value(&cpuinfo, "model name").unwrap_or_else(|| "unknown".into())
    } else {
        first_line("sysctl", &["-n", "machdep.cpu.brand_string"])
    };
    let memory = if linux {
        proc_value(&meminfo, "MemTotal").unwrap_or_else(|| "unknown".into())
    } else {
        first_line("sysctl", &["-n", "hw.memsize"])
    };
    let cpus = std::thread::available_parallelism().map_or(0, |n| n.get() as i64);
    let (gpu, software) = gpu_path();
    r.word("run.os", &os, &[])
        .word("run.kernel", &first_line("uname", &["-sr"]), &[])
        .word("run.arch", std::env::consts::ARCH, &[])
        .word("run.cpu_model", &cpu, &[])
        .int("run.cpu_count", cpus, "count", &[])
        .word("run.memory_total", &memory, &[])
        .word("run.gpu_path", &gpu, &[])
        .word(
            "run.real_gpu",
            if software { "no" } else { "not_established" },
            &[],
        )
        .word("run.rune_revision", meldframe_rune_pin::RUNE_REVISION, &[])
        .word("run.rustc", &first_line("rustc", &["--version"]), &[])
        .word(
            "run.meldframe_commit",
            &first_line("git", &["rev-parse", "HEAD"]),
            &[],
        )
        .word(
            "run.date_utc",
            &first_line("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"]),
            &[],
        )
        .word(
            "run.profile",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            &[],
        );
    if !linux {
        r.word(
            "run.gpu_chipset",
            &first_line(
                "sh",
                &[
                    "-c",
                    "system_profiler SPDisplaysDataType | grep -m1 'Chipset Model'",
                ],
            ),
            &[],
        );
    }
    r.verdict("open");
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_names_the_revision_and_never_claims_a_real_gpu_it_did_not_read() {
        let text = run_record().text();
        assert!(text.contains(&format!(
            "run.rune_revision {}",
            meldframe_rune_pin::RUNE_REVISION
        )));
        assert!(!text.contains("run.real_gpu yes"));
    }

    #[test]
    fn a_program_that_does_not_exist_reads_as_unknown() {
        assert_eq!(first_line("meldframe-no-such-program", &[]), "unknown");
    }
}
