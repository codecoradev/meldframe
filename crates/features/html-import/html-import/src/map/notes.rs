//! What one node could not take: the properties that were refused ([`Lost`]) and the values that were changed to the
//! nearest the model holds ([`Change`]). Everything here comes from the document, so it is made safe to show where it
//! enters (rule U9): the display profile for a value, the strict one for a property name, each cut.

use meldframe_text::{Profile, sanitize_clip};

/// The most entries of each list a node keeps.
const MAX_PER_NODE: usize = 64;
const MAX_VALUE: usize = 80;

/// A value changed to the nearest the model holds: `property` was `from` in the source and is `to` in the node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub property: String,
    pub from: String,
    pub to: String,
}

/// A property that was not carried. `value` is cut to 80 characters; `code` is a stable reason
/// (`refused_not_modelled`, `var_undefined`, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lost {
    pub property: String,
    pub value: String,
    pub code: String,
}

/// The two lists of a node, filled while it is read. A property is listed once.
#[derive(Debug, Default, Clone)]
pub(crate) struct Notes {
    pub approx: Vec<Change>,
    pub lost: Vec<Lost>,
}

fn show(s: &str) -> String {
    sanitize_clip(s, Profile::Display, MAX_VALUE, usize::MAX)
}

impl Notes {
    /// `property` was read as `to` where the source said `from`.
    pub fn change(&mut self, property: &str, from: &str, to: &str) {
        let property = sanitize_clip(property, Profile::Strict, MAX_VALUE, usize::MAX);
        if self.approx.len() < MAX_PER_NODE && !self.approx.iter().any(|c| c.property == property) {
            self.approx.push(Change {
                property,
                from: show(from),
                to: show(to),
            });
        }
    }

    /// `property` was not carried.
    pub fn lose(&mut self, property: &str, value: &str, code: &str) {
        let property = sanitize_clip(property, Profile::Strict, MAX_VALUE, usize::MAX);
        if self.lost.len() < MAX_PER_NODE && !self.lost.iter().any(|l| l.property == property) {
            self.lost.push(Lost {
                property,
                value: show(value),
                code: sanitize_clip(code, Profile::Strict, MAX_VALUE, usize::MAX),
            });
        }
    }
}
