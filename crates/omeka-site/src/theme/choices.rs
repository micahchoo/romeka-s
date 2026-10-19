//! The theme tokens that take one of a fixed set of values: the font stacks,
//! the density, the header and button styles, the content width and the
//! colour scheme. Each is an enum whose stored key is its only spelling, and
//! each knows the CSS it stands for.

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
    /// A curated font stack. Every stack is a list of faces a platform ships,
    /// so the page requests no font file from another origin.
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
    }
}

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
