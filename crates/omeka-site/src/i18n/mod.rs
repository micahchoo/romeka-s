//! The public site's locales: the message catalogue the theme and the blocks
//! render, and the locale-aware date formatter.
//!
//! Upstream reads a site's `locale` setting and switches the translator and the
//! default locale for the request (`Mvc\MvcListeners::bootstrapLocale`,
//! `application/src/Mvc/MvcListeners.php:376-381`). The port keeps the same
//! setting key (`site_setting.locale`) and the same effect: the theme and the
//! blocks render through a [`Translator`] built from it.
//!
//! Upstream's catalogues are gettext `.po`/`.mo` files under
//! `application/language/` (30 locales plus `template.pot` at this SHA).
//! DESIGN.md excludes full catalog completeness, so the port ships English (the
//! source language), Spanish, whose terms follow upstream's `es.po` so an
//! overlapping string has its upstream translation, and the seven Indic locales
//! the admin ships, machine-translated ([`machine_translated`]).
//!
//! Dates follow upstream's `I18n::dateFormat`; [`dates`] says how.

use chrono::NaiveDateTime;

mod dates;
mod dates_cldr;
mod machine_translated;
mod messages;
mod search_labels;
mod vocabulary;

use dates::{DateNames, EN_NAMES, ES_NAMES};
use dates_cldr::{HI_NAMES, KN_NAMES, ML_NAMES, MR_NAMES, NE_NAMES, TA_NAMES, TE_NAMES};
use machine_translated::Catalogue;
pub use messages::{EN, ES, Messages};
pub use search_labels::{EN_SEARCH, ES_SEARCH, SearchLabels};
pub use vocabulary::term_label;

/// A locale the site layer can render in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    /// English, the source language and the default.
    #[default]
    En,
    /// Spanish, the shipped second locale.
    Es,
    /// Hindi, machine-translated.
    Hi,
    /// Marathi, machine-translated.
    Mr,
    /// Malayalam, machine-translated.
    Ml,
    /// Tamil, machine-translated.
    Ta,
    /// Kannada, machine-translated.
    Kn,
    /// Telugu, machine-translated.
    Te,
    /// Nepali, machine-translated.
    Ne,
}

/// The tables one locale renders with.
struct Tables {
    messages: &'static Messages,
    search: &'static SearchLabels,
    names: &'static DateNames,
}

impl Tables {
    fn machine(catalogue: &'static Catalogue, names: &'static DateNames) -> Self {
        Self {
            messages: &catalogue.messages,
            search: &catalogue.search,
            names,
        }
    }
}

impl Locale {
    /// Every shipped locale, in the order the site settings offer them.
    pub const ALL: [Self; 9] = [
        Self::En,
        Self::Es,
        Self::Hi,
        Self::Mr,
        Self::Ml,
        Self::Ta,
        Self::Kn,
        Self::Te,
        Self::Ne,
    ];

    /// Every shipped locale's tag, the choices of `site_setting.locale`.
    pub const TAGS: [&'static str; 9] = {
        let mut tags = [""; 9];
        let mut index = 0;
        while index < tags.len() {
            tags[index] = Self::ALL[index].tag();
            index += 1;
        }
        tags
    };

    /// The BCP 47 tag, the value `site_setting.locale` stores.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Es => "es",
            Self::Hi => "hi",
            Self::Mr => "mr",
            Self::Ml => "ml",
            Self::Ta => "ta",
            Self::Kn => "kn",
            Self::Te => "te",
            Self::Ne => "ne",
        }
    }

    /// The locale a stored tag names. A bare language matches its region tags
    /// (`en_US` is English, `hi_IN` is Hindi), and an unknown tag falls back to
    /// [`Locale::En`], the installation default.
    #[must_use]
    pub fn parse(tag: &str) -> Self {
        let language = tag.split(['_', '-']).next().unwrap_or("");
        Self::ALL
            .into_iter()
            .find(|locale| locale.tag() == language)
            .unwrap_or_default()
    }

    /// The translator for this locale.
    #[must_use]
    pub const fn translator(self) -> Translator {
        Translator { locale: self }
    }

    fn tables(self) -> Tables {
        match self {
            Self::En => Tables {
                messages: &EN,
                search: &EN_SEARCH,
                names: &EN_NAMES,
            },
            Self::Es => Tables {
                messages: &ES,
                search: &ES_SEARCH,
                names: &ES_NAMES,
            },
            Self::Hi => Tables::machine(&machine_translated::HI, &HI_NAMES),
            Self::Mr => Tables::machine(&machine_translated::MR, &MR_NAMES),
            Self::Ml => Tables::machine(&machine_translated::ML, &ML_NAMES),
            Self::Ta => Tables::machine(&machine_translated::TA, &TA_NAMES),
            Self::Kn => Tables::machine(&machine_translated::KN, &KN_NAMES),
            Self::Te => Tables::machine(&machine_translated::TE, &TE_NAMES),
            Self::Ne => Tables::machine(&machine_translated::NE, &NE_NAMES),
        }
    }
}

/// `sprintf($pattern, $value)` for the one-argument patterns the search views
/// print, e.g. `Search results for “%s”`.
#[must_use]
pub fn sprintf1(pattern: &str, value: &str) -> String {
    pattern.replacen("%s", value, 1)
}

/// `sprintf($pattern, ...$values)` for `%s` patterns with several arguments,
/// e.g. `%s with "%s: %s"`. Each `%s` takes the next value; a value is never
/// scanned again, so a title that contains `%s` prints as written.
#[must_use]
pub fn sprintf(pattern: &str, values: &[&str]) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut parts = pattern.split("%s");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    let mut values = values.iter();
    for part in parts {
        out.push_str(values.next().copied().unwrap_or(""));
        out.push_str(part);
    }
    out
}

/// A locale and its catalogue, the value the theme and the blocks render with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Translator {
    locale: Locale,
}

impl Translator {
    /// A translator for one locale.
    #[must_use]
    pub const fn new(locale: Locale) -> Self {
        Self { locale }
    }

    /// The locale in force.
    #[must_use]
    pub const fn locale(self) -> Locale {
        self.locale
    }

    /// The catalogue for this locale.
    #[must_use]
    pub fn messages(self) -> &'static Messages {
        self.locale.tables().messages
    }

    /// The search labels for this locale.
    #[must_use]
    pub fn search_labels(self) -> &'static SearchLabels {
        self.locale.tables().search
    }

    /// `I18n::dateFormat` for this locale: the `date_format` and `time_format`
    /// a `pageDateTime` block stores, applied to a naive timestamp.
    ///
    /// `full` asks for a timezone the naive timestamp cannot supply, so the
    /// port prints none there, as the block module documents.
    #[must_use]
    pub fn format_datetime(
        self,
        value: NaiveDateTime,
        date_format: &str,
        time_format: &str,
    ) -> String {
        dates::format_datetime(value, date_format, time_format, self.locale.tables().names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-10-16 09:30:00", "%Y-%m-%d %H:%M:%S").unwrap()
    }

    /// A stored tag narrows to a locale, and a region tag to its language.
    #[test]
    fn locale_parse_accepts_a_language_and_a_region() {
        assert_eq!(Locale::parse("en_US"), Locale::En);
        assert_eq!(Locale::parse("es"), Locale::Es);
        assert_eq!(Locale::parse("es_419"), Locale::Es);
        assert_eq!(Locale::parse("hi_IN"), Locale::Hi);
        assert_eq!(Locale::parse("ne-NP"), Locale::Ne);
        assert_eq!(Locale::parse("ta"), Locale::Ta);
        assert_eq!(Locale::parse("de_DE"), Locale::En);
        assert_eq!(Locale::parse(""), Locale::En);
        assert_eq!(Locale::default(), Locale::En);
    }

    /// The tag round-trips through parse.
    #[test]
    fn locale_tags_round_trip() {
        for locale in Locale::ALL {
            assert_eq!(Locale::parse(locale.tag()), locale);
        }
        assert_eq!(Locale::TAGS.map(Locale::parse), Locale::ALL);
        assert_eq!(
            Locale::TAGS,
            ["en", "es", "hi", "mr", "ml", "ta", "kn", "te", "ne"]
        );
    }

    /// Each Indic locale prints what ICU 78.2 (CLDR 48) prints for the same
    /// timestamp, `Intl.DateTimeFormat(tag, { dateStyle | timeStyle })` in node
    /// 24.14: the CLDR names, the CLDR order and, for `mr` and `ne`, the
    /// Devanagari digits of their default numbering system. The timestamp is
    /// not the generator's sample, so the patterns are tested, not echoed.
    #[test]
    fn indic_dates_match_icu() {
        let morning = stamp();
        let evening =
            NaiveDateTime::parse_from_str("2026-10-16 21:05:07", "%Y-%m-%d %H:%M:%S").unwrap();
        let icu = [
            (
                Locale::Hi,
                [
                    "16/10/26",
                    "16 अक्टू॰ 2026",
                    "16 अक्टूबर 2026",
                    "शुक्रवार, 16 अक्टूबर 2026",
                    "9:30 am",
                    "9:05:07 pm",
                ],
            ),
            (
                Locale::Mr,
                [
                    "१६/१०/२६",
                    "१६ ऑक्टो, २०२६",
                    "१६ ऑक्टोबर, २०२६",
                    "शुक्रवार, १६ ऑक्टोबर, २०२६",
                    "९:३० AM",
                    "९:०५:०७ PM",
                ],
            ),
            (
                Locale::Ml,
                [
                    "16/10/26",
                    "2026 ഒക്ടോ 16",
                    "2026 ഒക്‌ടോബർ 16",
                    "2026 ഒക്‌ടോബർ 16, വെള്ളിയാഴ്‌ച",
                    "9:30 AM",
                    "9:05:07 PM",
                ],
            ),
            (
                Locale::Ta,
                [
                    "16/10/26",
                    "16 அக்., 2026",
                    "16 அக்டோபர், 2026",
                    "வெள்ளி, 16 அக்டோபர், 2026",
                    "9:30 AM",
                    "9:05:07 PM",
                ],
            ),
            (
                Locale::Kn,
                [
                    "16/10/26",
                    "ಅಕ್ಟೋ 16, 2026",
                    "ಅಕ್ಟೋಬರ್ 16, 2026",
                    "ಶುಕ್ರವಾರ, ಅಕ್ಟೋಬರ್ 16, 2026",
                    "09:30 AM",
                    "09:05:07 PM",
                ],
            ),
            (
                Locale::Te,
                [
                    "16-10-26",
                    "16 అక్టో, 2026",
                    "16 అక్టోబర్, 2026",
                    "16, అక్టోబర్ 2026, శుక్రవారం",
                    "9:30 AM",
                    "9:05:07 PM",
                ],
            ),
            (
                Locale::Ne,
                [
                    "२६/१०/१६",
                    "२०२६ अक्टोबर १६",
                    "२०२६ अक्टोबर १६",
                    "२०२६ अक्टोबर १६, शुक्रबार",
                    "०९:३०",
                    "२१:०५:०७",
                ],
            ),
        ];
        for (locale, [short, medium, long, full, time_short, time_medium]) in icu {
            let tr = locale.translator();
            assert_eq!(
                tr.format_datetime(morning, "short", "none"),
                short,
                "{locale:?}"
            );
            assert_eq!(
                tr.format_datetime(morning, "medium", "none"),
                medium,
                "{locale:?}"
            );
            assert_eq!(
                tr.format_datetime(morning, "long", "none"),
                long,
                "{locale:?}"
            );
            assert_eq!(
                tr.format_datetime(morning, "full", "none"),
                full,
                "{locale:?}"
            );
            assert_eq!(
                tr.format_datetime(morning, "none", "short"),
                time_short,
                "{locale:?}"
            );
            assert_eq!(
                tr.format_datetime(evening, "none", "medium"),
                time_medium,
                "{locale:?}"
            );
        }
        // A date and a time join with a space, as the fallback branch joins them.
        assert_eq!(
            Locale::Hi
                .translator()
                .format_datetime(morning, "long", "short"),
            "16 अक्टूबर 2026 9:30 am"
        );
    }

    /// English reproduces `I18n::dateFormat`'s fallback patterns.
    #[test]
    fn translator_formats_dates_like_upstream_i18n() {
        let tr = Locale::En.translator();
        let value = stamp();
        assert_eq!(tr.format_datetime(value, "short", "none"), "10/16/26");
        assert_eq!(tr.format_datetime(value, "medium", "none"), "Oct 16, 2026");
        assert_eq!(
            tr.format_datetime(value, "long", "none"),
            "October 16, 2026"
        );
        assert_eq!(
            tr.format_datetime(value, "full", "none"),
            "Friday, October 16, 2026"
        );
        assert_eq!(tr.format_datetime(value, "none", "short"), "9:30am");
        assert_eq!(tr.format_datetime(value, "none", "long"), "9:30:00am");
        assert_eq!(tr.format_datetime(value, "none", "none"), "");
    }

    /// Spanish renders the same timestamp with Spanish names and ICU `es`
    /// patterns: `16 de octubre de 2026`.
    #[test]
    fn translator_formats_dates_in_the_locale() {
        let tr = Locale::Es.translator();
        let value = stamp();
        assert_eq!(tr.format_datetime(value, "short", "none"), "16/10/26");
        assert_eq!(tr.format_datetime(value, "medium", "none"), "16 oct 2026");
        assert_eq!(
            tr.format_datetime(value, "long", "none"),
            "16 de octubre de 2026"
        );
        assert_eq!(
            tr.format_datetime(value, "full", "none"),
            "viernes, 16 de octubre de 2026"
        );
        assert_eq!(tr.format_datetime(value, "none", "short"), "9:30am");
    }

    /// Every catalogue names every message, and the two differ.
    #[test]
    fn messages_are_present_in_both_locales() {
        assert_eq!(
            Locale::En.translator().messages().powered_by,
            "Powered by Romeka S"
        );
        assert_eq!(
            Locale::Es.translator().messages().powered_by,
            "Funciona con Romeka S"
        );
        assert_ne!(
            Locale::En.translator().messages().item_sets,
            Locale::Es.translator().messages().item_sets
        );
        assert_eq!(Locale::Es.translator().locale(), Locale::Es);
    }

    /// The search catalogue differs per locale where `es.po` has the term, and
    /// every comparison carries a value and a label.
    #[test]
    fn search_labels_follow_the_spanish_catalogue() {
        let en = Locale::En.translator().search_labels();
        let es = Locale::Es.translator().search_labels();
        assert_eq!(en.search, "Search");
        assert_eq!(es.search, "Búsqueda");
        assert_eq!(en.search_full_text, "Search full-text");
        assert_eq!(es.search_full_text, "Buscar en cualquier campo");
        // The `type` select's values are the wire names `buildPropertyQuery`
        // reads, so they stay English in both catalogues.
        assert_eq!(en.comparisons[0], ("eq", "is exactly"));
        assert_eq!(es.comparisons[0], ("eq", "es exactamente"));
        assert_eq!(en.comparisons[13], ("ndt", "does not have data type"));
        assert_eq!(es.comparisons[13], ("ndt", "does not have data type"));
        for (value, label) in en.comparisons {
            assert!(!value.is_empty() && !label.is_empty(), "{value}");
        }
        assert_eq!(en.sort_fields[0], ("title", "Title"));
        assert_eq!(es.sort_fields[0], ("title", "Título"));
        assert_eq!(es.sort_fields[1], ("created", "Creado"));
        assert_eq!(
            es.form_title(crate::search::SearchResource::ItemSets),
            "Búsqueda avanzada de conjunto de fichas"
        );
        assert_eq!(
            es.form_title(crate::search::SearchResource::Items),
            "Búsqueda avanzada de fichas"
        );
    }

    /// `sprintf($pattern, $value)` substitutes one argument.
    #[test]
    fn sprintf1_fills_the_first_placeholder() {
        assert_eq!(
            sprintf1("Search results for “%s”", "cat"),
            "Search results for “cat”"
        );
        assert_eq!(
            sprintf1("View all results (%s total)", "4"),
            "View all results (4 total)"
        );
        assert_eq!(sprintf1("No placeholder", "cat"), "No placeholder");
    }

    /// `sprintf` fills each placeholder in turn and leaves a value's own `%s`.
    #[test]
    fn sprintf_fills_placeholders_in_order() {
        assert_eq!(
            sprintf("%s with \"%s: %s\"", &["Items", "Creator", "100%s"]),
            "Items with \"Creator: 100%s\""
        );
        assert_eq!(sprintf("%s and %s", &["one"]), "one and ");
    }
}
