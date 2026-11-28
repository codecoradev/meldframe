//! The computed style of an element: what the cascade, the browser's defaults and inheritance leave it, as the
//! mapping stages read it. Properties are longhands (shorthands are expanded) as the text of their value after
//! `var()` was replaced by the literal; the three the layout and text need in numbers are also resolved: `color`
//! (hex), `font-size` (pixels) and `font-weight`. What is refused is in [`Style::notes`], on the element it belongs
//! to; nothing refused is guessed, and the value of the parent (or the initial one) stays.
//!
//! - **Declared**: a property some rule, the `style` attribute or the defaults table gave this element
//!   ([`Source::Author`], [`Source::Inline`], [`Source::UserAgent`]). The mapping counts its refusals from these.
//! - **Inherited**: the properties of `defaults::INHERITED` the element got from its parent ([`Source::Inherited`]).
//!   They are in [`Style::props`] too, so the mapping reads one place, but they are not counted again.
//! - **Absent**: no value anywhere. The mapping then applies section 3.1 (CSS initial values, and the model fields
//!   written explicitly).

use std::collections::BTreeMap;
use std::sync::Arc;

use super::value::{Len, Unit, length};

/// Where the value of a property came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    UserAgent,
    Author,
    Inline,
    Inherited,
}

/// A property with its value as text, and where it came from. The text is shared with the rule it came from
/// unless it was made (a `var()` replaced, a shorthand cut into its longhands).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prop {
    pub value: Arc<str>,
    pub source: Source,
}

pub type Props = BTreeMap<Arc<str>, Prop>;

/// The custom properties an element sees, as the literal each has (no `var()` is left in it). `None` is the
/// guaranteed-invalid value: a declaration that could not be resolved, or `initial`. An element that sets none
/// shares its parent's map.
pub type Custom = Arc<BTreeMap<Arc<str>, Option<Arc<str>>>>;

/// Something the element could not take: a property left out, with its value cut to 80 characters and the reason
/// (`var_undefined`, `var_too_deep`, `var_too_long`, `var_invalid`, a shorthand's reason such as
/// `border_shorthand`, or `color_unreadable`, `font_size_unreadable`, `font_weight_unreadable`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub property: String,
    pub value: String,
    pub code: &'static str,
}

/// The most notes one element keeps; the rest are counted in the report.
pub const MAX_NOTES: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// Every longhand the element has, declared or inherited, but not the custom properties.
    pub props: Props,
    /// The custom properties (`--x`) in effect, which inherit.
    pub custom: Custom,
    /// The text colour as hex (eight digits when the alpha is below 1): the inherited `color`, black at the top.
    pub color: String,
    /// The font size in pixels: 16 at the top, and `em` is read against the parent's.
    pub font_size: f64,
    /// The font weight, 1 to 1000: 400 at the top, `bold` is 700. The mapping rounds it to a step of 100.
    pub font_weight: f64,
    /// The pixels of one `rem`: the computed font size of the root (`html`, or the one a fragment does not have),
    /// 16 for the root itself, and 16 where the root's size is not readable.
    pub rem: f64,
    /// What the rules that name `::before` and `::after` give the generated boxes (declared only: they take
    /// nothing from the element and the defaults, and the `style` attribute does not reach them).
    pub before: Option<Props>,
    pub after: Option<Props>,
    pub notes: Vec<Note>,
    /// The classes that no rule names and that are not Tailwind utilities of the export's vocabulary
    /// (`Matched::unresolved`), for the mapping to list on the node.
    pub unresolved: Vec<Arc<str>>,
}

impl Style {
    /// The style above everything: nothing declared, black, 16 px, weight 400.
    pub fn root() -> Style {
        Style {
            props: Props::new(),
            custom: Custom::default(),
            color: "#000000".into(),
            font_size: 16.0,
            font_weight: 400.0,
            rem: 16.0,
            before: None,
            after: None,
            notes: Vec::new(),
            unresolved: Vec::new(),
        }
    }

    /// The text of `prop`, if the element has it.
    pub fn get(&self, prop: &str) -> Option<&str> {
        self.props.get(prop).map(|p| &*p.value)
    }

    /// `prop` as a length in pixels, with `em` against this element's font size and `rem` against the root's. `None`:
    /// absent, or not a length this reads (a keyword, a percentage, another unit), or inherited and written in
    /// `em`: that is the size of the element that wrote it, which is not this one's (the computed length is not
    /// inherited, only the text).
    pub fn len(&self, prop: &str) -> Option<Len> {
        let p = self.props.get(prop)?;
        let l = length(&p.value, self.font_size, self.rem)?;
        (p.source != Source::Inherited || l.unit != Unit::Em).then_some(l)
    }

    /// The properties this element declared itself (not inherited ones), for the mapping to count its refusals.
    pub fn declared(&self) -> impl Iterator<Item = (&str, &Prop)> {
        self.props
            .iter()
            .filter(|(_, p)| p.source != Source::Inherited)
            .map(|(k, p)| (&**k, p))
    }
}
