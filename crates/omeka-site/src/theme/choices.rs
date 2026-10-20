//! The theme tokens that take one of a fixed set of values: the font stacks,
//! the density, the header and button styles, the content width, the colour
//! scheme, and the structural tokens (hero, cards, search, footer, item page).
//! Each is an enum whose stored key is its only spelling, and each knows the
//! CSS it stands for: a custom property, or a class on the root element that
//! the stylesheet keys its structure on.

/// Declare a choice token: an enum, its stored keys in schema order, and the
/// parser from a stored key.
macro_rules! choice {
    (
        $(#[$meta:meta])*
        $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident = $key:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($(#[$variant_meta])* $variant),+
        }

        impl $name {
            /// Every value, in schema order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            /// Every stored key, in schema order.
            pub const KEYS: &'static [&'static str] = &[$($key),+];

            /// The stored key.
            #[must_use]
            pub const fn key(self) -> &'static str {
                match self {
                    $(Self::$variant => $key),+
                }
            }

            /// The value a stored key names, `None` for any other string.
            #[must_use]
            pub fn parse(key: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|value| value.key() == key)
            }
        }
    };
}

pub(crate) use choice;

choice! {
    /// A curated font stack. A system stack names faces a platform ships; a
    /// webfont stack names a face the theme serves from its own assets, then
    /// the Noto faces for the Indic scripts, so the page requests no font file
    /// from another origin.
    FontStack {
        /// The platform's sans-serif face.
        SystemSans = "system-sans",
        /// The platform's serif face.
        SystemSerif = "system-serif",
        /// A humanist sans stack.
        Humanist = "humanist",
        /// A geometric sans stack.
        Geometric = "geometric",
        /// A transitional serif stack.
        TransitionalSerif = "transitional-serif",
        /// A high-contrast (Didone) serif stack for display headings.
        Didone = "didone",
        /// A monospace stack.
        Mono = "mono",
        /// Fraunces, a display serif with an optical size axis.
        Fraunces = "fraunces",
        /// Source Sans 3, a humanist text sans.
        SourceSans3 = "source-sans-3",
        /// IBM Plex Sans, a grotesque for dense catalogues.
        IbmPlexSans = "ibm-plex-sans",
        /// Inter, a neo-grotesque with an optical size axis.
        Inter = "inter",
        /// IBM Plex Mono, for identifiers and code.
        IbmPlexMono = "ibm-plex-mono",
    }
}

/// The Noto faces a serif webfont stack falls back to for the Indic scripts.
macro_rules! noto_serif {
    () => {
        "\"Noto Serif Devanagari\", \"Noto Serif Tamil\", \"Noto Serif Telugu\", \"Noto Serif Kannada\", \"Noto Serif Malayalam\""
    };
}

/// The Noto faces a sans webfont stack falls back to for the Indic scripts.
macro_rules! noto_sans {
    () => {
        "\"Noto Sans Devanagari\", \"Noto Sans Tamil\", \"Noto Sans Telugu\", \"Noto Sans Kannada\", \"Noto Sans Malayalam\""
    };
}

const NOTO_SERIF: &[&str] = &[
    "Noto Serif Devanagari",
    "Noto Serif Tamil",
    "Noto Serif Telugu",
    "Noto Serif Kannada",
    "Noto Serif Malayalam",
];

const NOTO_SANS: &[&str] = &[
    "Noto Sans Devanagari",
    "Noto Sans Tamil",
    "Noto Sans Telugu",
    "Noto Sans Kannada",
    "Noto Sans Malayalam",
];

impl FontStack {
    /// The CSS `font-family` value.
    #[must_use]
    pub const fn stack(self) -> &'static str {
        match self {
            Self::SystemSans => "system-ui, -apple-system, \"Segoe UI\", Roboto, sans-serif",
            Self::SystemSerif => "Georgia, \"Times New Roman\", Times, serif",
            Self::Humanist => "\"Gill Sans\", \"Trebuchet MS\", Calibri, sans-serif",
            Self::Geometric => "Futura, \"Century Gothic\", \"Avenir Next\", sans-serif",
            Self::TransitionalSerif => "Charter, \"Bitstream Charter\", Cambria, serif",
            Self::Didone => "Didot, \"Bodoni 72\", \"Bodoni MT\", \"Libre Bodoni\", Georgia, serif",
            Self::Mono => "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
            Self::Fraunces => concat!("\"Fraunces\", ", noto_serif!(), ", Georgia, serif"),
            Self::SourceSans3 => {
                concat!(
                    "\"Source Sans 3\", ",
                    noto_sans!(),
                    ", system-ui, sans-serif"
                )
            }
            Self::IbmPlexSans => {
                concat!(
                    "\"IBM Plex Sans\", ",
                    noto_sans!(),
                    ", system-ui, sans-serif"
                )
            }
            Self::Inter => concat!("\"Inter\", ", noto_sans!(), ", system-ui, sans-serif"),
            Self::IbmPlexMono => "\"IBM Plex Mono\", ui-monospace, Menlo, Consolas, monospace",
        }
    }

    /// The faces the theme serves for this stack, its own first: every family
    /// [`crate::theme::fonts`] must declare for the stack to render as drawn.
    /// A system stack serves none.
    #[must_use]
    pub const fn served(self) -> (Option<&'static str>, &'static [&'static str]) {
        match self {
            Self::SystemSans
            | Self::SystemSerif
            | Self::Humanist
            | Self::Geometric
            | Self::TransitionalSerif
            | Self::Didone
            | Self::Mono => (None, &[]),
            Self::Fraunces => (Some("Fraunces"), NOTO_SERIF),
            Self::SourceSans3 => (Some("Source Sans 3"), NOTO_SANS),
            Self::IbmPlexSans => (Some("IBM Plex Sans"), NOTO_SANS),
            Self::Inter => (Some("Inter"), NOTO_SANS),
            Self::IbmPlexMono => (Some("IBM Plex Mono"), &[]),
        }
    }
}

choice! {
    /// How much air the layout leaves around its content.
    Density {
        /// Tight spacing for a dense catalogue.
        Compact = "compact",
        /// The default spacing.
        Comfortable = "comfortable",
        /// Generous spacing for a reading site.
        Spacious = "spacious",
    }
}

impl Density {
    /// The five spacing steps (`--space-1` through `--space-5`).
    #[must_use]
    pub const fn spaces(self) -> [&'static str; 5] {
        match self {
            Self::Compact => ["0.125rem", "0.375rem", "0.75rem", "1rem", "2rem"],
            Self::Comfortable => ["0.25rem", "0.5rem", "1rem", "1.5rem", "3rem"],
            Self::Spacious => ["0.375rem", "0.75rem", "1.25rem", "2rem", "4rem"],
        }
    }

    /// The body line height.
    #[must_use]
    pub const fn line_height(self) -> &'static str {
        match self {
            Self::Compact => "1.4",
            Self::Comfortable => "1.5",
            Self::Spacious => "1.65",
        }
    }
}

choice! {
    /// Where the site's logo and title sit in the header.
    HeaderLayout {
        /// The logo and title align left, with the navigation on the same row.
        LogoLeft = "logo-left",
        /// The logo, title and navigation centre.
        Centred = "centred",
    }
}

impl HeaderLayout {
    /// The class the frame puts on the header.
    #[must_use]
    pub const fn class(self) -> &'static str {
        match self {
            Self::LogoLeft => "header-logo-left",
            Self::Centred => "header-centred",
        }
    }
}

choice! {
    /// How the header sets itself off from the page.
    HeaderStyle {
        /// The page background with a thin rule under it.
        Plain = "plain",
        /// A band of the brand colour.
        Band = "band",
        /// A double rule in the text colour, like a printed masthead.
        Rule = "rule",
        /// No rule and no band; the content starts at once.
        Minimal = "minimal",
    }
}

impl HeaderStyle {
    /// The header's `(background, text, link, rule under it, stripe above it,
    /// current-page mark)`. The header only ever draws text or links on the
    /// page background, or text on the brand colour, so the contrast pairs
    /// cover it. The stripe is a fill, never behind text.
    #[must_use]
    pub const fn properties(self) -> [(&'static str, &'static str); 6] {
        match self {
            Self::Plain => [
                ("header-background", "transparent"),
                ("header-text", "var(--color-text)"),
                ("header-link", "var(--color-link)"),
                ("header-border", "1px solid var(--color-border)"),
                ("header-stripe", "4px solid var(--color-brand)"),
                ("header-mark", "var(--color-brand)"),
            ],
            Self::Band => [
                ("header-background", "var(--color-brand)"),
                ("header-text", "var(--color-on-brand)"),
                ("header-link", "var(--color-on-brand)"),
                ("header-border", "0 none"),
                ("header-stripe", "0 none"),
                ("header-mark", "var(--color-on-brand)"),
            ],
            Self::Rule => [
                ("header-background", "transparent"),
                ("header-text", "var(--color-text)"),
                ("header-link", "var(--color-text)"),
                ("header-border", "3px double var(--color-text)"),
                ("header-stripe", "0 none"),
                ("header-mark", "var(--color-brand)"),
            ],
            Self::Minimal => [
                ("header-background", "transparent"),
                ("header-text", "var(--color-text)"),
                ("header-link", "var(--color-text)"),
                ("header-border", "0 none"),
                ("header-stripe", "0 none"),
                ("header-mark", "var(--color-brand)"),
            ],
        }
    }
}

choice! {
    /// How a button looks.
    ButtonStyle {
        /// The surface colour with a thin border.
        Soft = "soft",
        /// Filled with the brand colour.
        Solid = "solid",
        /// Transparent, outlined in the link colour.
        Outline = "outline",
    }
}

impl ButtonStyle {
    /// The button's `(background, text, border colour)`, which combine only
    /// pairs the contrast gate checks.
    #[must_use]
    pub const fn properties(self) -> [(&'static str, &'static str); 3] {
        match self {
            Self::Soft => [
                ("button-background", "var(--color-surface)"),
                ("button-text", "var(--color-text)"),
                ("button-border", "var(--color-border)"),
            ],
            Self::Solid => [
                ("button-background", "var(--color-brand)"),
                ("button-text", "var(--color-on-brand)"),
                ("button-border", "var(--color-brand)"),
            ],
            Self::Outline => [
                ("button-background", "transparent"),
                ("button-text", "var(--color-link)"),
                ("button-border", "var(--color-link)"),
            ],
        }
    }
}

choice! {
    /// The widest the header and the page content grow.
    ContentWidth {
        /// A reading measure.
        Narrow = "narrow",
        /// The default width.
        Standard = "standard",
        /// Room for wide image grids.
        Wide = "wide",
        /// Nearly the whole screen, for an image wall.
        Full = "full",
    }
}

impl ContentWidth {
    /// The `--content-width` value.
    #[must_use]
    pub const fn css(self) -> &'static str {
        match self {
            Self::Narrow => "44rem",
            Self::Standard => "60rem",
            Self::Wide => "76rem",
            Self::Full => "90rem",
        }
    }
}

choice! {
    /// Which palette the page shows.
    ColorScheme {
        /// The light palette.
        Light = "light",
        /// The dark palette.
        Dark = "dark",
        /// The light palette, and the dark one when the reader's system asks
        /// for dark.
        Auto = "auto",
    }
}

impl ColorScheme {
    /// The CSS `color-scheme` value.
    #[must_use]
    pub const fn css(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Auto => "light dark",
        }
    }
}

/// A structural token: one of a fixed set of layouts, drawn by the stylesheet
/// under a class the frame puts on the root element.
macro_rules! structure {
    (
        $(#[$meta:meta])*
        $name:ident, $prefix:literal {
            $($(#[$variant_meta:meta])* $variant:ident = $key:literal),+ $(,)?
        }
    ) => {
        choice! {
            $(#[$meta])*
            $name {
                $($(#[$variant_meta])* $variant = $key),+
            }
        }

        impl $name {
            /// The class the frame puts on the root element.
            #[must_use]
            pub const fn class(self) -> &'static str {
                match self {
                    $(Self::$variant => concat!($prefix, $key)),+
                }
            }
        }
    };
}

structure! {
    /// How a hero block with a photograph lays the photograph out. A hero with
    /// no photograph is type only in every style.
    HeroStyle, "hero-" {
        /// The photograph fills the screen's width, the text on a scrim at its foot.
        Cover = "cover",
        /// The text in a panel beside the photograph.
        Split = "split",
        /// The whole photograph, never cropped, on a band, the text below.
        Framed = "framed",
    }
}

structure! {
    /// How a resource list draws each resource.
    CardStyle, "cards-" {
        /// A 4:3 thumbnail with the name and details below it.
        Caption = "caption",
        /// Square tiles edge to edge, the name over the foot of the image.
        Overlay = "overlay",
        /// One row per resource: a small thumbnail, the name and its details.
        Row = "row",
    }
}

structure! {
    /// How the header shows the search form on a wide screen. A phone always
    /// keeps it behind the icon.
    SearchDisplay, "search-" {
        /// The form is always open in the header.
        Open = "open",
        /// The form opens under a search icon.
        Icon = "icon",
    }
}

structure! {
    /// How the footer arranges the site name, the columns, the links and the
    /// credit.
    FooterLayout, "footer-" {
        /// The site and its columns beside the links, the credit below.
        Columns = "columns",
        /// A large site name on the left, the links stacked on the right.
        Split = "split",
        /// One wrapping line.
        Inline = "inline",
    }
}

choice! {
    /// The footer's ground.
    FooterTone {
        /// The page background.
        Page = "page",
        /// The surface colour.
        Surface = "surface",
        /// Text-coloured ink with page-coloured text.
        Inverse = "inverse",
    }
}

impl FooterTone {
    /// The footer's `(background, text, muted text, link)`. The inverse
    /// ground is the text colour with the page colour on it, the text and
    /// background pair the palette gate holds to AA; its muted text is the
    /// page colour softened toward the ink.
    #[must_use]
    pub const fn properties(self) -> [(&'static str, &'static str); 4] {
        match self {
            Self::Page => [
                ("footer-background", "var(--color-background)"),
                ("footer-text", "var(--color-text)"),
                ("footer-muted", "var(--color-muted)"),
                ("footer-link", "var(--color-link)"),
            ],
            Self::Surface => [
                ("footer-background", "var(--color-surface)"),
                ("footer-text", "var(--color-text)"),
                ("footer-muted", "var(--color-muted)"),
                ("footer-link", "var(--color-link)"),
            ],
            Self::Inverse => [
                ("footer-background", "var(--color-text)"),
                ("footer-text", "var(--color-background)"),
                (
                    "footer-muted",
                    "color-mix(in srgb, var(--color-background) 80%, var(--color-text))",
                ),
                ("footer-link", "var(--color-background)"),
            ],
        }
    }
}

structure! {
    /// Where an item page puts its media.
    ItemLayout, "item-" {
        /// The media first and wide, the record below.
        Stacked = "stacked",
        /// The media in a column beside the record on a wide screen.
        Side = "side",
    }
}
