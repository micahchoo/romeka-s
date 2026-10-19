//! What a merge changes, decided before anything is written.

use std::collections::{BTreeSet, HashSet};

use omeka_db::ids::{PropertyId, ResourceId, ValueId};
use omeka_db::rows::TextValueRow;

/// The values one merge rewrites and deletes, and the resources it changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MergePlan {
    pub rewrite: Vec<ValueId>,
    pub delete: Vec<ValueId>,
    pub changed: BTreeSet<ResourceId>,
}

/// Rewrite every value spelled one of `from` to `to`.
///
/// `rows` are the scope's values on resources the caller may edit, in id order. A value the merge
/// would make repeat an earlier value of its resource (same property, language and visibility, now
/// the same text) is deleted instead, unless it carries an annotation, which a delete would lose.
pub(crate) fn plan_merge(rows: &[&TextValueRow], from: &[String], to: &str) -> MergePlan {
    let merged = |text: &str| text == to || from.iter().any(|spelling| spelling == text);
    let mut plan = MergePlan::default();
    let mut kept: HashSet<(ResourceId, PropertyId, Option<&str>, bool)> = HashSet::new();
    for row in rows {
        if !merged(&row.text) {
            continue;
        }
        let slot = (
            row.resource_id,
            row.property_id,
            row.lang.as_deref(),
            row.is_public,
        );
        let repeats = !kept.insert(slot);
        if row.text == to {
            continue;
        }
        if repeats && !row.has_annotation {
            plan.delete.push(row.id);
        } else {
            plan.rewrite.push(row.id);
        }
        plan.changed.insert(row.resource_id);
    }
    plan
}

/// A list's words after a merge: each merged spelling leaves unless it is `retained`, and the kept
/// one takes the place of the first merged spelling when the list did not hold it already.
pub(crate) fn merged_terms(
    terms: &[String],
    from: &[String],
    retained: &[String],
    to: &str,
) -> Vec<String> {
    let mut merged = Vec::with_capacity(terms.len() + 1);
    let mut has_to = terms.iter().any(|term| term == to);
    for term in terms {
        if term != to && from.contains(term) {
            if !has_to {
                merged.push(to.to_string());
                has_to = true;
            }
            if retained.contains(term) {
                merged.push(term.clone());
            }
        } else {
            merged.push(term.clone());
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: u64, resource: u64, text: &str) -> TextValueRow {
        TextValueRow {
            id: ValueId(id),
            resource_id: ResourceId(resource),
            property_id: PropertyId(3),
            r#type: "literal".to_string(),
            lang: None,
            text: text.to_string(),
            is_public: true,
            has_annotation: false,
            resource_type: "Omeka\\Entity\\Item".to_string(),
            owner_id: None,
            resource_is_public: true,
            template_id: None,
        }
    }

    fn spellings(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|text| (*text).to_string()).collect()
    }

    #[test]
    fn every_merged_spelling_is_rewritten_and_the_rest_are_untouched() {
        let rows = [
            row(1, 10, "Co. Cork"),
            row(2, 11, "Cork City"),
            row(3, 12, "Cork"),
            row(4, 13, "Boston"),
        ];
        let refs: Vec<&TextValueRow> = rows.iter().collect();
        let plan = plan_merge(&refs, &spellings(&["Co. Cork", "Cork City"]), "Cork");
        assert_eq!(plan.rewrite, vec![ValueId(1), ValueId(2)]);
        assert!(plan.delete.is_empty());
        assert_eq!(
            plan.changed,
            [ResourceId(10), ResourceId(11)].into_iter().collect()
        );
    }

    #[test]
    fn a_value_the_merge_makes_repeat_is_deleted_unless_annotated() {
        let mut annotated = row(4, 20, "Cork City");
        annotated.has_annotation = true;
        let mut other_language = row(5, 20, "Co. Cork");
        other_language.lang = Some("ga".to_string());
        let rows = [
            row(1, 20, "Cork"),
            row(2, 20, "Co. Cork"),
            row(3, 21, "Co. Cork"),
            annotated,
            other_language,
            row(6, 21, "Cork City"),
        ];
        let refs: Vec<&TextValueRow> = rows.iter().collect();
        let plan = plan_merge(&refs, &spellings(&["Co. Cork", "Cork City"]), "Cork");
        assert_eq!(plan.rewrite, vec![ValueId(3), ValueId(4), ValueId(5)]);
        assert_eq!(plan.delete, vec![ValueId(2), ValueId(6)]);
    }

    #[test]
    fn the_kept_spelling_among_the_merged_ones_changes_nothing() {
        let rows = [row(1, 10, "Cork"), row(2, 10, "Cork")];
        let refs: Vec<&TextValueRow> = rows.iter().collect();
        let plan = plan_merge(&refs, &spellings(&["Cork"]), "Cork");
        assert_eq!(plan, MergePlan::default());
    }

    #[test]
    fn the_kept_spelling_takes_the_place_of_the_first_merged_term() {
        let terms = spellings(&["Bantry", "Co. Cork", "Boston", "Cork City"]);
        assert_eq!(
            merged_terms(&terms, &spellings(&["Co. Cork", "Cork City"]), &[], "Cork"),
            spellings(&["Bantry", "Cork", "Boston"])
        );
        let with_cork = spellings(&["Cork", "Co. Cork", "Boston"]);
        assert_eq!(
            merged_terms(&with_cork, &spellings(&["Co. Cork"]), &[], "Cork"),
            spellings(&["Cork", "Boston"])
        );
        assert_eq!(
            merged_terms(
                &spellings(&["Boston"]),
                &spellings(&["Co. Cork"]),
                &[],
                "Cork"
            ),
            spellings(&["Boston"])
        );
    }

    #[test]
    fn a_retained_spelling_stays_after_the_kept_one() {
        let terms = spellings(&["Bantry", "Cork City", "Co. Cork"]);
        assert_eq!(
            merged_terms(
                &terms,
                &spellings(&["Cork City", "Co. Cork"]),
                &spellings(&["Cork City"]),
                "Cork"
            ),
            spellings(&["Bantry", "Cork", "Cork City"])
        );
    }
}
