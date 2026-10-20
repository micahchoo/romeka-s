//! CSV: the RFC 4180 reader the CLI shares, and the import's spreadsheet.

mod reader;
pub mod sheet;
pub mod windows1252;

pub use reader::{CsvError, Record, parse, parse_with, render};
