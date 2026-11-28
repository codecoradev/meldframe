//! A CSS length as the model holds it: a number of pixels. `px` and a bare number are read as they are and `rem` is
//! sixteen pixels. Nothing here evaluates anything: it reads digits.

use crate::number::number;

/// The pixels in one `rem`: the browser's default root size.
pub const REM_PX: f64 = 16.0;

/// A length, in pixels, and whether the text said `rem` (so a report can say a conversion was made).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Length {
    pub value: f64,
    pub from_rem: bool,
}

/// `12px`, `1.5rem` or a bare number, as pixels. `None` for anything else: another unit, a percentage, a function or
/// a number that is not finite.
pub fn length(s: &str) -> Option<Length> {
    let l = s.to_ascii_lowercase();
    let (n, rem) = if let Some(n) = l.strip_suffix("rem") {
        (n, true)
    } else if let Some(n) = l.strip_suffix("px") {
        (n, false)
    } else {
        (l.as_str(), false)
    };
    let v = number(n)?;
    Some(if rem {
        Length {
            value: v * REM_PX,
            from_rem: true,
        }
    } else {
        Length {
            value: v,
            from_rem: false,
        }
    })
}
