//! The names a node gets, and the source it is traced back to.

use html_import_tree::{Element, clean};

/// The most characters of a node name (`meldframe_check::Limits::max_name`).
const MAX_NAME: usize = 256;
/// The most characters of a source label or of a listed `src`.
pub(crate) const MAX_SOURCE: usize = 200;

/// The name of the node an element makes: `data-name` (what the export writes), else `id`, else the first class,
/// else the tag. Hidden characters are removed and the name cut to 256 characters, so it passes the check of a name.
pub(crate) fn name_of(el: &Element) -> String {
    let first_class = el
        .attr("class")
        .and_then(|c| c.split_ascii_whitespace().next());
    [el.attr("data-name"), el.attr("id"), first_class]
        .into_iter()
        .flatten()
        .map(|s| clean(s, MAX_NAME))
        .find(|s| !s.trim().is_empty())
        .unwrap_or_else(|| clean(&el.tag, MAX_NAME))
}

/// `tag.first-class#id`, for the agent to match what it sent to what it got.
pub(crate) fn source_of(el: &Element) -> String {
    let mut s = el.tag.clone();
    if let Some(c) = el
        .attr("class")
        .and_then(|c| c.split_ascii_whitespace().next())
    {
        s.push('.');
        s.push_str(c);
    }
    if let Some(i) = el.attr("id").filter(|i| !i.is_empty()) {
        s.push('#');
        s.push_str(i);
    }
    clean(&s, MAX_SOURCE)
}
