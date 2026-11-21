// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as archive, DcbOrigin, TestResult, at, guid, text_row};
use crate::health_origin_assertions as proof;

use self::fixture::{AMENITY, FUTURE_ICON, PRESET, RADAR_CONTACT, REQUIRED_ENTITY, TYPE};

#[path = "starmap_definitions_fixture.rs"]
mod fixture;

#[test]
fn starmap_definitions_preserve_physical_type_and_amenity_facts() -> TestResult {
    let (_folder, input) = archive::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let catalog = document(&input, None)?;
    let kind = at(&catalog, "/definitions/starmapTypes/rows/0")?;
    proof::present(kind, "guid", &json!(guid(TYPE)), Some(true))?;
    proof::present(kind, "spawnNavPoints", &json!(true), Some(true))?;
    proof::present(kind, "showInMapSelectList", &json!(false), Some(true))?;
    proof::present(
        kind,
        "validQuantumTravelDestination",
        &json!(false),
        Some(true),
    )?;
    assert_eq!(
        at(kind, "/markerConfig/value/status")?,
        "dangling",
        "original missing marker"
    );
    let amenity = at(&catalog, "/definitions/starmapAmenities/rows/0")?;
    proof::present(amenity, "guid", &json!(guid(AMENITY)), Some(true))?;
    proof::present(amenity, "icon", &json!("fixtures/amenity.dds"), Some(true))?;
    let label = text_row(&catalog, "publicNames", at(amenity, "/displayName/value")?)?;
    assert_eq!(
        at(label, "/key/value")?,
        "@shared",
        "the authored short label"
    );
    assert_eq!(
        at(amenity, "/requiredEntity/value/guid/value")?,
        &json!(guid(REQUIRED_ENTITY)),
        "exact required location entity"
    );
    proof::closure(&catalog)
}

#[test]
fn starmap_definitions_keep_preset_navigation_radar_and_owned_original_numbers() -> TestResult {
    let (_folder, input) = archive::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let catalog = document(&input, None)?;
    let preset = at(&catalog, "/definitions/starmapPresets/rows/0")?;
    proof::present(preset, "guid", &json!(guid(PRESET)), Some(true))?;
    verify_preset_display(&catalog, preset)?;
    verify_preset_navigation(preset)?;
    verify_preset_radar(preset)?;
    proof::cell(
        at(&catalog, "/definitions/starmapPresets/rows/1")?,
        "quantumTravelData",
        "null",
    )?;
    proof::closure(&catalog)
}

fn verify_preset_display(catalog: &Value, preset: &Value) -> TestResult {
    let display = at(preset, "/displaySettings/value")?;
    proof::present(display, "navIcon", &json!(FUTURE_ICON), Some(true))?;
    proof::present(
        display,
        "size",
        &json!({"rawBits":"7ff8000000001067","finite":null}),
        Some(true),
    )?;
    proof::present(
        display,
        "minimumDisplaySize",
        &json!({"rawBits":"80000000","finite":-0.0}),
        Some(true),
    )?;
    let lore = text_row(catalog, "gameTexts", at(display, "/description/value")?)?;
    assert_eq!(
        at(lore, "/key/value")?,
        "@shared",
        "protected original preset description"
    );
    assert_eq!(
        at(
            display,
            "/assetManagerLocationParams/value/previewImagePath/value"
        )?,
        "fixtures/preview.dds",
        "original asset path"
    );
    Ok(())
}

fn verify_preset_navigation(preset: &Value) -> TestResult {
    let visibility = at(preset, "/visibilitySettings/value")?;
    proof::present(visibility, "blockTravel", &json!(true), Some(true))?;
    proof::present(visibility, "hideInWorld", &json!(true), Some(true))?;
    let qt = at(preset, "/quantumTravelData/value")?;
    proof::present(
        qt,
        "arrivalRadius",
        &json!({"rawBits":"40400000","finite":3.0}),
        Some(true),
    )?;
    proof::present(
        qt,
        "obstructionRadius",
        &json!({"rawBits":"40e00000","finite":7.0}),
        Some(true),
    )?;
    let location = at(preset, "/locationParams/value")?;
    proof::present(location, "excludeFromLevelLoad", &json!(false), Some(true))?;
    proof::present(
        location,
        "exposeForPlayerCreatedMissions",
        &json!(true),
        Some(true),
    )?;
    Ok(())
}

fn verify_preset_radar(preset: &Value) -> TestResult {
    let radar = at(preset, "/radarSettings/value")?;
    proof::present(radar, "isScannable", &json!(true), Some(true))?;
    assert_eq!(
        at(radar, "/radarProperties/value/contactType/value/guid/value")?,
        &json!(guid(RADAR_CONTACT)),
        "physical radar type reference"
    );
    assert_eq!(
        at(
            radar,
            "/radarProperties/value/authoredFields/values/0/value/value/value/rawBits"
        )?,
        "7fc01067",
        "exact unknown radar member"
    );
    Ok(())
}

#[test]
fn starmap_definitions_keep_filtered_type_links_and_a_single_reusable_definition() -> TestResult {
    let (_folder, input) = archive::open(&fixture::database(), DcbOrigin::Plain)?;
    let filter = Filter::new("selected/*");
    let catalog = document(&input, Some(&filter))?;
    assert!(
        proof::rows(&catalog, "/definitions/starmapTypes/rows")?.is_empty(),
        "the type is outside the filter"
    );
    let presets = proof::rows(&catalog, "/definitions/starmapPresets/rows")?;
    assert_eq!(presets.len(), 2, "both physical presets survive selection");
    let expected = json!(guid(TYPE));
    for preset in presets {
        assert_eq!(
            at(preset, "/type/value/guid/value")?,
            &expected,
            "original shared type identity"
        );
        assert_eq!(
            at(preset, "/type/value/status")?,
            "present",
            "physical reference remains available"
        );
    }
    let unresolved = proof::rows(&catalog, "/exportHealth/unresolvedReferences")?;
    assert!(
        unresolved.iter().any(|row| {
            row.pointer("/targetGuid/value") == Some(&expected)
                && row["kind"] == "starmapPresets"
                && row["status"] == "excludedByFilter"
        }),
        "the filter result is distinct from physical resolution"
    );
    proof::closure(&catalog)
}

fn document(input: &p4kx_extraction::Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let mut options = archive::options();
    options.filter = filter;
    Ok(serde_json::from_slice(
        build_catalog(input, &options)?.json(),
    )?)
}
