// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Complete synthetic resource declarations through the public catalog interface.

use std::error::Error;

use p4kx_export::Kind;
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use crate::catalog_crafting_support;
use crate::resource_fixture as fixture;
use crate::resource_quality_schema_fixture::OverrideEntryTarget;

type TestResult = Result<(), Box<dyn Error>>;

fn build(
    target: OverrideEntryTarget,
) -> Result<(p4kx_export::BuiltCatalog, Value), Box<dyn Error>> {
    catalog_crafting_support::build_with_entries(
        &fixture::database(target),
        vec![TestEntry::zstd(
            "Data\\Libs\\CosmoTools\\LayerRecovery\\LayerBackups.xml",
            b"<LayerBackups/>".to_vec(),
        )],
    )
}

fn row<'catalog>(
    catalog: &'catalog Value,
    kind: &str,
    guid: u128,
) -> Result<&'catalog Value, Box<dyn Error>> {
    let wanted = catalog_crafting_support::text(guid);
    catalog[kind]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["guid"] == wanted))
        .ok_or_else(|| format!("the synthetic {kind} row").into())
}

#[test]
fn commodity_identity_and_resource_cardinality_are_independent() -> TestResult {
    let (built, catalog) = build(OverrideEntryTarget::Concrete)?;
    assert_eq!(
        built
            .summary()
            .kind(Kind::ResourceTypes)
            .map(|kind| (kind.count, kind.complete)),
        Some((3, true))
    );
    let commodities = catalog["commodities"].as_array().ok_or("the commodities")?;
    assert_eq!(
        commodities.len(),
        4,
        "three Entities and the existing resource-only fallback"
    );
    let subtype = catalog_crafting_support::text(fixture::RESOURCE_ALPHA);
    assert_eq!(
        commodities
            .iter()
            .filter(|row| row["subtypeGuid"] == subtype)
            .count(),
        2
    );
    assert_ne!(
        row(&catalog, "commodities", fixture::COMMODITY_ALPHA)?["guid"],
        row(&catalog, "commodities", fixture::COMMODITY_BETA)?["guid"]
    );
    assert_eq!(
        row(&catalog, "commodities", fixture::COMMODITY_NULL)?["subtypeGuid"],
        Value::Null
    );
    let fallback = row(&catalog, "commodities", fixture::RESOURCE_BETA)?;
    assert_eq!(fallback["guid"], fallback["subtypeGuid"]);
    assert!(row(&catalog, "resourceTypes", fixture::RESOURCE_UNUSED).is_ok());
    assert!(row(&catalog, "commodities", fixture::RESOURCE_UNUSED).is_err());
    Ok(())
}

#[test]
fn concrete_and_base_declared_override_entries_produce_the_same_catalog() -> TestResult {
    let (_, mut live) = build(OverrideEntryTarget::Concrete)?;
    let (_, mut ptu) = build(OverrideEntryTarget::Base)?;
    for catalog in [&mut live, &mut ptu] {
        let object = catalog.as_object_mut().ok_or("the catalog object")?;
        assert!(object.remove("source").is_some());
    }
    assert_eq!(
        live, ptu,
        "the owned concrete value controls projection for either declaration"
    );
    Ok(())
}

#[test]
fn source_units_molar_mass_and_legacy_commodity_density_stay_distinct() -> TestResult {
    let (_, catalog) = build(OverrideEntryTarget::Base)?;
    let alpha = row(&catalog, "resourceTypes", fixture::RESOURCE_ALPHA)?;
    assert_eq!(
        alpha["density"]["payload"]["densityUnit"]["payload"],
        json!({"type": "gramsPerCubicCentimeter", "value": 3.25})
    );
    let beta = row(&catalog, "resourceTypes", fixture::RESOURCE_BETA)?;
    assert_eq!(
        beta["density"]["payload"]["densityUnit"]["payload"],
        json!({"type": "kilogramsPerCubicMeter", "value": 12.5})
    );
    assert_eq!(
        row(&catalog, "resourceTypes", fixture::RESOURCE_UNUSED)?["density"]["payload"],
        json!({"type": "molarMass", "molarMass": 6.75})
    );
    assert_eq!(
        row(&catalog, "commodities", fixture::COMMODITY_ALPHA)?["densityGramsPerCubicCentimetre"],
        3.25
    );
    assert_eq!(
        row(&catalog, "commodities", fixture::RESOURCE_BETA)?["densityGramsPerCubicCentimetre"],
        Value::Null
    );
    Ok(())
}

#[test]
fn cargo_slots_flags_media_paths_and_property_positions_are_authored_facts() -> TestResult {
    let (_, catalog) = build(OverrideEntryTarget::Concrete)?;
    let alpha = row(&catalog, "resourceTypes", fixture::RESOURCE_ALPHA)?;
    assert_eq!(alpha["validateDefaultCargoBox"], false);
    assert_eq!(
        alpha["defaultThumbnailPath"],
        "textures/fixture/resource.dds"
    );
    assert_eq!(alpha["defaultThumbnailPathSVG"], "");
    assert_eq!(
        alpha["rttThumbnailEntityClass"]["guid"],
        catalog_crafting_support::text(fixture::CONTAINER)
    );
    let slots = alpha["defaultCargoContainers"]
        .as_array()
        .ok_or("the cargo slots")?;
    let names: Option<Vec<_>> = slots.iter().map(|slot| slot["slot"].as_str()).collect();
    assert_eq!(
        names,
        Some(vec![
            "one_eighthSCU",
            "one_quarterSCU",
            "one_halfSCU",
            "oneSCU",
            "twoSCU",
            "fourSCU",
            "eightSCU",
            "sixteenSCU",
            "twentyFourSCU",
            "thirtyTwoSCU"
        ])
    );
    assert_eq!(
        slots[0]["entityClass"]["guid"],
        slots[4]["entityClass"]["guid"]
    );
    assert_eq!(slots[1]["entityClass"], Value::Null);
    let properties = alpha["properties"]
        .as_array()
        .ok_or("the ordered properties")?;
    assert_eq!(properties.len(), 4);
    assert_eq!(properties[0]["name"], "");
    assert_eq!(properties[0]["payload"]["volatility"], 0.0);
    assert_eq!(properties[2], Value::Null);
    assert_eq!(properties[3]["payload"]["healthDecayPerSecond"], 0.0);
    Ok(())
}

#[test]
fn quality_sources_reuse_exact_curves_and_preserve_nested_record_links() -> TestResult {
    let (_, catalog) = build(OverrideEntryTarget::Base)?;
    let alpha =
        &row(&catalog, "resourceTypes", fixture::RESOURCE_ALPHA)?["properties"][1]["payload"];
    let curve = &alpha["qualityDistribution"]["payload"]["value"];
    assert_eq!(curve["min"], 16_777_217);
    assert_eq!(curve["max"], i32::MAX);
    assert_eq!(curve["mean"], 9.5);
    let overrides = &alpha["qualityLocationOverride"]["payload"]["value"]["locationOverrideList"];
    assert_eq!(
        overrides[0]["payload"]["location"]["guid"],
        catalog_crafting_support::text(fixture::LOCATION)
    );
    assert_eq!(
        overrides[0]["payload"]["qualityDistribution"]["payload"]["reference"]["guid"],
        catalog_crafting_support::text(fixture::DISTRIBUTION)
    );
    assert_eq!(overrides[1], Value::Null);
    let beta = &row(&catalog, "resourceTypes", fixture::RESOURCE_BETA)?["properties"][0]["payload"];
    assert_eq!(
        beta["qualityLocationOverride"]["payload"]["reference"]["guid"],
        catalog_crafting_support::text(fixture::OVERRIDE)
    );
    assert_eq!(
        beta["qualityQuantization"]["payload"]["value"]["bands"],
        json!([{"start": 0, "end": 42, "mappedValue": 11}])
    );
    assert_eq!(
        alpha["qualityQuantization"]["payload"]["reference"]["guid"],
        catalog_crafting_support::text(fixture::QUANTIZATION)
    );
    assert_eq!(
        row(&catalog, "qualityDistributions", fixture::DISTRIBUTION)?["distribution"]["min"],
        i32::MIN
    );
    Ok(())
}

#[test]
fn group_source_memberships_keep_duplicates_nulls_and_cycles() -> TestResult {
    let (_, catalog) = build(OverrideEntryTarget::Concrete)?;
    let alpha = row(&catalog, "resourceTypes", fixture::RESOURCE_ALPHA)?;
    assert_eq!(
        alpha["groupGuids"],
        json!([
            catalog_crafting_support::text(fixture::GROUP_ALPHA),
            catalog_crafting_support::text(fixture::GROUP_BETA)
        ])
    );
    let group = row(&catalog, "resourceGroups", fixture::GROUP_ALPHA)?;
    assert_eq!(group["defaultThumbnailPathSVG"], "");
    assert_eq!(group["resources"].as_array().map(Vec::len), Some(4));
    assert_eq!(group["resources"][1], Value::Null);
    assert_eq!(group["resources"][0], group["resources"][2]);
    assert_eq!(
        group["parentGuid"],
        catalog_crafting_support::text(fixture::GROUP_BETA)
    );
    assert_eq!(
        row(&catalog, "commodities", fixture::COMMODITY_ALPHA)?["typeGuid"],
        catalog_crafting_support::text(fixture::GROUP_ALPHA)
    );
    Ok(())
}

#[test]
fn source_only_entity_links_do_not_fabricate_missing_catalog_targets() -> TestResult {
    let (_, catalog) = build(OverrideEntryTarget::Base)?;
    assert_eq!(catalog["exportHealth"]["status"], "complete");
    let target = catalog_crafting_support::text(fixture::CONTAINER);
    let references = catalog["exportHealth"]["unresolvedReferences"]
        .as_array()
        .ok_or("the unresolved references")?;
    assert!(
        !references
            .iter()
            .any(|reference| reference["targetGuid"] == target)
    );
    Ok(())
}
