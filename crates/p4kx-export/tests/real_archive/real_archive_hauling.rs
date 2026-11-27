// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Hauling orders of contracts and cargo manifests in the P4KX catalog of a real Archive; runs
//! with `--ignored` when `P4KX_DATA_P4K` names one.

use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::CatalogOptions;
use p4kx_extraction::Archive;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

#[expect(
    clippy::print_stdout,
    reason = "the figures are the result of the verification"
)]
fn print_line(line: &str) {
    println!("{line}");
}

fn items(order: &Value) -> impl Iterator<Item = &Value> {
    order
        .get("requirements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|requirement| {
            requirement
                .get("options")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .flat_map(|option| option.as_array().into_iter().flatten())
}

fn dataset_rows<'catalog>(
    catalog: &'catalog Value,
    path: &str,
) -> Result<&'catalog [Value], Box<dyn Error>> {
    let dataset = catalog.pointer(path).ok_or("canonical dataset")?;
    if dataset.get("availability").and_then(Value::as_str) != Some("available") {
        return Err(format!("canonical dataset {path} unavailable").into());
    }
    dataset
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("canonical rows at {path}").into())
}

fn present<'row>(row: &'row Value, field: &str) -> Result<&'row Value, Box<dyn Error>> {
    let carrier = row
        .get(field)
        .ok_or_else(|| format!("missing sourced field {field}"))?;
    if carrier.get("state").and_then(Value::as_str) != Some("present") {
        return Err(format!("sourced field {field} is not present").into());
    }
    if carrier
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .is_none()
        || carrier
            .get("sourceSetComplete")
            .and_then(Value::as_bool)
            .is_none()
        || !matches!(
            carrier.get("containsEncryptedSource"),
            Some(Value::Null | Value::Bool(_))
        )
    {
        return Err(format!("invalid source origin for {field}").into());
    }
    carrier
        .get("value")
        .ok_or_else(|| format!("missing sourced payload for {field}").into())
}

fn commodity_subtypes(catalog: &Value) -> Result<HashSet<&str>, Box<dyn Error>> {
    let mut subtypes = HashSet::new();
    for commodity in dataset_rows(catalog, "/projections/commodityPresentation")? {
        match present(commodity, "subtypeGuid")? {
            Value::Null => {}
            value => {
                subtypes.insert(value.as_str().ok_or("Commodity subtype GUID")?);
            }
        }
    }
    Ok(subtypes)
}
#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn contracts_of_a_real_archive_list_their_hauling_orders() -> TestResult {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let built = p4kx_export::build_catalog(
        &archive,
        CatalogOptions::new(SystemTime::now()).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let contracts = crate::real_archive_contracts::v3::rows(&catalog)?;
    let subtypes = commodity_subtypes(&catalog)?;
    let mut sources: BTreeMap<&str, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    let entities = entity_guids(&catalog)?;
    let mut unknown: HashSet<&str> = HashSet::new();
    let mut unlisted = 0_usize;
    let (mut hauling, mut resources) = (0_usize, 0_usize);
    for contract in contracts {
        let orders = contract
            .get("haulingOrders")
            .and_then(Value::as_array)
            .ok_or("haulingOrders")?;
        hauling += usize::from(!orders.is_empty());
        for order in orders {
            *sources
                .entry(
                    order
                        .get("source")
                        .and_then(Value::as_str)
                        .ok_or("source")?,
                )
                .or_default() += 1;
            for item in items(order) {
                *kinds
                    .entry(item.get("kind").and_then(Value::as_str).ok_or("kind")?)
                    .or_default() += 1;
                if let Some(class) = item.get("entityClassGuid").and_then(Value::as_str) {
                    unlisted += usize::from(!entities.contains(class));
                }
                if let Some(resource) = item.get("resourceGuid").and_then(Value::as_str) {
                    resources += 1;
                    if !subtypes.contains(resource) {
                        unknown.insert(resource);
                    }
                }
            }
        }
    }
    print_line(&format!(
        "{hauling} of {} contracts haul; orders by source {sources:?}; items by kind {kinds:?}; \
         {resources} resource items, {} resources without a commodity",
        contracts.len(),
        unknown.len()
    ));
    assert!(hauling > 400, "contracts that haul: {hauling}");
    assert_eq!(unknown.len(), 11, "resources without a commodity");
    assert_eq!(unlisted, 5, "entity classes in no kind");
    Ok(())
}

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K and P4KX_CATALOG_MEASUREMENT_DIR"]
fn the_cargo_manifests_of_a_real_archive_name_commodities() -> TestResult {
    let archive = Archive::open(p4kx_testkit::real_archive_path()?)?;
    let directory = std::env::var_os("P4KX_CATALOG_MEASUREMENT_DIR")
        .map(std::path::PathBuf::from)
        .ok_or("configure a completed catalog for the exact installed Build")?;
    let catalog = read_completed_cargo(&directory)?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let subtypes = commodity_subtypes(&catalog)?;
    let manifests = dataset_rows(&catalog, "/definitions/cargoManifests")?;
    let mut fills: BTreeMap<&str, usize> = BTreeMap::new();
    let mut unmatched = HashSet::new();
    for manifest in manifests {
        let fill = present(manifest, "cargoFillCapacity")?;
        let source_type = present(fill, "sourceType")?
            .as_str()
            .ok_or("fill source type")?;
        *fills.entry(source_type).or_default() += 1;
        let slots = present(fill, "resources")?
            .as_array()
            .ok_or("ordered resources")?;
        for slot in slots {
            let state = slot
                .get("state")
                .and_then(Value::as_str)
                .ok_or("resource slot source state")?;
            if state != "present" {
                continue;
            }
            let slot = slot.get("value").ok_or("resource slot payload")?;
            let resource = slot
                .pointer("/resources/value")
                .filter(|value| value.is_object())
                .unwrap_or(slot);
            if let Some(guid) = resource
                .pointer("/resource/value/guid/value")
                .and_then(Value::as_str)
                && !subtypes.contains(guid)
            {
                unmatched.insert(guid);
            }
        }
    }
    print_line(&format!(
        "{} cargo manifests, fills {fills:?}, {} ResourceType links without a commodity",
        manifests.len(),
        unmatched.len()
    ));
    assert!(
        !manifests.is_empty(),
        "the complete Build must contain CargoManifest rows"
    );
    Ok(())
}

fn read_completed_cargo(directory: &std::path::Path) -> Result<Value, Box<dyn Error>> {
    let mut capture =
        crate::catalog_capture::CatalogCapture::<std::marker::PhantomData<Value>>::default();
    for pointer in [
        "/source",
        "/schemaVersion",
        "/completeness/cargoManifests/status",
        "/completeness/commodities/status",
        "/definitions/cargoManifests",
        "/projections/commodityPresentation",
    ] {
        capture.keep(pointer);
    }
    let catalog = crate::real_archive_catalog_size::artifact::read_completed(directory, |input| {
        Ok(capture.read(input)?)
    })?;
    if catalog.get("schemaVersion").and_then(Value::as_u64)
        != Some(u64::from(p4kx_export::SCHEMA_VERSION))
        || catalog
            .pointer("/completeness/cargoManifests/status")
            .and_then(Value::as_str)
            != Some("complete")
        || catalog
            .pointer("/completeness/commodities/status")
            .and_then(Value::as_str)
            != Some("complete")
    {
        return Err("complete same-Build CargoManifest and Commodity datasets are required".into());
    }
    Ok(catalog)
}

fn sourced(value: Value) -> Value {
    Value::Object(
        [
            ("state".to_owned(), Value::String("present".to_owned())),
            ("value".to_owned(), value),
            ("sourceUsesRefs".to_owned(), Value::Array(Vec::new())),
            ("containsEncryptedSource".to_owned(), Value::Null),
            ("sourceSetComplete".to_owned(), Value::Bool(false)),
        ]
        .into_iter()
        .collect(),
    )
}

#[test]
fn canonical_commodity_readers_keep_entity_identity_and_nullable_resource_links_separate()
-> TestResult {
    use serde_json::json;
    let catalog = json!({
        "commodities":[{"guid":"legacy-entity", "subtypeGuid":"legacy-resource"}],
        "definitions":{"commodities":{"availability":"available", "rows":[
            {"guid":sourced(json!("canonical-entity"))},
            {"guid":sourced(json!("other-entity"))}
        ]}},
        "projections":{"commodityPresentation":{"availability":"available", "rows":[
            {"commodityGuid":sourced(json!("canonical-entity")),
                "subtypeGuid":sourced(json!("canonical-resource"))},
            {"commodityGuid":sourced(json!("other-entity")), "subtypeGuid":sourced(Value::Null)}
        ]}}
    });
    let definitions = dataset_rows(&catalog, "/definitions/commodities")?;
    let declaration = definitions.first().ok_or("original Commodity definition")?;
    assert_eq!(
        present(declaration, "guid")?,
        "canonical-entity",
        "Commodity identity comes from the canonical definition"
    );
    assert_eq!(
        commodity_subtypes(&catalog)?,
        HashSet::from(["canonical-resource"]),
        "the canonical presentation supplies only the exposed ResourceType GUID"
    );
    Ok(())
}

#[test]
fn canonical_commodity_readers_reject_unavailable_or_missing_nullable_payloads() {
    use serde_json::json;
    for row in [
        json!({}),
        json!({"subtypeGuid":null}),
        json!({"subtypeGuid":{"state":"present"}}),
        json!({"subtypeGuid":{"state":"absent", "value":null}}),
        json!({"subtypeGuid":sourced(json!(false))}),
    ] {
        let catalog = json!({"projections":{"commodityPresentation":{"availability":"available", "rows":[row]}}});
        assert!(
            commodity_subtypes(&catalog).is_err(),
            "invalid sourced data cannot be silently dropped as a null link"
        );
    }
    for state in ["absent", "unavailable"] {
        let catalog =
            json!({"projections":{"commodityPresentation":{"availability":state, "rows":[]}}});
        assert!(
            commodity_subtypes(&catalog).is_err(),
            "a missing source family is not an empty resource set"
        );
    }
}

fn entity_guids(catalog: &Value) -> Result<HashSet<&str>, Box<dyn Error>> {
    let mut entities: HashSet<&str> = ["items", "ships"]
        .into_iter()
        .filter_map(|kind| catalog.get(kind).and_then(Value::as_array))
        .flatten()
        .filter_map(|entry| entry.get("guid").and_then(Value::as_str))
        .collect();
    for commodity in dataset_rows(catalog, "/definitions/commodities")? {
        entities.insert(
            present(commodity, "guid")?
                .as_str()
                .ok_or("Commodity identity")?,
        );
    }
    Ok(entities)
}
