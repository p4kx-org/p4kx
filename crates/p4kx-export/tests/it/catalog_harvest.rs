// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Mining compositions, mining parameters, provider presets, multi configs and where the presets
//! are used, in the P4KX catalog.

use crate::harvest_fixture::{
    COMPOSITION, CONTACT_TYPE, ELEMENT, ENTITY, ENTITY_BARE, ENTITY_SILENT, HARVESTABLE,
    MULTI_CONFIG, PARAMS, PLANET, PRESET_NOWHERE, PRESET_PLACED, PRESET_PRIVATE, PRESET_UNREACHED,
    RESOURCE, write_archive, write_archive_without_mass_multiplier,
};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    BuiltCatalog, CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog,
};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[path = "harvest_provider_sdk.rs"]
mod provider_sdk;

fn build(filter: Option<&Filter>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    build_archive(&archive, filter)
}

fn build_archive(
    archive: &Archive,
    filter: Option<&Filter>,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    options.filter = filter;
    let built = build_catalog(archive, &options)?;
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
    let requested = guid(record);
    let rows = if kind == "miningGlobalParams" {
        catalog.pointer("/configurations/miningGlobal/rows")
    } else {
        catalog.get(kind)
    };
    rows.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|entry| {
            let identity = &entry["guid"];
            identity == requested.as_str() || identity["value"] == requested.as_str()
        })
        .ok_or_else(|| format!("{kind} has no {requested}").into())
}

#[test]
fn a_composition_preserves_authored_order_widths_and_dangling_reference() -> TestResult {
    let (built, catalog) = build(None)?;
    let source = entry(&catalog, "mineableCompositions", COMPOSITION)?;
    verify_composition_header(&catalog, source)?;
    assert_eq!(
        source["minimumDistinctElements"]["value"],
        json!({"type": "int32", "value": 2})
    );
    let parts = source["parts"]["value"].as_array().ok_or("parts")?;
    assert_eq!(parts.len(), 2, "authored length");
    let first = &parts[0]["value"];
    let second = &parts[1]["value"];
    let link = &first["mineableElement"]["value"];
    assert_eq!(link["status"], "present");
    assert_eq!(link["guid"]["value"], guid(ELEMENT));
    let dangling = &second["mineableElement"]["value"];
    assert_eq!(dangling["status"], "dangling");
    assert_eq!(dangling["guid"]["value"], guid(RESOURCE + 0xff));
    assert_eq!(dangling["targetRecordIndex"]["state"], "absent");
    assert_eq!(first["minPercentage"]["value"]["finite"], 30.0);
    assert_eq!(second["minPercentage"]["value"]["finite"], 20.0);
    assert_eq!(
        first["qualityScale"]["value"],
        json!({"type": "f64", "rawBits": "3fe0000000000001", "finite": 0.500_000_000_000_000_1})
    );
    assert_eq!(first["curveExponent"]["state"], "absent");
    for value in [
        &source["parts"],
        &parts[0],
        &parts[1],
        &first["maxPercentage"],
    ] {
        assert_eq!(value["containsEncryptedSource"], true);
        assert!(
            value["sourceUsesRefs"]
                .as_array()
                .is_some_and(|ids| !ids.is_empty()),
            "each original array and field occurrence has direct evidence"
        );
    }
    assert_ne!(
        first["maxPercentage"]["sourceUsesRefs"], second["maxPercentage"]["sourceUsesRefs"],
        "equal values at different positions keep distinct uses"
    );
    let kind = built.summary().kind(Kind::MineableCompositions);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped)),
        Some((1, 1)),
        "the test composition is a leftover"
    );
    Ok(())
}

fn verify_composition_header(catalog: &Value, source: &Value) -> TestResult {
    crate::catalog_mining::verify_name(
        catalog,
        source,
        "depositName",
        "@hud_mining_asteroid_name_5",
        Some("Iron Deposit"),
    )?;
    assert_eq!(
        source.pointer("/className/value").and_then(Value::as_str),
        Some("Asteroid_CType_Iron"),
        "original composition class name",
    );
    assert_eq!(
        source.pointer("/recordPath/value").and_then(Value::as_str),
        Some("libs/foundry/records/mining/rockcompositionpresets/asteroid_ctype_iron"),
        "original composition Record path",
    );
    Ok(())
}

#[test]
fn the_mining_parameters_carry_their_numbers_and_nested_blocks() -> TestResult {
    let (built, catalog) = build(None)?;
    let params = entry(&catalog, "miningGlobalParams", PARAMS)?;
    assert_eq!(params["className"]["value"], "MiningGlobalParamsShip");
    assert_eq!(params["cSCUPerVolume"]["value"]["finite"], 0.7);
    assert_eq!(params["defaultMass"]["value"]["finite"], 100.0);
    assert_eq!(params["compositionMassMultiplier"]["value"]["finite"], 1.5);
    assert_eq!(params["gadgetDestroyThreshold"]["state"], "absent");
    assert_eq!(
        params["hitConsistencyParams"]["value"]["timeExponent"]["value"]["finite"],
        2.0
    );
    assert_eq!(
        params["hitConsistencyParams"]["value"]["minDeviation"]["state"],
        "absent"
    );
    assert_eq!(
        params["wasteResourceType"]["value"]["guid"]["value"],
        guid(RESOURCE).as_str()
    );
    let kind = built.summary().kind(Kind::MiningGlobalParams);
    assert_eq!(kind.map(|kind| (kind.count, kind.skipped)), Some((1, 1)));
    Ok(())
}

#[test]
fn a_record_without_the_mass_multiplier_retains_its_absence() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        write_archive_without_mass_multiplier(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000))
    };
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let params = entry(&catalog, "miningGlobalParams", PARAMS)?;
    assert_eq!(params["compositionMassMultiplier"]["state"], "absent");
    assert_eq!(params["defaultMass"]["value"]["finite"], 100.0);
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
