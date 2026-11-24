// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_export::mining_locations::{
    HarvestableLocation, MiningLocationOptions, MiningLocationSnapshot, read_mining_locations,
};
use p4kx_extraction::{Archive, ArchiveWatch, Language};
use serde::Serialize;
use serde_json::{Value, json};

use crate::catalog_capture::CatalogCapture;
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_catalog_table_parity::{
    BoundMeasurement, first_difference, with_bound_measurement,
};
use crate::real_archive_verification::print_line;

#[path = "real_archive_mining_catalog_scope.rs"]
mod occurrence_text;
#[path = "real_archive_mining_catalog_rows.rs"]
mod rows;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const STARMAP_DEFINITIONS: &str = "/definitions/starmapObjects";
const STARMAP_PLACEMENTS: &str = "/placements/starmapPlacements";
const RESOURCE_TYPES: &str = "/definitions/resourceTypes";

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_mining_sdk_matches_the_same_complete_catalog() -> TestResult {
    with_bound_measurement(|archive, measurement| {
        verify_mining_and_row_arrays(measurement, archive, &[])
    })
}

pub(crate) fn verify_mining_and_row_arrays(
    measurement: &BoundMeasurement,
    archive: &Archive,
    arrays: &[(&str, &[Value])],
) -> TestResult {
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let snapshot =
        read_mining_locations(archive, MiningLocationOptions::new(Language::English, None))
            .map_err(|_| "read the mining SDK snapshot")?;
    watch.check()?;
    let catalog = read_catalog(measurement, &snapshot, arrays)?;
    check_mining_scope(&catalog, archive, &snapshot)?;
    compare_rows(&catalog, &snapshot)?;
    for (path, expected) in arrays {
        compare_ordered(&catalog, path, expected)?;
    }
    watch.check()?;
    print_line(&format!(
        "mining SDK parity: elements={} compositions={} providers={} locations={} starmap={} resources={} cataloguedRoutes={} SDKOnlyOccurrences={} publicNames={} gameTexts={}",
        snapshot.mineable_elements.len(),
        snapshot.mineable_compositions.len(),
        snapshot.harvestable_provider_presets.len(),
        snapshot.harvestable_locations.len(),
        snapshot.starmap_objects.len(),
        snapshot.resource_types.len(),
        rows_at(&catalog, rows::CONTAINERS)?.len(),
        snapshot
            .harvestable_occurrences
            .len()
            .checked_sub(rows_at(&catalog, rows::CONTAINERS)?.len())
            .ok_or("Catalog routes outnumber SDK occurrences")?,
        snapshot.public_names.len(),
        snapshot.game_texts.len(),
    ));
    Ok(())
}

fn read_catalog(
    measurement: &BoundMeasurement,
    snapshot: &MiningLocationSnapshot,
    arrays: &[(&str, &[Value])],
) -> TestResult<Value> {
    let keys = rows::SelectedKeys::for_snapshot(snapshot)?;
    let mut paths = CatalogCapture::default();
    for path in [
        "/schemaVersion",
        "/source",
        "/completeness",
        "/exportHealth/status",
        "/exportHealth/read",
        "/exportHealth/projection",
        "/exportHealth/publicationMode",
        "/texts/languageSources/en",
        "/mineableElements",
        "/mineableCompositions",
        "/harvestableProviderPresets",
        "/harvestableLocations",
        STARMAP_DEFINITIONS,
        STARMAP_PLACEMENTS,
        RESOURCE_TYPES,
        "/placements/containerInstances/availability",
    ] {
        paths.keep(path);
    }
    for (path, _) in arrays {
        paths.keep(path);
    }
    keys.project(&mut paths);
    measurement.read_catalog(&paths)
}

fn check_mining_scope(
    catalog: &Value,
    archive: &Archive,
    snapshot: &MiningLocationSnapshot,
) -> TestResult {
    if snapshot.build.as_ref() != archive.build()
        || !snapshot.coverage.language_available
        || snapshot.coverage.archive_inventory_incomplete
        || snapshot.coverage.placement_inventory_incomplete
        || catalog
            .pointer("/texts/languageSources/en/state")
            .and_then(Value::as_str)
            != Some("present")
    {
        return Err(
            "mining parity requires a readable locale and complete source inventories".into(),
        );
    }
    Ok(())
}

fn compare_rows(catalog: &Value, snapshot: &MiningLocationSnapshot) -> TestResult {
    compare_ordered(catalog, "/mineableElements", &snapshot.mineable_elements)?;
    compare_ordered(
        catalog,
        "/mineableCompositions",
        &snapshot.mineable_compositions,
    )?;
    compare_ordered(
        catalog,
        "/harvestableProviderPresets",
        &snapshot.harvestable_provider_presets,
    )?;
    compare_ordered(
        catalog,
        "/harvestableLocations",
        &logical_locations(&snapshot.harvestable_locations),
    )?;
    compare_starmap(catalog, snapshot)?;
    compare_resources(catalog, snapshot)?;
    occurrence_text::compare_routes(catalog, snapshot)?;
    occurrence_text::compare_texts(catalog, snapshot)
}

fn logical_locations(rows: &[HarvestableLocation]) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            json!({
                "presetGuid": row.preset_guid.to_string(),
                "starmapObjectGuid": row.starmap_object_guid.map(|guid| guid.to_string()),
                "system": row.system,
                "container": row.container,
            })
        })
        .collect()
}

fn compare_ordered<T: Serialize>(catalog: &Value, path: &str, sdk: &[T]) -> TestResult {
    let exported = rows_at(catalog, path)?;
    if exported.len() != sdk.len() || sdk.is_empty() {
        return Err(format!("mismatched or empty Catalog and SDK rows at {path}").into());
    }
    for (position, (actual, typed)) in exported.iter().zip(sdk).enumerate() {
        let projected = wire_value(typed)?;
        if let Some(field) = first_difference(actual, &projected) {
            return Err(format!("Catalog and SDK rows differ at {path}/{position}{field}").into());
        }
    }
    Ok(())
}

fn wire_value<T: Serialize>(value: &T) -> serde_json::Result<Value> {
    serde_json::from_slice(&serde_json::to_vec(value)?)
}

fn compare_starmap(catalog: &Value, snapshot: &MiningLocationSnapshot) -> TestResult {
    let definitions = index_rows(
        consumer::table(catalog, STARMAP_DEFINITIONS)?,
        "/recordIndex/value",
    )?;
    let placements = index_rows(
        consumer::table(catalog, STARMAP_PLACEMENTS)?,
        "/objectRecordIndex/value",
    )?;
    if snapshot.starmap_objects.is_empty()
        || definitions.len() != snapshot.starmap_objects.len()
        || placements.len() != definitions.len()
    {
        return Err("Catalog and SDK Starmap inventories differ".into());
    }
    let mut previous = BTreeSet::new();
    for object in &snapshot.starmap_objects {
        let index = u64::from(
            *object
                .source
                .record_index
                .value()
                .ok_or("SDK Starmap Record index")?,
        );
        if !previous.insert(index) {
            return Err("duplicate physical SDK Starmap Record".into());
        }
        let definition = definitions.get(&index).ok_or("Catalog Starmap Record")?;
        compare_fields(
            definition,
            &serde_json::to_value(&object.source)?,
            "Starmap identity",
        )?;
        compare_fields(
            definition,
            &json!({"name": object.name, "parent": object.parent}),
            "authored Starmap cells",
        )?;
        let placement = placements.get(&index).ok_or("Catalog Starmap placement")?;
        compare_fields(
            placement,
            &json!({
                "objectRecordIndex": object.source.record_index,
                "objectGuid": object.source.guid.map_ref(ToString::to_string),
                "system": object.system,
                "entityName": object.entity_name,
                "placedUnderGuid": object.placed_under_guid.map_ref(ToString::to_string),
                "position": object.position,
            }),
            "placed Starmap cells",
        )?;
    }
    Ok(())
}

fn compare_resources(catalog: &Value, snapshot: &MiningLocationSnapshot) -> TestResult {
    let resources = index_rows(
        consumer::table(catalog, RESOURCE_TYPES)?,
        "/recordIndex/value",
    )?;
    let ore: BTreeSet<_> = rows_at(catalog, "/mineableElements")?
        .iter()
        .filter_map(|row| resource_target(row, "/resourceType/value"))
        .collect();
    let mut reachable = ore.clone();
    for index in ore {
        let row = resources
            .get(&index)
            .ok_or("the original ore link has no Catalog resource row")?;
        reachable.extend(resource_target(row, "/refinedVersion/value"));
    }
    if reachable.len() != snapshot.resource_types.len() || reachable.is_empty() {
        return Err("Catalog and SDK reachable resource inventories differ".into());
    }
    for row in &snapshot.resource_types {
        let index = u64::from(
            *row.source
                .record_index
                .value()
                .ok_or("SDK resource Record index")?,
        );
        if !reachable.remove(&index) {
            return Err("SDK resource Record is duplicated or not a matching dependency".into());
        }
        let resource = resources.get(&index).ok_or("Catalog resource Record")?;
        compare_fields(resource, &wire_value(row)?, "reachable resource cells")?;
    }
    Ok(())
}

fn resource_target(row: &Value, pointer: &str) -> Option<u64> {
    let reference = row.pointer(pointer)?;
    if reference.get("status")?.as_str()? != "present"
        || reference.pointer("/targetType/value")?.as_str()? != "ResourceType"
    {
        return None;
    }
    reference.pointer("/targetRecordIndex/value")?.as_u64()
}

fn index_rows<'catalog>(
    rows: &'catalog [Value],
    field: &str,
) -> TestResult<BTreeMap<u64, &'catalog Value>> {
    let mut index = BTreeMap::new();
    for row in rows {
        let identity = row
            .pointer(field)
            .and_then(Value::as_u64)
            .ok_or("required physical Record index")?;
        if index.insert(identity, row).is_some() {
            return Err("duplicate physical Catalog Record index".into());
        }
    }
    Ok(index)
}

fn rows_at<'catalog>(catalog: &'catalog Value, path: &str) -> TestResult<&'catalog [Value]> {
    catalog
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("required catalog rows at {path}").into())
}

fn compare_fields(actual: &Value, expected: &Value, scope: &str) -> TestResult {
    for (field, value) in expected
        .as_object()
        .ok_or("the SDK row must be an object")?
    {
        require_equal(
            actual.get(field),
            Some(value),
            &format!("{scope}, field {field}"),
        )?;
    }
    Ok(())
}

fn require_equal(actual: Option<&Value>, expected: Option<&Value>, scope: &str) -> TestResult {
    if actual.is_none() || actual != expected {
        return Err(format!("catalog comparison differs for {scope}").into());
    }
    Ok(())
}

#[test]
fn ordered_mining_rows_compare_the_wire_decimal_for_binary32_cells() -> TestResult {
    #[derive(Serialize)]
    struct Tuning {
        finite: f32,
    }

    let catalog = serde_json::from_str(r#"{"mineableElements":[{"finite":0.1}]}"#)?;
    compare_ordered(&catalog, "/mineableElements", &[Tuning { finite: 0.1 }])?;

    let mismatch = compare_ordered(&catalog, "/mineableElements", &[Tuning { finite: 0.2 }]);
    assert!(
        mismatch.is_err_and(|error| error.to_string().contains("/mineableElements/0/finite")),
        "a different wire value still fails at the original field"
    );
    Ok(())
}
