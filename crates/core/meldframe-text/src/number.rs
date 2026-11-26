/// A whole number from outside (a path segment, a key suffix, a prefix of a stored key) in its one spelling: ASCII
/// digits only, no sign, no space, and no leading zero except the single digit `0`, within the range of a `u32`.
/// `None` for anything else, so `01`, `+1` and `1` never name the same number.
///
/// This is a rule of spelling, not a bound: a caller that wants a lower or an upper limit (a checkpoint number starts at
/// 1) applies it to the result, and maps `None` to its own error.
pub fn canonical_u32(s: &str) -> Option<u32> {
    let digits = !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (digits && (s == "0" || !s.starts_with('0')))
        .then(|| s.parse().ok())
        .flatten()
}
