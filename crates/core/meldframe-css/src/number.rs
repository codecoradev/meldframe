//! The one reader of a CSS number: digits, a point and a sign, and an exponent only where the caller asks for it.
//! Finite. A word that Rust would parse (`NaN`, `inf`) is no number here, because every byte is checked first.

/// `s` trimmed, when it is not empty and every byte passes `allowed`, read as a finite number.
fn read(s: &str, allowed: impl Fn(u8) -> bool) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() || !s.bytes().all(allowed) {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Does the word start like a number or a length (`10px`, `-3`, `.5`, `+1%`)? Only the first byte is read.
pub fn starts_like_number(w: &str) -> bool {
    w.bytes()
        .next()
        .is_some_and(|b| b.is_ascii_digit() || matches!(b, b'.' | b'+' | b'-'))
}

/// A number as CSS writes it without an exponent: digits, a point and a sign. Finite.
pub fn number(s: &str) -> Option<f64> {
    read(s, |b| b.is_ascii_digit() || matches!(b, b'.' | b'+' | b'-'))
}

/// A number as CSS writes it with an exponent (`1e-2`): digits, a point, a sign and `e` or `E`. Finite.
pub fn number_exp(s: &str) -> Option<f64> {
    read(s, |b| {
        b.is_ascii_digit() || matches!(b, b'.' | b'+' | b'-' | b'e' | b'E')
    })
}
