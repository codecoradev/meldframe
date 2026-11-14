//! Main-axis arithmetic shared by sizing and placement: the share of each fill child, and how
//! justify spreads leftover space when no child fills.

use meldframe_protocol::Justify;

/// The size each fill child gets: the space left after fixed children and gaps, never negative,
/// split equally. Zero when no child fills.
pub(super) fn fill_share(inner_main: f64, fixed: f64, gaps: f64, fills: usize) -> f64 {
    if fills == 0 {
        0.0
    } else {
        (inner_main - fixed - gaps).max(0.0) / fills as f64
    }
}

/// Where justify puts children on the main axis: a shift of the first child and an extra gap
/// between children.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Spread {
    pub offset: f64,
    pub extra_gap: f64,
}

/// Distribute `left` (leftover main-axis space, clamped at zero) over `count` children.
pub(super) fn spread(justify: Justify, left: f64, count: usize) -> Spread {
    let left = left.max(0.0);
    match justify {
        Justify::Center => Spread {
            offset: left / 2.0,
            extra_gap: 0.0,
        },
        Justify::End => Spread {
            offset: left,
            extra_gap: 0.0,
        },
        Justify::Between if count > 1 => Spread {
            offset: 0.0,
            extra_gap: left / (count - 1) as f64,
        },
        _ => Spread {
            offset: 0.0,
            extra_gap: 0.0,
        },
    }
}
