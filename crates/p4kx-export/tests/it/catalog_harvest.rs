// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Mining compositions, mining parameters, provider presets, multi configs and where the presets
//! are used, in the Basetool catalog.

use crate::harvest_fixture::{
    COMPOSITION, CONTACT_TYPE, ELEMENT, ENTITY, ENTITY_BARE, ENTITY_SILENT, HARVESTABLE,
    MULTI_CONFIG, PARAMS, PLANET, PRESET_NOWHERE, PRESET_PLACED, PRESET_PRIVATE, PRESET_UNREACHED,
    RESOURCE, write_archive, write_archive_without_mass_multiplier,
};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, Filter, Kind, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn build(filter: Option<&Filter>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn guid(value: u128) -> String {
    let text = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    )
}

fn entry<'a>(catalog: &'a Value, kind: &str, record: u128) -> Result<&'a Value, Box<dyn Error>> {
    catalog[kind]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["guid"] == guid(record).as_str())
        .ok_or_else(|| format!("{kind} has no {}", guid(record)).into())
}

#[test]
fn a_composition_lists_its_parts_and_leaves_a_dangling_element_null() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        *entry(&catalog, "mineableCompositions", COMPOSITION)?,
        json!({
            "guid": guid(COMPOSITION),
            "className": "Asteroid_CType_Iron",
            "recordPath": "libs/foundry/records/mining/rockcompositionpresets/asteroid_ctype_iron",
            "depositName": {
                "key": "@hud_mining_asteroid_name_5",
                "en": "Iron Deposit",
                "de": null
            },
            "minimumDistinctElements": 2,
            "parts": [
                {
                    "mineableElementGuid": guid(ELEMENT),
                    "minPercentage": 30.0,
                    "maxPercentage": 70.0,
                    "probability": 0.8,
                    "curveExponent": null,
                    "qualityScale": null
                },
                {
                    "mineableElementGuid": null,
                    "minPercentage": 20.0,
                    "maxPercentage": 70.0,
                    "probability": 0.8,
                    "curveExponent": null,
                    "qualityScale": null
                }
            ]
        })
    );
    let kind = built.summary().kind(Kind::MineableCompositions);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped)),
        Some((1, 1)),
        "the test composition is a leftover"
    );
    Ok(())
}

#[test]
fn the_mining_parameters_carry_their_numbers_and_nested_blocks() -> TestResult {
    let (built, catalog) = build(None)?;
    let params = entry(&catalog, "miningGlobalParams", PARAMS)?;
    assert_eq!(params["className"], "MiningGlobalParamsShip");
    assert_eq!(params["cScuPerVolume"], 0.7);
    assert_eq!(params["defaultMass"], 100.0);
    assert_eq!(params["compositionMassMultiplier"], 1.5);
    assert_eq!(params["gadgetDestroyThreshold"], Value::Null);
    assert_eq!(params["hitConsistency"]["timeExponent"], 2.0);
    assert_eq!(params["hitConsistency"]["minDeviation"], Value::Null);
    assert_eq!(params["wasteResourceTypeGuid"], guid(RESOURCE).as_str());
    let kind = built.summary().kind(Kind::MiningGlobalParams);
    assert_eq!(kind.map(|kind| (kind.count, kind.skipped)), Some((1, 1)));
    Ok(())
}

#[test]
fn a_record_without_the_mass_multiplier_leaves_it_null() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        write_archive_without_mass_multiplier(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let built = build_catalog(
        &archive,
        &CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000)),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let params = entry(&catalog, "miningGlobalParams", PARAMS)?;
    assert_eq!(params["compositionMassMultiplier"], Value::Null);
    assert_eq!(params["defaultMass"], 100.0);
    Ok(())
}

#[test]
fn a_provider_preset_follows_each_harvestable_to_its_composition() -> TestResult {
    let (_, catalog) = build(None)?;
    let preset = entry(&catalog, "harvestableProviderPresets", PRESET_PLACED)?;
    assert_eq!(preset["className"], "HPP_Placed");
    let group = &preset["groups"][0];
    assert_eq!(group["groupName"], "SpaceShip_Mineables");
    let via_preset = json!({
        "presetGuid": guid(HARVESTABLE),
        "presetClassName": "Mining_Iron",
        "entityGuid": guid(ENTITY),
        "mineableCompositionGuid": guid(COMPOSITION),
        "miningGlobalParamsGuid": guid(PARAMS),
    });
    assert_eq!(group["harvestables"][0]["harvestable"], via_preset);
    assert_eq!(group["harvestables"][0]["relativeProbability"], 22.0);
    assert_eq!(
        group["harvestables"][0]["geometryTagGuids"],
        json!([guid(RESOURCE)])
    );
    assert_eq!(
        group["harvestables"][1]["harvestable"],
        json!({
            "presetGuid": null,
            "presetClassName": null,
            "entityGuid": guid(ENTITY),
            "mineableCompositionGuid": guid(COMPOSITION),
            "miningGlobalParamsGuid": guid(PARAMS),
        }),
        "an element that names the entity only"
    );
    assert_eq!(
        preset["areas"],
        json!([{
            "name": "Acidic",
            "globalModifier": 1.0,
            "objectPresetPaths": [
                "Libs/ObjectPresets/PU/system/stanton/hurston/acidic_pools_01.opr"
            ],
            "modifiers": []
        }])
    );
    Ok(())
}

#[test]
fn a_multi_config_lists_its_tagged_configs_and_sub_harvestables() -> TestResult {
    let (_, catalog) = build(None)?;
    let config = entry(&catalog, "subHarvestableMultiConfigs", MULTI_CONFIG)?;
    assert_eq!(config["defaultConfigGuid"], guid(MULTI_CONFIG).as_str());
    assert_eq!(config["ignoreAttachableTags"], true);
    let tagged = &config["taggedConfigs"][0];
    assert_eq!(tagged["name"], "Rock");
    assert_eq!(tagged["tagGuids"], json!([guid(RESOURCE)]));
    assert_eq!(tagged["configGuid"], Value::Null);
    assert_eq!(tagged["initialSlotsProbability"], 0.5);
    let sub = &tagged["subHarvestables"][0];
    assert_eq!(sub["harvestable"]["presetGuid"], guid(HARVESTABLE).as_str());
    assert_eq!(
        sub["harvestable"]["mineableCompositionGuid"],
        guid(COMPOSITION).as_str()
    );
    assert_eq!(sub["relativeProbability"], 2.0);
    assert_eq!(sub["relativeProbabilityDeepest"], 0.15);
    Ok(())
}

#[test]
fn a_container_that_names_a_preset_is_a_location_of_the_object_it_hangs_below() -> TestResult {
    let (built, catalog) = build(None)?;
    let locations: Vec<(&str, Option<&str>, &str)> = catalog["harvestableLocations"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|location| {
            (
                location["presetGuid"].as_str().unwrap_or_default(),
                location["starmapObjectGuid"].as_str(),
                location["container"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    let planet = guid(PLANET);
    let mut expected = [
        (
            guid(PRESET_PLACED),
            Some(planet.clone()),
            "Data\\ObjectContainers\\PU\\system\\stanton\\stanton1.socpak",
        ),
        (
            guid(PRESET_PRIVATE),
            Some(planet),
            "Data\\ObjectContainers\\PU\\system\\stanton\\pmp.socpak",
        ),
        (
            guid(PRESET_UNREACHED),
            None,
            "Data\\ObjectContainers\\PU\\system\\stanton\\unreached.socpak",
        ),
    ];
    expected.sort();
    let expected: Vec<(&str, Option<&str>, &str)> = expected
        .iter()
        .map(|(preset, object, path)| (preset.as_str(), object.as_deref(), *path))
        .collect();
    assert_eq!(
        locations, expected,
        "a mining claim marker gives way to the object above, an unreached container has none, \
         and a preset no container names has no location"
    );
    assert!(
        locations
            .iter()
            .all(|(preset, _, _)| *preset != guid(PRESET_NOWHERE)),
        "no location for a preset nobody names"
    );
    assert_eq!(
        catalog["harvestableLocations"][0]["system"], "stanton",
        "the system folder"
    );
    let kind = built.summary().kind(Kind::HarvestableLocations);
    assert_eq!(kind.map(|kind| kind.count), Some(3));
    Ok(())
}

#[test]
fn a_mineable_entity_takes_its_scan_signature_from_the_slot_the_radar_parameters_name() -> TestResult
{
    let (built, catalog) = build(None)?;
    assert_eq!(
        *entry(&catalog, "mineableEntities", ENTITY)?,
        json!({
            "guid": guid(ENTITY),
            "className": "MineableRock_Iron",
            "recordPath": "libs/foundry/records/entities/mineable/mineablerock_iron",
            "mineableCompositionGuid": guid(COMPOSITION),
            "miningGlobalParamsGuid": guid(PARAMS),
            "scanSignature": 4270.0,
            "signatures": [0.0, 0.0, 4270.0, 0.0],
            "radarContactTypeGuid": guid(CONTACT_TYPE),
        }),
        "the named slot is the third, not the fifth"
    );
    let kind = built.summary().kind(Kind::MineableEntities);
    assert_eq!(kind.map(|kind| kind.count), Some(3));
    Ok(())
}

#[test]
fn an_entity_with_only_zero_signatures_has_no_scan_signature_but_keeps_the_array() -> TestResult {
    let (_, catalog) = build(None)?;
    let silent = entry(&catalog, "mineableEntities", ENTITY_SILENT)?;
    assert_eq!(silent["scanSignature"], Value::Null);
    assert_eq!(silent["signatures"], json!([0.0, 0.0, 0.0, 0.0]));
    assert_eq!(silent["radarContactTypeGuid"], guid(CONTACT_TYPE).as_str());
    Ok(())
}

#[test]
fn an_entity_without_signature_or_mining_components_leaves_every_link_null() -> TestResult {
    let (_, catalog) = build(None)?;
    let bare = entry(&catalog, "mineableEntities", ENTITY_BARE)?;
    assert_eq!(bare["scanSignature"], Value::Null);
    assert_eq!(bare["signatures"], json!([]));
    assert_eq!(bare["radarContactTypeGuid"], Value::Null);
    assert_eq!(bare["mineableCompositionGuid"], Value::Null);
    assert_eq!(bare["miningGlobalParamsGuid"], Value::Null);
    Ok(())
}

#[test]
fn a_filter_keeps_only_the_locations_of_the_presets_it_selects() -> TestResult {
    let filter = Filter::new("*providerpresets*hpp_unreached*");
    let (_, catalog) = build(Some(&filter))?;
    let presets = catalog["harvestableProviderPresets"]
        .as_array()
        .map_or(0, Vec::len);
    assert_eq!(presets, 1);
    let locations = catalog["harvestableLocations"]
        .as_array()
        .ok_or("locations")?;
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0]["presetGuid"], guid(PRESET_UNREACHED).as_str());
    assert_eq!(catalog["mineableCompositions"], json!([]), "filtered");
    assert_eq!(catalog["mineableEntities"], json!([]), "filtered");
    assert_eq!(
        catalog["completeness"]["harvestableLocations"]["status"],
        "partial"
    );
    Ok(())
}
