// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use super::{COMPOSITION_POINTER, TestResult, composition, guids, list, named, print_line};
use crate::catalog_v3_consumer;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::too_many_lines,
    reason = "the checks of one Archive follow each other"
)]
fn an_asteroid_composition_and_a_location_of_a_real_archive_are_linked() -> TestResult {
    let path = real_archive_path()?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::real_archive_key()?))?;
    let built = build_catalog(
        &archive,
        CatalogOptions::new(SystemTime::now()).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let elements = guids(&catalog, "mineableElements")?;
    let compositions = guids(&catalog, "mineableCompositions")?;
    let objects = crate::real_archive_verification::guids(&catalog, "starmapObjects")?;
    let object_rows = catalog_v3_consumer::named_rows(&catalog, "starmapObjects")?;

    let iron = named(&catalog, "mineableCompositions", "Asteroid_CType_Iron")?;
    let parts = catalog_v3_consumer::present(&iron["parts"])?
        .as_array()
        .ok_or("the parts")?;
    print_line(&format!("composition sample: {iron}"));
    assert!(parts.len() >= 3, "an iron asteroid has several parts");
    for slot in parts {
        let part = catalog_v3_consumer::present(slot)?;
        let link = catalog_v3_consumer::present(&part["mineableElement"])?;
        let element = catalog_v3_consumer::present(&link["guid"])?
            .as_str()
            .ok_or("an element")?;
        assert!(
            elements.contains(element),
            "{element} is a mineable element"
        );
        let (min, max) = (
            composition::decimal(&part["minPercentage"])?,
            composition::decimal(&part["maxPercentage"])?,
        );
        assert!(min <= max, "the range of a part: {min} to {max}");
    }

    let presets = list(&catalog, "harvestableProviderPresets")?;
    let preset = named(&catalog, "harvestableProviderPresets", "HPP_Lagrange_A")?;
    let preset_guid = preset["guid"]["value"].as_str().ok_or("a GUID")?;
    let place = list(&catalog, "harvestableLocations")?
        .iter()
        .find(|location| {
            location["presetGuid"] == preset_guid
                && location["starmapObjectGuid"].as_str().is_some_and(|guid| {
                    object_rows.iter().any(|object| {
                        object.pointer("/guid/value").and_then(Value::as_str) == Some(guid)
                            && object.pointer("/className/value").and_then(Value::as_str)
                                == Some("Stanton1_L1_01")
                    })
                })
        })
        .ok_or("HPP_Lagrange_A is located at Stanton1_L1_01")?;
    print_line(&format!("location sample: {place}"));
    let container = place["container"].as_str().ok_or("a container")?;
    assert!(
        container.to_ascii_lowercase().ends_with(".socpak"),
        "{container}"
    );

    let mut linked = 0_usize;
    for harvestable in presets
        .iter()
        .flat_map(|preset| preset["groups"]["value"].as_array().into_iter().flatten())
        .flat_map(|group| {
            group["value"]["harvestables"]["value"]
                .as_array()
                .into_iter()
                .flatten()
        })
    {
        if let Some(composition) = harvestable
            .pointer(COMPOSITION_POINTER)
            .and_then(Value::as_str)
        {
            assert!(
                compositions.contains(composition),
                "{composition} is a kept composition"
            );
            linked += 1;
        }
    }
    assert!(
        linked > 100,
        "mineable harvestables reach compositions: {linked}"
    );
    for location in list(&catalog, "harvestableLocations")? {
        let preset = location["presetGuid"].as_str().ok_or("a preset")?;
        assert!(
            presets.iter().any(|entry| entry["guid"]["value"] == preset),
            "{preset} is a kept preset"
        );
        if let Some(object) = location["starmapObjectGuid"].as_str() {
            assert!(objects.contains(object), "{object} is a starmap object");
        }
    }
    Ok(())
}
