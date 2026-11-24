//! CSS value readers shared by the importers (core, portable): the colour forms the model can hold, read as hex
//! ([`color`]), and a length as pixels ([`length`]). Text in, a description out. Nothing is evaluated, fetched or
//! opened, and nothing here reads a clock or a file.

pub mod color;
pub mod length;
