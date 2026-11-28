//! CSS value readers shared by the importers (core, portable): the colour forms the model can hold, read as hex
//! ([`color`]), a length as pixels ([`length`]) and the number both read ([`number`]). Text in, a description out. Nothing is evaluated, fetched or
//! opened, and nothing here reads a clock or a file.

pub mod color;
pub mod length;
pub mod number;
