//! Which of the three value shapes a stored value reads back as.
//!
//! `DataType\Manager::getForExtract` picks the data type a stored row is read
//! with, and that type's `getJsonLd`, `render` and `getFulltextText` decide the
//! shape. The core types name their shape. A term list type
//! (`DataType\CustomVocab`) holds any of the three and decides by the columns
//! the row fills: a value resource first, then a URI, else the text.

use crate::term_list;

/// The shape of a stored value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueShape {
    /// `@value`, optionally `@language`.
    Literal,
    /// `@id`, optionally `o:label`.
    Uri,
    /// A link to another resource.
    Resource,
}

impl ValueShape {
    /// The shape of a row with this `type`, given whether it fills `uri` and
    /// `value_resource_id`. A name no data type registers reads as text, which
    /// is the port's fallback for module types it does not know.
    #[must_use]
    pub fn of(data_type: &str, has_uri: bool, has_resource: bool) -> Self {
        if term_list::list_id(data_type).is_some() {
            return if has_resource {
                Self::Resource
            } else if has_uri {
                Self::Uri
            } else {
                Self::Literal
            };
        }
        if data_type == "uri" {
            Self::Uri
        } else if data_type == "resource" || data_type.starts_with("resource:") {
            Self::Resource
        } else {
            Self::Literal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_types_name_their_shape_whatever_the_columns() {
        assert_eq!(ValueShape::of("literal", true, true), ValueShape::Literal);
        assert_eq!(ValueShape::of("uri", false, false), ValueShape::Uri);
        assert_eq!(
            ValueShape::of("resource", false, false),
            ValueShape::Resource
        );
        assert_eq!(
            ValueShape::of("resource:media", false, true),
            ValueShape::Resource
        );
        assert_eq!(
            ValueShape::of("numeric:integer", true, false),
            ValueShape::Literal
        );
    }

    /// `DataType\CustomVocab::getJsonLd`: `valueResource()` first, then `uri()`.
    #[test]
    fn a_term_list_value_takes_the_shape_of_its_columns() {
        assert_eq!(
            ValueShape::of("customvocab:3", true, true),
            ValueShape::Resource
        );
        assert_eq!(
            ValueShape::of("customvocab:3", true, false),
            ValueShape::Uri
        );
        assert_eq!(
            ValueShape::of("customvocab:3", false, false),
            ValueShape::Literal
        );
    }
}
