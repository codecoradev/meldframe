//! The few statistics the criteria print: the mean, a percentile and the largest sample.

/// The milliseconds `f` takes.
pub fn ms(f: impl FnOnce()) -> f64 {
    let t = std::time::Instant::now();
    f();
    t.elapsed().as_secs_f64() * 1e3
}

/// The mean of `a` (0 for no sample).
pub fn mean(a: &[f64]) -> f64 {
    if a.is_empty() {
        0.0
    } else {
        a.iter().sum::<f64>() / a.len() as f64
    }
}

/// The percentile `p` (0 to 1) of `a` the way `tools/hit-perf.mjs` reads it: the sample at `floor(len * p)` of the sorted list,
/// the last one at most. 0 for no sample.
pub fn percentile(a: &[f64], p: f64) -> f64 {
    if a.is_empty() {
        return 0.0;
    }
    let mut s = a.to_vec();
    s.sort_by(f64::total_cmp);
    s[((s.len() as f64 * p).floor() as usize).min(s.len() - 1)]
}

/// The largest sample (0 for none).
pub fn max(a: &[f64]) -> f64 {
    a.iter().copied().fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_percentile_reads_the_sorted_list_at_floor_len_times_p() {
        let a: Vec<f64> = (1..=100).map(f64::from).rev().collect();
        assert_eq!(percentile(&a, 0.99), 100.0);
        assert_eq!(percentile(&a, 0.95), 96.0);
        assert_eq!(percentile(&a, 0.5), 51.0);
        assert_eq!(percentile(&[], 0.99), 0.0);
        assert_eq!(max(&a), 100.0);
    }
}
