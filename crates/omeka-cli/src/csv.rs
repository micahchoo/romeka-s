//! The CSV reader and writer for `romeka import csv` and `romeka export csv`.
//!
//! The reader lives in `omeka_api::import::csv`, shared with the archive
//! import. This module maps its error to [`CliError::Parse`], which names the
//! record the input broke on.

use crate::error::CliError;

pub use omeka_api::import::csv::{Record, render};

/// Parse comma-separated input. Blank records are dropped.
///
/// # Errors
/// A [`CliError::Parse`] for an unterminated quoted field.
pub fn parse(input: &str) -> Result<Vec<Record>, CliError> {
    omeka_api::import::csv::parse(input).map_err(|error| CliError::Parse {
        line: error.line(),
        message: "unterminated quoted field".to_string(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn rejects_an_unterminated_quote() {
        let error = parse("a,b\n1,\"oops\n").expect_err("unterminated");
        assert!(
            matches!(error, CliError::Parse { line: 2, .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn renders_what_it_parses_back() {
        let text = render(&[vec!["x,y".to_string(), "z".to_string()]]);
        let back = parse(&text).expect("parse");
        assert_eq!(back[0].fields, ["x,y", "z"]);
    }
}
