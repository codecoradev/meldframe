//! Fractional index: base-62 keys for sibling order (`Node.pos`). A small dependency-free port of the
//! algorithm by David Greenspan that `rocicorp/fractional-indexing` uses.
//!
//! - A key is a variable-length integer part followed by a fractional part. The head letter gives the
//!   integer length: `a`..`z` mean 2..27 digits, `A`..`Z` mean 27..2 digits (for "negative" keys). The
//!   fractional part uses base-62 digits `0-9A-Za-z`, so byte order is ASCII order. Appending or
//!   prepending repeatedly grows the length only logarithmically.
//! - A key is valid when every character is in the alphabet, the integer part matches its head letter,
//!   the fractional part does NOT end in `0` (so a fraction has one representation) and it is not the
//!   smallest key (`A` followed by 26 zeros).
//! - Inserting between two keys returns a key STRICTLY between them. The length grows by about one
//!   character per six consecutive insertions into the same gap (each insertion halves the gap and a
//!   base-62 digit holds about six bits).

use std::fmt;

const DIGITS: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const SMALLEST_INT: &str = "A00000000000000000000000000";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FracError {
    /// The key is invalid (character, head length or a trailing `0`).
    Invalid(String),
    /// `a >= b`: there is no key between them.
    Order(String, String),
    /// The key space is exhausted (in practice never: it needs more than 26 integer digits).
    Exhausted,
}

impl fmt::Display for FracError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FracError::Invalid(k) => write!(f, "invalid pos: {k:?}"),
            FracError::Order(a, b) => write!(f, "pos {a:?} is not smaller than {b:?}"),
            FracError::Exhausted => write!(f, "pos space exhausted"),
        }
    }
}
impl std::error::Error for FracError {}

fn idx(c: u8) -> usize {
    DIGITS.iter().position(|d| *d == c).expect("valid digit")
}

fn int_len(head: u8) -> Option<usize> {
    match head {
        b'a'..=b'z' => Some((head - b'a') as usize + 2),
        b'A'..=b'Z' => Some((b'Z' - head) as usize + 2),
        _ => None,
    }
}

/// Whether `k` is a valid key under the rules above.
pub fn is_valid(k: &str) -> bool {
    let b = k.as_bytes();
    if b.is_empty() || !b.iter().all(|c| DIGITS.contains(c)) {
        return false;
    }
    let Some(n) = int_len(b[0]) else { return false };
    if b.len() < n || k == SMALLEST_INT || (b.len() > n && b[b.len() - 1] == b'0') {
        return false;
    }
    true
}

fn split(k: &str) -> (&str, &str) {
    let n = int_len(k.as_bytes()[0]).expect("valid key");
    k.split_at(n)
}

/// Midpoint of two fractions `a < b` (`a` may be empty = 0, `b = None` = 1). The result never ends in `0`.
fn midpoint(a: &str, b: Option<&str>) -> String {
    let (a, b) = (a.as_bytes(), b.map(str::as_bytes));
    if let Some(b) = b {
        // drop the common prefix (`a` is padded with '0')
        let mut n = 0;
        while n < b.len() && a.get(n).copied().unwrap_or(b'0') == b[n] {
            n += 1;
        }
        if n > 0 {
            let rest_a = if n < a.len() { &a[n..] } else { &[][..] };
            let mut out = String::from_utf8(b[..n].to_vec()).unwrap();
            out.push_str(&midpoint(
                std::str::from_utf8(rest_a).unwrap(),
                Some(std::str::from_utf8(&b[n..]).unwrap()),
            ));
            return out;
        }
    }
    let da = a.first().map_or(0, |c| idx(*c));
    let db = b.map_or(62, |b| idx(b[0]));
    if db - da > 1 {
        let mid = ((da + db) as f64 * 0.5).round() as usize;
        return (DIGITS[mid] as char).to_string();
    }
    if let Some(b) = b
        && b.len() > 1
    {
        return (b[0] as char).to_string();
    }
    // The first digits are adjacent (or `b` is used up): keep the digit of `a` and continue with the rest of `a`.
    let rest_a = if a.len() > 1 {
        std::str::from_utf8(&a[1..]).unwrap()
    } else {
        ""
    };
    let mut out = (DIGITS[da] as char).to_string();
    out.push_str(&midpoint(rest_a, None));
    out
}

fn increment_int(x: &str) -> Option<String> {
    let mut b: Vec<u8> = x.bytes().collect();
    let head = b[0];
    let mut carry = true;
    for i in (1..b.len()).rev() {
        let d = idx(b[i]) + 1;
        if d == 62 {
            b[i] = b'0';
        } else {
            b[i] = DIGITS[d];
            carry = false;
            break;
        }
    }
    if carry {
        if head == b'Z' {
            return Some("a0".into());
        }
        if head == b'z' {
            return None;
        }
        let h = head + 1;
        if h > b'a' {
            b.push(b'0');
        } else {
            b.pop();
        }
        b[0] = h;
    }
    Some(String::from_utf8(b).unwrap())
}

fn decrement_int(x: &str) -> Option<String> {
    let mut b: Vec<u8> = x.bytes().collect();
    let head = b[0];
    let mut borrow = true;
    for i in (1..b.len()).rev() {
        let d = idx(b[i]);
        if d == 0 {
            b[i] = b'z';
        } else {
            b[i] = DIGITS[d - 1];
            borrow = false;
            break;
        }
    }
    if borrow {
        if head == b'a' {
            return Some("Zz".into());
        }
        if head == b'A' {
            return None;
        }
        let h = head - 1;
        if h < b'Z' {
            b.push(b'z');
        } else {
            b.pop();
        }
        b[0] = h;
    }
    Some(String::from_utf8(b).unwrap())
}

/// A key strictly between `a` and `b` (`None` = unbounded on that side). Both keys must be valid and `a < b`.
pub fn between(a: Option<&str>, b: Option<&str>) -> Result<String, FracError> {
    for k in [a, b].into_iter().flatten() {
        if !is_valid(k) {
            return Err(FracError::Invalid(k.to_string()));
        }
    }
    if let (Some(a), Some(b)) = (a, b)
        && a >= b
    {
        return Err(FracError::Order(a.to_string(), b.to_string()));
    }
    match (a, b) {
        (None, None) => Ok("a0".into()),
        (None, Some(b)) => {
            let (ib, fb) = split(b);
            if ib == SMALLEST_INT {
                return Ok(format!("{ib}{}", midpoint("", Some(fb))));
            }
            if ib < b {
                return Ok(ib.to_string());
            }
            decrement_int(ib).ok_or(FracError::Exhausted)
        }
        (Some(a), None) => {
            let (ia, fa) = split(a);
            Ok(increment_int(ia).unwrap_or_else(|| format!("{ia}{}", midpoint(fa, None))))
        }
        (Some(a), Some(b)) => {
            let (ia, fa) = split(a);
            let (ib, fb) = split(b);
            if ia == ib {
                return Ok(format!("{ia}{}", midpoint(fa, Some(fb))));
            }
            if let Some(i) = increment_int(ia)
                && i.as_str() < b
            {
                return Ok(i);
            }
            Ok(format!("{ia}{}", midpoint(fa, None)))
        }
    }
}

/// `n` ascending keys between `a` and `b` (for migration and rebalancing: key length grows logarithmically).
pub fn n_between(a: Option<&str>, b: Option<&str>, n: usize) -> Result<Vec<String>, FracError> {
    if n == 0 {
        return Ok(vec![]);
    }
    if n == 1 {
        return Ok(vec![between(a, b)?]);
    }
    match b {
        None => {
            let mut c = between(a, b)?;
            let mut out = vec![c.clone()];
            for _ in 0..n - 1 {
                c = between(Some(&c), b)?;
                out.push(c.clone());
            }
            Ok(out)
        }
        Some(_) if a.is_none() => {
            let mut c = between(a, b)?;
            let mut out = vec![c.clone()];
            for _ in 0..n - 1 {
                c = between(a, Some(&c))?;
                out.push(c.clone());
            }
            out.reverse();
            Ok(out)
        }
        Some(_) => {
            let mid = n / 2;
            let c = between(a, b)?;
            let mut out = n_between(a, Some(&c), mid)?;
            out.push(c.clone());
            out.extend(n_between(Some(&c), b, n - mid - 1)?);
            Ok(out)
        }
    }
}
