//! The four vocabularies `romeka install` stores, read from upstream's own files.
//!
//! Upstream's `InstallDefaultVocabulariesTask` imports `dcterms.rdf`, `dctype.rdf`, `bibo.owl`
//! and `foaf.rdf` with `RdfImporter`. `omeka_api::vocab_import::defaults` bundles the same four
//! files at the frozen SHA, and the same importer reads them, so the install stores what
//! upstream's does.
//!
//! The install converges a database on the files rather than only filling an empty one. An
//! install made from an older bundle, which gave every FOAF member its local name as its label
//! and no comment and left out `foaf:givenname`, is corrected by the next `romeka install`:
//! a member the file has and the database lacks is added, and a member that still holds only
//! that fallback text (label equal to its local name, no comment) takes the file's label and
//! comment. A label or comment anyone wrote is never touched, so a second run changes nothing.

use omeka_api::vocab_import::Term;
use omeka_api::vocab_import::defaults::DEFAULT_VOCABULARIES;
use omeka_db::DbPool;
use omeka_db::ids::VocabularyId;
use omeka_db::repos::{
    ClassFilter, ClassRepo, Paging, PropertyFilter, PropertyRepo, VocabularyRepo,
};
use omeka_db::rows::{NewClass, NewProperty, NewVocabulary};
use omeka_db::with_db;

use crate::error::CliError;

/// What converging on the bundled files wrote.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Converged {
    pub vocabularies_inserted: usize,
    pub classes_inserted: usize,
    pub properties_inserted: usize,
    /// Members whose fallback label and comment were replaced by the file's.
    pub members_corrected: usize,
}

/// Store every default vocabulary and member the database lacks, and correct the members an
/// older bundle left with fallback text. A vocabulary that holds a default prefix under another
/// namespace is someone else's and is left alone.
///
/// # Errors
/// A [`CliError`] for a storage failure or an unparsable bundled file.
pub async fn converge(pool: &DbPool) -> Result<Converged, CliError> {
    let mut converged = Converged::default();
    for vocabulary in &DEFAULT_VOCABULARIES {
        let row = match with_db!(pool, |db| VocabularyRepo::new(db)
            .get_by_prefix(vocabulary.prefix)
            .await)?
        {
            Some(row) if row.namespace_uri == vocabulary.namespace_uri => row,
            Some(_) => continue,
            None => {
                converged.vocabularies_inserted += 1;
                with_db!(pool, |db| VocabularyRepo::new(db)
                    .create(&NewVocabulary {
                        owner_id: None,
                        namespace_uri: vocabulary.namespace_uri.to_string(),
                        prefix: vocabulary.prefix.to_string(),
                        label: vocabulary.label.to_string(),
                        comment: Some(vocabulary.comment.to_string()),
                    })
                    .await)?
            }
        };
        let file = vocabulary.members().map_err(|error| {
            CliError::message(format!("bundled {}: {error}", vocabulary.prefix))
        })?;
        for (kind, terms) in [
            (Kind::Class, &file.classes),
            (Kind::Property, &file.properties),
        ] {
            let stored = kind.list(pool, row.id).await?;
            let plan = plan(&stored, terms);
            for (id, wanted) in plan.corrected {
                kind.update(pool, id, wanted).await?;
                converged.members_corrected += 1;
            }
            for wanted in plan.new {
                kind.create(pool, row.id, wanted).await?;
                match kind {
                    Kind::Class => converged.classes_inserted += 1,
                    Kind::Property => converged.properties_inserted += 1,
                }
            }
        }
    }
    Ok(converged)
}

/// A stored member: its row id and its text.
type Stored = (u64, Term);

/// The writes that bring one member list to the file.
#[derive(Debug, PartialEq, Eq)]
struct Plan<'a> {
    /// Members the file has and the database lacks, in the file's order.
    new: Vec<&'a Term>,
    /// Stored members holding only fallback text, with the file's text for them.
    corrected: Vec<(u64, &'a Term)>,
}

fn plan<'a>(stored: &[Stored], file: &'a [Term]) -> Plan<'a> {
    let find = |local_name: &str| {
        stored
            .iter()
            .find(|(_, term)| term.local_name == local_name)
    };
    Plan {
        new: file
            .iter()
            .filter(|wanted| find(&wanted.local_name).is_none())
            .collect(),
        corrected: file
            .iter()
            .filter_map(|wanted| {
                let (id, term) = find(&wanted.local_name)?;
                let fallback = term.label == term.local_name && term.comment.is_none();
                (fallback && term != wanted).then_some((*id, wanted))
            })
            .collect(),
    }
}

/// The two member tables, which hold the same columns.
#[derive(Debug, Clone, Copy)]
enum Kind {
    Class,
    Property,
}

impl Kind {
    async fn list(
        self,
        pool: &DbPool,
        vocabulary_id: VocabularyId,
    ) -> Result<Vec<Stored>, CliError> {
        let term = |local_name: String, label: String, comment: Option<String>| Term {
            local_name,
            label,
            comment,
        };
        Ok(match self {
            Self::Class => with_db!(pool, |db| ClassRepo::new(db)
                .list(
                    &ClassFilter {
                        vocabulary_id: Some(vocabulary_id),
                        ..ClassFilter::default()
                    },
                    &Paging::default()
                )
                .await)?
            .into_iter()
            .map(|row| (row.id.0, term(row.local_name, row.label, row.comment)))
            .collect(),
            Self::Property => with_db!(pool, |db| PropertyRepo::new(db)
                .list(
                    &PropertyFilter {
                        vocabulary_id: Some(vocabulary_id),
                        ..PropertyFilter::default()
                    },
                    &Paging::default()
                )
                .await)?
            .into_iter()
            .map(|row| (row.id.0, term(row.local_name, row.label, row.comment)))
            .collect(),
        })
    }

    async fn create(
        self,
        pool: &DbPool,
        vocabulary_id: VocabularyId,
        term: &Term,
    ) -> Result<(), CliError> {
        let (local_name, label, comment) = (
            term.local_name.clone(),
            term.label.clone(),
            term.comment.clone(),
        );
        match self {
            Self::Class => {
                with_db!(pool, |db| ClassRepo::new(db)
                    .create(&NewClass {
                        owner_id: None,
                        vocabulary_id,
                        local_name,
                        label,
                        comment,
                    })
                    .await)?;
            }
            Self::Property => {
                with_db!(pool, |db| PropertyRepo::new(db)
                    .create(&NewProperty {
                        owner_id: None,
                        vocabulary_id,
                        local_name,
                        label,
                        comment,
                    })
                    .await)?;
            }
        }
        Ok(())
    }

    /// Rewrite the label and comment of one stored member, keeping its owner.
    async fn update(self, pool: &DbPool, id: u64, term: &Term) -> Result<(), CliError> {
        match self {
            Self::Class => {
                if let Some(mut row) = with_db!(pool, |db| ClassRepo::new(db).get(id.into()).await)?
                {
                    row.label.clone_from(&term.label);
                    row.comment.clone_from(&term.comment);
                    with_db!(pool, |db| ClassRepo::new(db).update(&row).await)?;
                }
            }
            Self::Property => {
                if let Some(mut row) =
                    with_db!(pool, |db| PropertyRepo::new(db).get(id.into()).await)?
                {
                    row.label.clone_from(&term.label);
                    row.comment.clone_from(&term.comment);
                    with_db!(pool, |db| PropertyRepo::new(db).update(&row).await)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(local_name: &str, label: &str, comment: Option<&str>) -> Term {
        Term {
            local_name: local_name.to_string(),
            label: label.to_string(),
            comment: comment.map(str::to_string),
        }
    }

    /// A member is corrected only while it holds the bare local name and no comment.
    #[test]
    fn only_fallback_text_is_corrected() {
        let stored = [
            (1, term("name", "name", None)),
            (2, term("nick", "Nickname (mine)", None)),
            (3, term("age", "age", Some("Written here."))),
            (4, term("title", "title", None)),
        ];
        let file = [
            term("name", "name", Some("A name.")),
            term("nick", "nickname", None),
            term("age", "age", Some("The age.")),
            term("title", "title", None),
            term("knows", "knows", None),
        ];
        let plan = plan(&stored, &file);
        assert_eq!(plan.new, [&file[4]]);
        assert_eq!(plan.corrected, [(1, &file[0])]);
    }
}
