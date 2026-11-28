//! The few statistics the criteria print: the mean, a percentile and the slope of a series.

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

/// The slope of the least squares line through `(x, y)` points, in y units per x unit (0 for fewer than two points).
pub fn slope(points: &[(f64, f64)]) -> f64 {
    let n = points.len() as f64;
    if points.len() < 2 {
        return 0.0;
    }
    let (sx, sy) = points
        .iter()
        .fold((0.0, 0.0), |(a, b), (x, y)| (a + x, b + y));
    let (mx, my) = (sx / n, sy / n);
    let num: f64 = points.iter().map(|(x, y)| (x - mx) * (y - my)).sum();
    let den: f64 = points.iter().map(|(x, _)| (x - mx) * (x - mx)).sum();
    if den == 0.0 { 0.0 } else { num / den }
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

    #[test]
    fn the_slope_of_a_line_is_its_gradient_and_a_flat_series_has_none() {
        let line: Vec<(f64, f64)> = (0..10)
            .map(|x| (f64::from(x), 3.0 * f64::from(x) + 5.0))
            .collect();
        assert!((slope(&line) - 3.0).abs() < 1e-9);
        let flat: Vec<(f64, f64)> = (0..10).map(|x| (f64::from(x), 7.0)).collect();
        assert_eq!(slope(&flat), 0.0);
        assert_eq!(slope(&[(1.0, 1.0)]), 0.0);
    }
}
