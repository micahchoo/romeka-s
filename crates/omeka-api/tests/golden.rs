//! Golden tests: each fixture parses into its wire type, serializes back, and
//! must byte-equal the file.
//!
//! The fixtures are the frozen shape (`verification/fixtures/api/`). If a
//! field is added, renamed, or reordered, one of these tests fails with the
//! two documents side by side.
//!
//! Derivation of every fixture from the PHP representations is in
//! `verification/fixtures/api/README.md`; the files themselves carry no
//! annotations because they must round-trip byte-equal.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use omeka_api::jsonld;
use omeka_api::wire;

/// Read a fixture and assert it survives parse -> pretty-print unchanged.
fn assert_round_trip<T>(fixture: &str)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let path = format!(
        "{}/../../verification/fixtures/api/{fixture}",
        env!("CARGO_MANIFEST_DIR")
    );
    let frozen = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("reading {path}: {err}");
    });
    let value: T = serde_json::from_str(&frozen)
        .unwrap_or_else(|err| panic!("{fixture} does not parse as its wire type: {err}"));
    let emitted = jsonld::to_string_pretty(&value).unwrap();
    assert_eq!(emitted, frozen.trim_end(), "{fixture} did not round-trip");
}

#[test]
fn item_fixture_round_trips() {
    assert_round_trip::<wire::item::Item>("item.json");
}

#[test]
fn item_set_fixture_round_trips() {
    assert_round_trip::<wire::item_set::ItemSet>("item_set.json");
}

#[test]
fn media_fixture_round_trips() {
    assert_round_trip::<wire::media::Media>("media.json");
}

#[test]
fn vocabulary_fixture_round_trips() {
    assert_round_trip::<wire::vocabulary::Vocabulary>("vocabulary.json");
}

#[test]
fn resource_template_fixture_round_trips() {
    assert_round_trip::<wire::resource_template::ResourceTemplate>("resource_template.json");
}

#[test]
fn site_fixture_round_trips() {
    assert_round_trip::<wire::site::Site>("site.json");
}

#[test]
fn user_fixture_round_trips() {
    assert_round_trip::<wire::user::User>("user.json");
}

#[test]
fn asset_fixture_round_trips() {
    assert_round_trip::<wire::asset::Asset>("asset.json");
}

#[test]
fn property_fixture_round_trips() {
    assert_round_trip::<wire::property::Property>("property.json");
}

#[test]
fn resource_class_fixture_round_trips() {
    assert_round_trip::<wire::resource_class::ResourceClass>("resource_class.json");
}

#[test]
fn job_fixture_round_trips() {
    assert_round_trip::<wire::job::Job>("job.json");
}

#[test]
fn value_annotation_fixture_round_trips() {
    assert_round_trip::<wire::value_annotation::ValueAnnotation>("value_annotation.json");
}

#[test]
fn module_fixture_round_trips() {
    assert_round_trip::<wire::module::Module>("module.json");
}

#[test]
fn api_resource_fixture_round_trips() {
    assert_round_trip::<wire::api_resource::ApiResource>("api_resource.json");
}

#[test]
fn data_type_fixture_round_trips() {
    assert_round_trip::<wire::data_type::DataType>("data_type.json");
}
