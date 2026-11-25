//! The small readers every paint value shares: the colour word. More come with the readers that need them.

/// A colour word as the model holds it: `#rrggbb[aa]` (or the short hex as written), and whether a colour outside
/// sRGB was clamped. `currentcolor` is the colour of the element. `None` for anything else (a system colour, `lab()`,
/// a word that is no colour).
pub(super) fn colour_word(w: &str, style: &crate::css::Style) -> Option<(String, bool)> {
    let w = w.trim();
    if w.eq_ignore_ascii_case("currentcolor") {
        return Some((style.color.clone(), false));
    }
    crate::css::value::colour(w).map(|h| (h.hex, h.clipped))
}
