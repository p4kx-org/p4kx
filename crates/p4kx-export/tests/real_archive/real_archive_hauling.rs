// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Hauling orders of contracts and cargo manifests in the Basetool catalog of a real Archive; runs
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
    order["requirements"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|requirement| requirement["options"].as_array().into_iter().flatten())
        .flat_map(|option| option.as_array().into_iter().flatten())
}

fn commodity_rows<'catalog>(
    catalog: &'catalog Value,
    path: &str,
) -> Result<&'catalog [Value], Box<dyn Error>> {
    let dataset = catalog.pointer(path).ok_or("canonical Commodity dataset")?;
    if dataset.get("availability").and_then(Value::as_str) != Some("available") {
        return Err("canonical Commodity dataset unavailable".into());
    }
    dataset
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "canonical Commodity rows".into())
}

fn present<'row>(row: &'row Value, field: &str) -> Result<&'row Value, Box<dyn Error>> {
    let carrier = row.get(field).ok_or("Commodity sourced field")?;
    if carrier.get("state").and_then(Value::as_str) != Some("present") {
        return Err("Commodity sourced field is not present".into());
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
        return Err("Commodity sourced origin shape".into());
    }
    carrier
        .get("value")
        .ok_or_else(|| "Commodity sourced payload".into())
}

fn commodity_subtypes(catalog: &Value) -> Result<HashSet<&str>, Box<dyn Error>> {
    let mut subtypes = HashSet::new();
    for commodity in commodity_rows(catalog, "/projections/commodityPresentation")? {
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
    let built = p4kx_export::build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let contracts = catalog["contracts"].as_array().ok_or("contracts")?;
    let subtypes = commodity_subtypes(&catalog)?;
    let mut sources: BTreeMap<&str, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    let entities = entity_guids(&catalog)?;
    let mut unknown: HashSet<&str> = HashSet::new();
    let mut unlisted = 0_usize;
    let (mut hauling, mut resources) = (0_usize, 0_usize);
    for contract in contracts {
        let orders = contract["haulingOrders"]
            .as_array()
            .ok_or("haulingOrders")?;
        hauling += usize::from(!orders.is_empty());
        for order in orders {
            *sources
                .entry(order["source"].as_str().ok_or("source")?)
                .or_default() += 1;
            for item in items(order) {
                *kinds
                    .entry(item["kind"].as_str().ok_or("kind")?)
                    .or_default() += 1;
                if let Some(class) = item["entityClassGuid"].as_str() {
                    unlisted += usize::from(!entities.contains(class));
                }
                if let Some(resource) = item["resourceGuid"].as_str() {
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
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_cargo_manifests_of_a_real_archive_name_commodities() -> TestResult {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let built = p4kx_export::build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let subtypes = commodity_subtypes(&catalog)?;
    let manifests = catalog["cargoManifests"]
        .as_array()
        .ok_or("cargoManifests")?;
    let mut fills: BTreeMap<&str, usize> = BTreeMap::new();
    let mut unknown: HashSet<&str> = HashSet::new();
    let (mut resources, mut rules) = (0_usize, 0_usize);
    for manifest in manifests {
        *fills
            .entry(manifest["fillType"].as_str().ok_or("fillType")?)
            .or_default() += 1;
        rules += manifest["spawnRules"].as_array().map_or(0, Vec::len);
        for resource in manifest["resources"].as_array().ok_or("resources")? {
            resources += 1;
            if let Some(guid) = resource["resourceGuid"].as_str()
                && !subtypes.contains(guid)
            {
                unknown.insert(guid);
            }
        }
    }
    print_line(&format!(
        "{} cargo manifests, fills {fills:?}, {resources} resources ({} resources \
         without a commodity), {rules} spawn rules",
        manifests.len(),
        unknown.len()
    ));
    assert!(manifests.len() > 60, "manifests: {}", manifests.len());
    assert_eq!(unknown.len(), 4, "resources without a commodity");
    Ok(())
}

#[cfg(test)]
#[must_use]
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
    let definitions = commodity_rows(&catalog, "/definitions/commodities")?;
    assert_eq!(present(&definitions[0], "guid")?, "canonical-entity");
    assert_eq!(
        commodity_subtypes(&catalog)?,
        HashSet::from(["canonical-resource"])
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
    for commodity in commodity_rows(catalog, "/definitions/commodities")? {
        entities.insert(
            present(commodity, "guid")?
                .as_str()
                .ok_or("Commodity identity")?,
        );
    }
    Ok(entities)
}
