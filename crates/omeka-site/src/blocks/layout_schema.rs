//! The layout-data schemas: the styling every block's `o:layout_data` carries,
//! a page's own layout data, and a group's section. [`super::schema`] carries
//! them to the admin beside the layouts' own schemas.

use super::schema::{
    ChoiceOption, Group, LayoutSchema, SideKeys, When, anchor, asset_ref, boolean, choice,
    integer_min, option, sides, text, theme_color,
};

/// The spacing scale's choices, as [`super::style::Space`] reads them.
const SPACE_OPTIONS: &[ChoiceOption] = &[
    option("none", "spaceChoice.none"),
    option("small", "spaceChoice.small"),
    option("medium", "spaceChoice.medium"),
    option("large", "spaceChoice.large"),
];

/// `Form/BlockLayoutDataForm.php`: the styling every block's `layout_data`
/// carries, in the sections the styling form shows. `grid_column_position` and
/// `grid_column_span` come from the page editor's grid rather than that form,
/// and are declared here because `AbstractLayout` reads them. The four padding
/// keys are one [`sides`] control.
pub const BLOCK_LAYOUT_DATA: LayoutSchema = LayoutSchema {
    fields: &[
        sides(
            "padding",
            "blockLayoutData.padding",
            "fieldHelp.layout.padding",
            SideKeys {
                top: "padding_top",
                right: "padding_right",
                bottom: "padding_bottom",
                left: "padding_left",
            },
        )
        .group(Group::Spacing),
        choice(
            "romeka.space_top",
            "blockLayoutData.space_top",
            "fieldHelp.layout.space_top",
            SPACE_OPTIONS,
        )
        .group(Group::Spacing),
        choice(
            "romeka.space_bottom",
            "blockLayoutData.space_bottom",
            "fieldHelp.layout.space_bottom",
            SPACE_OPTIONS,
        )
        .group(Group::Spacing),
        boolean(
            "romeka.bleed",
            "blockLayoutData.bleed",
            "fieldHelp.layout.bleed",
        )
        .group(Group::Size),
        text(
            "max_width",
            "blockLayoutData.max_width",
            "fieldHelp.layout.length",
        )
        .placeholder("fieldExample.layout.max_width")
        .group(Group::Size),
        text(
            "min_height",
            "blockLayoutData.min_height",
            "fieldHelp.layout.length",
        )
        .placeholder("fieldExample.layout.min_height")
        .group(Group::Size),
        theme_color(
            "romeka.text_color",
            "romeka.text_color",
            "blockLayoutData.text_color",
            "fieldHelp.layout.text_color",
        )
        .group(Group::Background),
        theme_color(
            "background_color",
            "romeka.background_role",
            "blockLayoutData.background_color",
            "fieldHelp.layout.background_color",
        )
        .group(Group::Background),
        asset_ref(
            "background_image_asset",
            "blockLayoutData.background_image_asset",
            "fieldHelp.layout.background_image_asset",
        )
        .group(Group::Background),
        choice(
            "background_image_size",
            "blockLayoutData.background_image_size",
            "fieldHelp.layout.background_image_size",
            &[
                option("cover", "blockChoice.cover"),
                option("contain", "blockChoice.contain"),
            ],
        )
        .group(Group::Background)
        .when(When::set("background_image_asset")),
        choice(
            "background_image_position_y",
            "blockLayoutData.background_image_position_y",
            "fieldHelp.layout.background_image_position",
            &[
                option("top", "blockChoice.top"),
                option("center", "blockChoice.center"),
                option("bottom", "blockChoice.bottom"),
            ],
        )
        .group(Group::Background)
        .when(When::set("background_image_asset")),
        choice(
            "background_image_position_x",
            "blockLayoutData.background_image_position_x",
            "fieldHelp.layout.background_image_position",
            &[
                option("left", "blockChoice.left"),
                option("center", "blockChoice.center"),
                option("right", "blockChoice.right"),
            ],
        )
        .group(Group::Background)
        .when(When::set("background_image_asset")),
        choice(
            "alignment_block",
            "blockLayoutData.alignment_block",
            "fieldHelp.layout.alignment_block",
            &[
                option("left", "blockChoice.left"),
                option("center", "blockChoice.center"),
                option("right", "blockChoice.right"),
            ],
        )
        .group(Group::Alignment),
        choice(
            "alignment_text",
            "blockLayoutData.alignment_text",
            "fieldHelp.layout.alignment_text",
            &[
                option("left", "blockChoice.left"),
                option("center", "blockChoice.center"),
                option("right", "blockChoice.right"),
                option("justify", "blockChoice.justify"),
            ],
        )
        .group(Group::Alignment),
        text(
            "grid_column_position",
            "blockLayoutData.grid_column_position",
            "fieldHelp.layout.grid_column_position",
        )
        .placeholder("fieldExample.layout.grid_column_position")
        .group(Group::Grid),
        text(
            "grid_column_span",
            "blockLayoutData.grid_column_span",
            "fieldHelp.layout.grid_column_span",
        )
        .group(Group::Grid),
        text(
            "template_name",
            "blockLayoutData.template_name",
            "fieldHelp.layout.template_name",
        )
        .advanced(),
        text("class", "blockLayoutData.class", "fieldHelp.layout.class").advanced(),
        anchor(
            "romeka.anchor",
            "blockLayoutData.anchor",
            "fieldHelp.layout.anchor",
        )
        .advanced(),
    ],
};

/// A `blockGroup`'s section, under `o:layout_data.romeka.settings.section`: how the
/// group lays out its blocks ([`super::style::Section`]). The editor shows it
/// as the group's own panel, with the layout as small diagrams.
pub const SECTION_LAYOUT_DATA: LayoutSchema = LayoutSchema {
    fields: &[
        choice(
            "romeka.section.layout",
            "section.layout",
            "fieldHelp.section.layout",
            &[
                option("stack", "sectionLayout.stack"),
                option("split-50-50", "sectionLayout.split5050"),
                option("split-40-60", "sectionLayout.split4060"),
                option("split-60-40", "sectionLayout.split6040"),
                option("cards-2", "sectionLayout.cards2"),
                option("cards-3", "sectionLayout.cards3"),
                option("cards-4", "sectionLayout.cards4"),
                option("band", "sectionLayout.band"),
            ],
        ),
        choice(
            "romeka.section.align",
            "section.align",
            "fieldHelp.section.align",
            &[
                option("top", "blockChoice.top"),
                option("center", "blockChoice.center"),
                option("bottom", "blockChoice.bottom"),
            ],
        )
        .default_text("top"),
        choice(
            "romeka.section.gap",
            "section.gap",
            "fieldHelp.section.gap",
            SPACE_OPTIONS,
        )
        .default_text("medium"),
        choice(
            "romeka.section.overlay",
            "section.overlay",
            "fieldHelp.section.overlay",
            &[
                option("none", "overlayChoice.none"),
                option("light", "overlayChoice.light"),
                option("dark", "overlayChoice.dark"),
            ],
        )
        .default_text("none"),
    ],
};

/// `Form/PageLayoutDataForm.php`: the page's own `layout_data`. `grid_columns`
/// comes from the page editor's grid rather than that form, and is declared
/// here because `View/Helper/PageLayout::render` reads it.
pub const PAGE_LAYOUT_DATA: LayoutSchema = LayoutSchema {
    fields: &[
        text(
            "template_name",
            "pageLayoutData.template_name",
            "fieldHelp.layout.template_name",
        )
        .advanced(),
        text("class", "pageLayoutData.class", "fieldHelp.layout.class").advanced(),
        integer_min(
            "grid_columns",
            "pageLayoutData.grid_columns",
            "fieldHelp.page.grid_columns",
            0,
        )
        .group(Group::Grid),
        integer_min(
            "grid_column_gap",
            "pageLayoutData.grid_column_gap",
            "fieldHelp.page.grid_gap",
            0,
        )
        .default_int(10)
        .group(Group::Grid),
        integer_min(
            "grid_row_gap",
            "pageLayoutData.grid_row_gap",
            "fieldHelp.page.grid_gap",
            0,
        )
        .default_int(10)
        .group(Group::Grid),
    ],
};

#[cfg(test)]
mod tests {
    use serde_json::{Value as JsonValue, json};

    use super::*;
    use omeka_core::site::BlockLayoutData;

    use crate::blocks::schema::{Field, FieldDefault, FieldKind, romeka_setting};
    use crate::blocks::style::{RomekaStyle, Section};

    /// The values a field can store, one JSON value each.
    fn samples(field: &Field) -> Vec<JsonValue> {
        match field.kind {
            FieldKind::Choice { options, .. } => {
                options.iter().map(|option| json!(option.value)).collect()
            }
            FieldKind::Boolean => vec![json!(true)],
            FieldKind::Anchor => vec![json!("intro")],
            FieldKind::Color { .. } => vec![json!("brand")],
            _ => panic!("{}: a kind the renderer does not read", field.key),
        }
    }

    /// `romeka.a.b` set to `value`, as the `romeka` object holds it: under
    /// `settings`.
    fn romeka_with(path: &str, value: JsonValue) -> JsonValue {
        let mut object = value;
        for part in path
            .split('.')
            .skip(1)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            object = json!({ part: object });
        }
        json!({ "block": "blockGroup", "v": 2, "settings": object })
    }

    /// Every `romeka.` key the styling form or the section panel writes is a
    /// member the renderer reads, and every value the form offers changes
    /// what it renders. A section field other than the layout is read beside a
    /// layout, because a group with no layout is no section.
    #[test]
    fn every_romeka_key_the_forms_write_is_rendered() {
        let keys = BLOCK_LAYOUT_DATA
            .fields
            .iter()
            .chain(SECTION_LAYOUT_DATA.fields)
            .flat_map(|field| {
                let token = match field.kind {
                    FieldKind::Color {
                        token_key: Some(key),
                    } => Some((key, field)),
                    _ => None,
                };
                std::iter::once((field.key, field)).chain(token)
            })
            .filter(|(key, _)| romeka_setting(key).is_some());
        let base =
            json!({"block": "blockGroup", "v": 2, "settings": {"section": {"layout": "stack"}}});
        let rendered = |romeka: &JsonValue| {
            let data = BlockLayoutData::parse(&json!({ "romeka": romeka }));
            let style = RomekaStyle::parse(&data);
            let section = Section::parse(&data).map(|section| section.classes());
            (
                style.bleed,
                style.background,
                style.styles(),
                style.anchor,
                section,
            )
        };
        let mut seen = 0;
        for (key, field) in keys {
            for value in samples(field) {
                if let Some(FieldDefault::Text(default)) = field.default
                    && value.as_str() == Some(default)
                {
                    continue;
                }
                let mut romeka = romeka_with(key, value.clone());
                if let Some(section) = romeka.pointer_mut("/settings/section")
                    && key != "romeka.section.layout"
                {
                    section["layout"] = json!("stack");
                }
                let before = if key.starts_with("romeka.section.") && key != "romeka.section.layout"
                {
                    rendered(&base)
                } else {
                    rendered(&json!({}))
                };
                assert_ne!(rendered(&romeka), before, "{key} = {value} renders nothing");
                seen += 1;
            }
        }
        assert!(seen > 20, "only {seen} values checked");
    }
}
