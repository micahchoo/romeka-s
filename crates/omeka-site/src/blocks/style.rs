//! The Romeka-only styling every block keeps in its Romeka settings, and the
//! section layout a `blockGroup` keeps there under `section`.
//!
//! Romeka settings on any block live in one shape, `o:layout_data.romeka =
//! {block, v, settings}`: a Romeka block's carrier names that block, and an
//! upstream block names its own layout (`"blockGroup"`, `"html"`). So the
//! styling sits in `settings` beside a hero's own settings, and one reader
//! serves every block.
//!
//! Upstream's admin keeps the `romeka` member and its renderer ignores it, so a
//! page styled here renders upstream as the same blocks in upstream's wrappers
//! (`docs/compat.md`, "Site pages: the one-way rule"). The members are read
//! here, at the render boundary, through [`BlockLayoutData::romeka_setting`]:
//! a value of the wrong kind or outside its list reads as absent and renders
//! nothing.
//!
//! Every value is one of a short list, so each list is an enum whose `css` or
//! `class` names what the wrapper carries. The stylesheet styles the classes
//! (`theme/style.css`, "Sections"), and the inline styles read only the theme's
//! custom properties or a validated hex colour.

use omeka_core::site::BlockLayoutData;
use serde_json::Value as JsonValue;

use crate::theme::palette::Role;

/// The longest anchor id the editor accepts.
pub const ANCHOR_MAX: usize = 64;

/// A step of the spacing scale, for a block's outer margin or a section's gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space {
    /// No space.
    None,
    /// The theme's second spacing step.
    Small,
    /// The theme's fourth step, which is also a block's default margin.
    Medium,
    /// The theme's fifth step.
    Large,
}

impl Space {
    /// Every step with its stored name.
    pub const ALL: [(&'static str, Self); 4] = [
        ("none", Self::None),
        ("small", Self::Small),
        ("medium", Self::Medium),
        ("large", Self::Large),
    ];

    /// The step a stored name names.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(name, _)| *name == value)
            .map(|(_, space)| *space)
    }

    /// The stored name.
    #[must_use]
    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, space)| *space == self)
            .map_or("medium", |(name, _)| name)
    }

    /// The CSS length: a theme spacing step, or zero.
    #[must_use]
    pub const fn css(self) -> &'static str {
        match self {
            Self::None => "0",
            Self::Small => "var(--space-2)",
            Self::Medium => "var(--space-4)",
            Self::Large => "var(--space-5)",
        }
    }
}

/// A colour a curator picks: one of the theme's colour roles, which follows
/// the Theme page, or a fixed `<hex-color>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Colour {
    /// A theme role, rendered as `var(--color-<role>)`.
    Token(Role),
    /// `#rgb` or `#rrggbb`.
    Hex(String),
}

impl Colour {
    /// A stored role name or hex colour.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        if is_hex_color(value) {
            return Some(Self::Hex(value.to_string()));
        }
        token(value).map(Self::Token)
    }

    /// The CSS value.
    #[must_use]
    pub fn css(&self) -> String {
        match self {
            Self::Token(role) => format!("var(--color-{})", role.key()),
            Self::Hex(hex) => hex.clone(),
        }
    }
}

/// The theme role a stored token names.
#[must_use]
pub fn token(value: &str) -> Option<Role> {
    Role::ALL.iter().copied().find(|role| role.key() == value)
}

/// `$isValidHexColor`: `#` and three or six hex digits.
#[must_use]
pub fn is_hex_color(value: &str) -> bool {
    let Some(digits) = value.strip_prefix('#') else {
        return false;
    };
    (digits.len() == 3 || digits.len() == 6) && digits.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Whether `value` is an anchor id the wrapper may carry: 1 to [`ANCHOR_MAX`]
/// lowercase letters, digits and hyphens, starting with a letter or a digit.
/// The editor turns what a curator types into this shape.
#[must_use]
pub fn is_anchor(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= ANCHOR_MAX
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The styling every block may keep under `o:layout_data.romeka`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RomekaStyle {
    /// `anchor`, the wrapper's `id`, so a link can jump to the block.
    pub anchor: Option<String>,
    /// `space_top`, the margin above the block.
    pub space_top: Option<Space>,
    /// `space_bottom`, the margin below the block.
    pub space_bottom: Option<Space>,
    /// `bleed`, whether the block spans the whole window width.
    pub bleed: bool,
    /// `text_color`, the colour of the block's text.
    pub text_color: Option<Colour>,
    /// `background_role`, a theme role behind the block. A hex background
    /// stays in upstream's own `background_color`, which upstream renders too.
    pub background: Option<Role>,
}

impl RomekaStyle {
    /// Read the styling out of the block's Romeka settings.
    #[must_use]
    pub fn parse(data: &BlockLayoutData) -> Self {
        let text = |key: &str| data.romeka_setting(key).and_then(JsonValue::as_str);
        Self {
            anchor: text("anchor")
                .filter(|id| is_anchor(id))
                .map(str::to_string),
            space_top: text("space_top").and_then(Space::parse),
            space_bottom: text("space_bottom").and_then(Space::parse),
            bleed: data.romeka_setting("bleed").and_then(JsonValue::as_bool) == Some(true),
            text_color: text("text_color").and_then(Colour::parse),
            background: text("background_role").and_then(token),
        }
    }

    /// The inline styles the styling adds after upstream's own.
    #[must_use]
    pub fn styles(&self) -> Vec<String> {
        let mut styles = Vec::new();
        if let Some(space) = self.space_top {
            styles.push(format!("margin-top: {}", space.css()));
        }
        if let Some(space) = self.space_bottom {
            styles.push(format!("margin-bottom: {}", space.css()));
        }
        if let Some(colour) = &self.text_color {
            styles.push(format!("color: {}", colour.css()));
        }
        if let Some(role) = self.background {
            styles.push(format!("background-color: var(--color-{})", role.key()));
        }
        styles
    }
}

/// How a section lays out its blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionLayout {
    /// One column, the blocks one under another.
    Stack,
    /// Two equal columns.
    Split5050,
    /// A narrow column, then a wide one.
    Split4060,
    /// A wide column, then a narrow one.
    Split6040,
    /// Two cards a row.
    Cards2,
    /// Three cards a row.
    Cards3,
    /// Four cards a row.
    Cards4,
    /// One column across the whole window width.
    Band,
}

impl SectionLayout {
    /// Every layout with its stored name, in the order the editor offers them.
    pub const ALL: [(&'static str, Self); 8] = [
        ("stack", Self::Stack),
        ("split-50-50", Self::Split5050),
        ("split-40-60", Self::Split4060),
        ("split-60-40", Self::Split6040),
        ("cards-2", Self::Cards2),
        ("cards-3", Self::Cards3),
        ("cards-4", Self::Cards4),
        ("band", Self::Band),
    ];

    /// The layout a stored name names.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(name, _)| *name == value)
            .map(|(_, layout)| *layout)
    }

    /// The stored name, which is also the class suffix.
    #[must_use]
    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, layout)| *layout == self)
            .map_or("stack", |(name, _)| name)
    }
}

/// Where a section's blocks sit in a row that a taller block sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// At the top of the row.
    Top,
    /// In the middle.
    Center,
    /// At the bottom.
    Bottom,
}

impl Align {
    /// Every alignment with its stored name.
    pub const ALL: [(&'static str, Self); 3] = [
        ("top", Self::Top),
        ("center", Self::Center),
        ("bottom", Self::Bottom),
    ];

    fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(name, _)| *name == value)
            .map(|(_, align)| *align)
    }

    fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, align)| *align == self)
            .map_or("top", |(name, _)| name)
    }
}

/// The scrim between a section's background and its blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// No scrim.
    None,
    /// A wash of the page background, under the theme's text colour.
    Light,
    /// A dark wash, under white text.
    Dark,
}

impl Overlay {
    /// Every overlay with its stored name.
    pub const ALL: [(&'static str, Self); 3] = [
        ("none", Self::None),
        ("light", Self::Light),
        ("dark", Self::Dark),
    ];

    fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(name, _)| *name == value)
            .map(|(_, overlay)| *overlay)
    }
}

/// A group's section: its layout and how its blocks sit in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    /// `layout`.
    pub layout: SectionLayout,
    /// `align`, absent for the top.
    pub align: Option<Align>,
    /// `gap`, absent for the medium step.
    pub gap: Option<Space>,
    /// `overlay`, absent for none.
    pub overlay: Option<Overlay>,
}

impl Section {
    /// Read `romeka.settings.section`. A group with no layout there is no
    /// section: it renders the way upstream does, its blocks following the page.
    #[must_use]
    pub fn parse(data: &BlockLayoutData) -> Option<Self> {
        let section = data.romeka_setting("section")?;
        let text = |key: &str| section.get(key).and_then(JsonValue::as_str);
        Some(Self {
            layout: text("layout").and_then(SectionLayout::parse)?,
            align: text("align").and_then(Align::parse),
            gap: text("gap").and_then(Space::parse),
            overlay: text("overlay").and_then(Overlay::parse),
        })
    }

    /// The classes the section adds to the group's wrapper.
    #[must_use]
    pub fn classes(&self) -> Vec<String> {
        let mut classes = vec![
            "romeka-section".to_string(),
            format!("romeka-section-{}", self.layout.name()),
        ];
        if let Some(align) = self.align {
            classes.push(format!("romeka-section-align-{}", align.name()));
        }
        if let Some(gap) = self.gap {
            classes.push(format!("romeka-section-gap-{}", gap.name()));
        }
        match self.overlay {
            Some(Overlay::Light) => classes.push("romeka-section-overlay-light".to_string()),
            Some(Overlay::Dark) => classes.push("romeka-section-overlay-dark".to_string()),
            Some(Overlay::None) | None => {}
        }
        classes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The layout data of a block whose `o:layout_data.romeka` is `romeka`.
    fn stored(romeka: JsonValue) -> BlockLayoutData {
        BlockLayoutData::parse(&json!({ "romeka": romeka }))
    }

    /// Each stored value reads as its own variant, and anything else reads as
    /// absent rather than as a default the curator never chose.
    #[test]
    fn stored_values_parse_or_read_as_absent() {
        let style = RomekaStyle::parse(&stored(json!({"block": "html", "v": 2, "settings": {
            "anchor": "our-story",
            "space_top": "large",
            "space_bottom": "none",
            "bleed": true,
            "text_color": "brand",
            "background_role": "surface",
        }})));
        assert_eq!(style.anchor.as_deref(), Some("our-story"));
        assert_eq!(style.space_top, Some(Space::Large));
        assert_eq!(style.space_bottom, Some(Space::None));
        assert!(style.bleed);
        assert_eq!(style.text_color, Some(Colour::Token(Role::Brand)));
        assert_eq!(style.background, Some(Role::Surface));

        let odd = RomekaStyle::parse(&stored(json!({"block": "html", "v": 2, "settings": {
            "anchor": "Our Story",
            "space_top": "huge",
            "bleed": "yes",
            "text_color": "red",
            "background_role": "#ffffff",
        }})));
        assert_eq!(odd, RomekaStyle::default());
        // Styling outside `settings` is not where any writer puts it.
        let odd = RomekaStyle::parse(&stored(json!({"block": "html", "v": 2, "anchor": "intro"})));
        assert_eq!(odd, RomekaStyle::default());
        assert_eq!(
            RomekaStyle::parse(&BlockLayoutData::default()),
            RomekaStyle::default()
        );
    }

    #[test]
    fn an_anchor_is_a_lowercase_slug() {
        for valid in ["intro", "2026", "our-story", "a-"] {
            assert!(is_anchor(valid), "{valid}");
        }
        let long = "a".repeat(ANCHOR_MAX + 1);
        for invalid in [
            "",
            "-intro",
            "Intro",
            "our story",
            "naïve",
            "a_b",
            long.as_str(),
        ] {
            assert!(!is_anchor(invalid), "{invalid}");
        }
    }

    /// A text colour may be a hex colour or a theme role; a background token
    /// is a role only, because a hex background is upstream's own member.
    #[test]
    fn colours_render_as_theme_properties_or_hex() {
        let style = RomekaStyle::parse(&stored(json!({"block": "html", "v": 2, "settings": {
            "text_color": "#336699",
            "background_role": "brand",
        }})));
        assert_eq!(
            style.styles(),
            ["color: #336699", "background-color: var(--color-brand)"]
        );
    }

    #[test]
    fn a_group_without_a_stored_layout_is_no_section() {
        assert_eq!(
            Section::parse(&stored(json!({"block": "blockGroup", "v": 2}))),
            None
        );
        assert_eq!(
            Section::parse(&stored(
                json!({"block": "blockGroup", "v": 2, "settings": {"section": {"layout": "grid"}}})
            )),
            None
        );
        assert_eq!(
            Section::parse(&stored(json!({"block": "blockGroup", "v": 2, "settings": {
                "section": {"layout": "cards-3", "gap": "wide"},
            }}))),
            Some(Section {
                layout: SectionLayout::Cards3,
                align: None,
                gap: None,
                overlay: None,
            })
        );
    }
}
