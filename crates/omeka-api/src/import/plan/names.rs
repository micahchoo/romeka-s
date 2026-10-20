//! The names the plan's members point with: ids, terms, slots, record kinds.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A source's id in the plan. Sheets and file entries name sources by it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceId(pub String);

/// A sheet's id in the plan. Column refs and row entries name sheets by it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SheetId(pub String);

/// A property term, `prefix:local`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Term(String);

impl Term {
    /// # Errors
    /// The text, when it is not a non-empty prefix, one `:`, and a non-empty
    /// local name, with no white space.
    pub fn parse(text: &str) -> Result<Self, String> {
        let well_formed = text.split_once(':').is_some_and(|(prefix, local)| {
            !prefix.is_empty() && !local.is_empty() && !local.contains(':')
        }) && !text.chars().any(|c| c.is_whitespace() || c.is_control());
        if well_formed {
            Ok(Self(text.to_string()))
        } else {
            Err(format!("{text:?} is not a term (prefix:local)"))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The term every record kind matches on by default.
    #[must_use]
    pub fn dcterms_identifier() -> Self {
        Self("dcterms:identifier".to_string())
    }

    /// The term a `newTerms` label makes in the `local` vocabulary, or
    /// `None` when the label has no letters or digits.
    #[must_use]
    pub fn local(label: &str) -> Option<Self> {
        let term = local_term(label);
        Self::parse(&term).ok()
    }
}

/// `local:` and the label in lower camel case, letters and digits of any
/// script: "Shelf mark" is `local:shelfMark`. Copied from v1, which the
/// cutover deletes.
#[must_use]
pub fn local_term(label: &str) -> String {
    let mut out = String::from("local:");
    let words = label
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty());
    for (index, word) in words.enumerate() {
        let lower = word.to_lowercase();
        let mut chars = lower.chars();
        if index > 0
            && let Some(first) = chars.next()
        {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        } else {
            out.push_str(&lower);
        }
    }
    out
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for Term {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Term {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// The three kinds of record an import writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecordKind {
    Item,
    Media,
    ItemSet,
}

/// One property of a record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SlotName {
    Sets,
    Template,
    Class,
    Owner,
    Visibility,
    Sites,
    MediaOrder,
    Ingester,
    MediaSource,
    ItemKey,
    Term(Term),
}

impl SlotName {
    /// The named slots, in the contract's order. Every other slot is a term.
    pub const NAMED: [Self; 10] = [
        Self::Sets,
        Self::Template,
        Self::Class,
        Self::Owner,
        Self::Visibility,
        Self::Sites,
        Self::MediaOrder,
        Self::Ingester,
        Self::MediaSource,
        Self::ItemKey,
    ];

    /// Whether the slot keeps every value. On a single-valued slot only the
    /// first value counts, and `combine: all` is refused.
    #[must_use]
    pub fn is_multi(&self) -> bool {
        match self {
            Self::Sets | Self::Sites | Self::Term(_) => true,
            Self::Template
            | Self::Class
            | Self::Owner
            | Self::Visibility
            | Self::MediaOrder
            | Self::Ingester
            | Self::MediaSource
            | Self::ItemKey => false,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Sets => "sets",
            Self::Template => "template",
            Self::Class => "class",
            Self::Owner => "owner",
            Self::Visibility => "visibility",
            Self::Sites => "sites",
            Self::MediaOrder => "mediaOrder",
            Self::Ingester => "ingester",
            Self::MediaSource => "mediaSource",
            Self::ItemKey => "itemKey",
            Self::Term(term) => term.as_str(),
        }
    }

    /// # Errors
    /// The text, when it is neither a named slot nor a term.
    pub fn parse(text: &str) -> Result<Self, String> {
        Self::NAMED
            .into_iter()
            .find(|slot| slot.as_str() == text)
            .map_or_else(|| Term::parse(text).map(Self::Term), Ok)
    }
}

impl fmt::Display for SlotName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for SlotName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SlotName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_and_multi_valued_slots_follow_the_contract() {
        let multi: Vec<&str> = SlotName::NAMED
            .iter()
            .filter(|slot| slot.is_multi())
            .map(SlotName::as_str)
            .collect();
        assert_eq!(multi, ["sets", "sites"]);
        for slot in SlotName::NAMED {
            assert_eq!(SlotName::parse(slot.as_str()), Ok(slot.clone()));
        }
        let title = SlotName::parse("dcterms:title");
        assert!(title.as_ref().is_ok_and(SlotName::is_multi), "{title:?}");
    }

    #[test]
    fn a_term_is_prefix_colon_local() {
        for good in ["dcterms:title", "local:दिनांक", "foaf:Person"] {
            assert!(Term::parse(good).is_ok(), "{good}");
        }
        for bad in ["title", ":title", "dcterms:", "a:b:c", "dc terms:title", ""] {
            assert!(Term::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_local_term_keeps_letters_of_any_script() {
        assert_eq!(local_term("Shelf mark"), "local:shelfMark");
        assert_eq!(local_term("  OLD accession-no. "), "local:oldAccessionNo");
        assert_eq!(local_term("दिनांक"), "local:दिनांक");
        assert_eq!(Term::local(" - "), None);
    }
}
