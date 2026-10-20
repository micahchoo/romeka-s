//! The column count a curator may pick for a block that lays its entries out
//! in a grid: two to six, or as many as the width holds.

use omeka_core::site::SiteBlock;
use serde_json::Value as JsonValue;

use crate::blocks::schema::{ChoiceOption, option};

/// The choices, as a select offers them; the empty choice fits as many
/// entries as the width holds.
pub(crate) const OPTIONS: &[ChoiceOption] = &[
    option("", "blockChoice.columns_auto"),
    option("2", "blockChoice.columns_2"),
    option("3", "blockChoice.columns_3"),
    option("4", "blockChoice.columns_4"),
    option("5", "blockChoice.columns_5"),
    option("6", "blockChoice.columns_6"),
];

/// The count stored in the block's `romeka.settings.columns`, `None` when it
/// is unset or not one of the choices.
pub(crate) fn of(block: &SiteBlock) -> Option<u8> {
    block
        .layout_data
        .romeka_setting("columns")
        .and_then(JsonValue::as_str)
        .and_then(|text| text.parse::<u8>().ok())
        .filter(|count| (2..=6).contains(count))
}

/// The class `<prefix>-columns-<count>` the sheet keys a count on.
pub(crate) fn class(prefix: &str, count: u8) -> String {
    format!("{prefix}-columns-{count}")
}
