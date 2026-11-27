//! Search and ranking: which entries a query leaves and in what order, and what the arrow keys and Enter do to the highlight.

use super::entries::Entry;

/// How well the words typed match an entry, or `None` when one of them matches nothing: a label that starts with the word beats a
/// word of it that does, then any part of the label, then the group, a keyword or the detail.
fn score(e: &Entry, words: &[String]) -> Option<u32> {
    let label = e.label.to_lowercase();
    let label_words: Vec<&str> = label.split(|c: char| !c.is_ascii_alphanumeric()).collect();
    let rest: Vec<String> = std::iter::once(e.group)
        .chain(e.detail.as_deref())
        .chain(e.keywords.iter().copied())
        .map(str::to_lowercase)
        .collect();
    let mut total = 0;
    for w in words {
        let s = if label.starts_with(w.as_str()) {
            4
        } else if label_words.iter().any(|x| x.starts_with(w.as_str())) {
            3
        } else if label.contains(w.as_str()) {
            2
        } else if rest.iter().any(|x| x.contains(w.as_str())) {
            1
        } else {
            return None;
        };
        total += s;
    }
    Some(total)
}

/// The entries a query leaves: every word of it must match (case does not matter, and it is text, not a pattern). The best match
/// is first and an entry that cannot run comes after those that can; equals keep the order of the list. A blank query keeps the
/// list as it is.
pub fn filter(entries: &[Entry], query: &str) -> Vec<Entry> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return entries.to_vec();
    }
    let mut hit: Vec<(usize, u32, &Entry)> = entries
        .iter()
        .enumerate()
        .filter_map(|(at, e)| score(e, &words).map(|s| (at, s, e)))
        .collect();
    hit.sort_by(|a, b| {
        b.2.enabled
            .cmp(&a.2.enabled)
            .then(b.1.cmp(&a.1))
            .then(a.0.cmp(&b.0))
    });
    hit.into_iter().map(|(_, _, e)| e.clone()).collect()
}

/// The first entry that can run, where the highlight starts.
pub fn first_enabled(entries: &[Entry]) -> Option<usize> {
    entries.iter().position(|e| e.enabled)
}

/// What a key does to the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResult {
    /// The highlight moves to this entry.
    Move(usize),
    /// The highlighted entry runs.
    Run(usize),
}

/// The up and down arrows move the highlight over the entries that can run, round the ends; Enter runs the highlighted one; every
/// other key is the field's.
pub fn on_key(key: &str, active: Option<usize>, entries: &[Entry]) -> Option<KeyResult> {
    if key == "Enter" {
        return active
            .filter(|&i| entries.get(i).is_some_and(|e| e.enabled))
            .map(KeyResult::Run);
    }
    let step: i64 = match key {
        "ArrowDown" => 1,
        "ArrowUp" => -1,
        _ => return None,
    };
    let count = i64::try_from(entries.len()).ok()?;
    let from = active.and_then(|a| i64::try_from(a).ok()).unwrap_or(-1);
    (1..=count)
        .map(|n| (from + step * n).rem_euclid(count))
        .find_map(|at| {
            let at = usize::try_from(at).ok()?;
            entries[at].enabled.then_some(KeyResult::Move(at))
        })
}

/// The row that is highlighted: the one the pointer chose while it is listed and can run, else the first that can run.
pub fn highlight_at(shown: &[Entry], picked: Option<&str>) -> Option<usize> {
    picked
        .and_then(|id| shown.iter().position(|e| e.id == id && e.enabled))
        .or_else(|| first_enabled(shown))
}

/// What the status line of the palette says about the rows listed.
pub fn results_text(n: usize) -> String {
    match n {
        0 => "No results".into(),
        1 => "1 result".into(),
        n => format!("{n} results"),
    }
}
