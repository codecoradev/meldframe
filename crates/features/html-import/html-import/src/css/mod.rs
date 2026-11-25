//! The CSS side of the importer (ADR-0024, `docs/import-html.md` sections 2.2 and 3.3). This stage holds the
//! selectors ([`selector`]); the stages after it read rules and declarations, cascade and decode Tailwind classes.

pub mod selector;

pub use selector::{Combinator, Compound, Pseudo, Refusal, Selector, Simple};
