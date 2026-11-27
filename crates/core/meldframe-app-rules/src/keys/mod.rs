//! The keys of the editor as data and as a rule (audit finding T7a and T7b, issue 324, decision 43 clause Q1): `table` is the one
//! table of what a key or a gesture does and what the shortcut sheet says about it, `decide` is the rule that turns a key press
//! into what it makes, and `sheet` writes the table as the groups of the sheet, for a platform. None of it holds state, reads
//! a clock or touches a front end.

pub mod decide;
pub mod sheet;
pub mod table;

pub use decide::decide;
pub use table::{Binding, Combo, Group, Held, Makes, Row, TABLE};
