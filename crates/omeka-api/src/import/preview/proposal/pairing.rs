//! Detector 6, pairing suggestions (design section 2): join rules that
//! would pair more of a list's rows. Each is an offer; nothing enters the
//! plan until a `pair` edit takes it.
//!
//! A rule compares only the last places of the cell and of a file's name
//! (`places::last_places`, the same comparison the join's `lastPlaces` fold
//! makes). It is offered when it pairs at least [`MIN_SUGGESTION_SHARE`] of
//! the list's rows, each to exactly one file of the whole archive, all in
//! one series, and more rows than the stated join. Of these rules, the
//! fewest places win. A list that pairs no row by any stated rule describes
//! the series its suggestion pairs into (detector 3, rule 3).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use unicode_normalization::UnicodeNormalization;

use super::lists::{Counted, Found, InSeries};
use super::thresholds::MIN_SUGGESTION_SHARE;
use super::{Evidence, PairCounts, Pick, Share};
use crate::import::catalog;
use crate::import::join::cell;
use crate::import::places::{last_places, places, stem};
use crate::import::plan::{ColumnRef, PairRule, Placement, Structure};
use crate::import::sheet::Sheet;

/// What comparing the last `places` places of a column's cells pairs.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct RuleCounts {
    /// Rows whose cell finds exactly one file.
    pub one_file: usize,
    /// Rows whose cell finds two files or more.
    pub several_files: usize,
    /// The rows that find a file, by the series of the files they find
    /// ([`SeriesIx::index`]; `None` is outside every series).
    ///
    /// [`SeriesIx::index`]: crate::import::plan::SeriesIx::index
    pub series: BTreeMap<Option<usize>, usize>,
}

/// Every media file of the archive by the last places of its name, made
/// once per count of places.
pub(super) struct Media<'e> {
    files: Vec<(&'e str, Option<usize>)>,
    by_last: HashMap<usize, HashMap<String, Vec<Option<usize>>>>,
}

impl<'e> Media<'e> {
    /// The media of `evidence`, each with the series of `structure` that
    /// takes it.
    pub fn of(evidence: &Evidence<'e>, structure: &Structure) -> Self {
        let files = evidence
            .index
            .sources
            .iter()
            .flat_map(|source| source.walk.entries.iter().map(move |entry| (source, entry)))
            .filter(|(_, entry)| {
                catalog::refusal(&evidence.catalog.files, entry.name(), &entry.sniffed).is_none()
            })
            .map(|(source, entry)| {
                let series = match structure.place(&source.id, &entry.path) {
                    Placement::Series { at, .. } => Some(at.index()),
                    Placement::Outside => None,
                };
                (entry.path.as_str(), series)
            })
            .collect();
        Self {
            files,
            by_last: HashMap::new(),
        }
    }

    fn by_last(&mut self, count: usize) -> &HashMap<String, Vec<Option<usize>>> {
        let files = &self.files;
        self.by_last.entry(count).or_insert_with(|| {
            let mut found: HashMap<String, Vec<Option<usize>>> = HashMap::new();
            for (path, series) in files {
                let last = last_places(path, count);
                if !last.is_empty() {
                    found.entry(last).or_default().push(*series);
                }
            }
            found
        })
    }

    /// What the rule `last places` pairs of `column`'s cells. A cell with
    /// no more places than that is never paired: its whole text is not a
    /// last part of it.
    pub fn count(&mut self, sheet: &Sheet, column: u32, last: usize) -> RuleCounts {
        let by_last = self.by_last(last);
        let mut counts = RuleCounts::default();
        for row in &sheet.rows {
            let text: String = cell(row, column).trim().nfc().collect();
            if places(stem(&text)).len() <= last {
                continue;
            }
            let Some(found) = by_last.get(&last_places(&text, last)) else {
                continue;
            };
            match found.len() {
                1 => counts.one_file += 1,
                _ => counts.several_files += 1,
            }
            let series: BTreeSet<Option<usize>> = found.iter().copied().collect();
            for series in series {
                *counts.series.entry(series).or_default() += 1;
            }
        }
        counts
    }
}

/// A rule offered for one list.
pub(super) struct Offer {
    pub column: ColumnRef,
    pub series: usize,
    pub pick: Pick<PairRule, PairCounts>,
}

/// The rule offered for `column`: the fewest last places whose counts pass.
fn offer(
    media: &mut Media<'_>,
    sheet: &Sheet,
    column: &ColumnRef,
    stated: usize,
) -> Option<(usize, Pick<PairRule, PairCounts>)> {
    let most = sheet
        .rows
        .iter()
        .map(|row| places(stem(cell(row, column.index).trim())).len())
        .max()?;
    let rows = sheet.rows.len();
    (1..most).find_map(|last| {
        let counts = media.count(sheet, column.index, last);
        let mut series = counts.series.keys();
        let (Some(Some(only)), None) = (series.next(), series.next()) else {
            return None;
        };
        let share = counts.one_file * MIN_SUGGESTION_SHARE.of >= MIN_SUGGESTION_SHARE.n * rows;
        let passes = counts.several_files == 0 && share && counts.one_file > stated;
        let places = u8::try_from(last).ok()?;
        passes.then(|| {
            (
                *only,
                Pick {
                    value: PairRule::LastPlaces { places },
                    counts: PairCounts {
                        rows: Share {
                            n: counts.one_file,
                            of: rows,
                        },
                        more: counts.one_file - stated,
                    },
                },
            )
        })
    })
}

/// Detector 6 on what detector 3 chose, over the series of `structure` (the
/// proposed patterns). Each series' first list with unpaired rows is tried
/// on its join column; each list that pairs no row anywhere is tried on
/// every column but row numbers, and describes the series its offer pairs
/// into. Gives each series' offers, by [`SeriesIx::index`].
///
/// [`SeriesIx::index`]: crate::import::plan::SeriesIx::index
pub(super) fn detect(
    evidence: &Evidence<'_>,
    structure: &Structure,
    found: &mut Found<'_>,
) -> Vec<Vec<Offer>> {
    let mut media = Media::of(evidence, structure);
    let mut offers: Vec<Vec<Offer>> = found.series.iter().map(|_| Vec::new()).collect();
    for (at, lists) in found.series.iter().enumerate() {
        let Some(first) = lists.joined.first() else {
            continue;
        };
        let (Some(join), stated) = (&first.counted.join, first.counted.paired) else {
            continue;
        };
        evidence.lists.read(&first.file, &mut |sheet| {
            let Ok(sheet) = sheet else {
                return;
            };
            if stated >= sheet.rows.len() {
                return;
            }
            if let Some((series, pick)) = offer(&mut media, sheet, &join.column, stated)
                && series == at
            {
                offers[at].push(Offer {
                    column: join.column.clone(),
                    series,
                    pick,
                });
            }
        });
    }
    let mut attached = Vec::new();
    for (place, list) in found.unattached.iter().enumerate() {
        if !list.pairs_nothing {
            continue;
        }
        evidence.lists.read(&list.file, &mut |sheet| {
            let Ok(sheet) = sheet else {
                return;
            };
            let best = sheet
                .columns
                .iter()
                .filter(|column| !super::grain::numbers_rows(sheet, column.index))
                .filter_map(|column| {
                    let column = ColumnRef {
                        index: column.index,
                        header: column.header.clone(),
                    };
                    let (series, pick) = offer(&mut media, sheet, &column, 0)?;
                    Some((column, series, pick))
                })
                .min_by_key(|(column, _, pick)| {
                    (std::cmp::Reverse(pick.counts.rows.n), column.index)
                });
            if let Some((column, series, pick)) = best {
                attached.push((
                    place,
                    Offer {
                        column,
                        series,
                        pick,
                    },
                ));
            }
        });
    }
    for (place, offer) in attached.into_iter().rev() {
        let list = found.unattached.remove(place);
        if let Some(lists) = found.series.get_mut(offer.series) {
            lists.joined.push(InSeries {
                file: list.file,
                counted: Counted::by_suggestion(list.counts, offer.column.clone()),
            });
            offers[offer.series].push(offer);
        }
    }
    offers
}
