// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Commodities and blueprints in the Basetool catalog of a real Archive; runs with `--ignored`
//! when `P4KX_DATA_P4K` names one.

use std::collections::HashSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, DECOMPRESSED_LIMIT, Kind, build_catalog, catalog_schema};
use p4kx_extraction::Archive;
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use crate::catalog_v3_consumer;
use crate::real_archive_canonical_texts::{TextDomain, TextIndex, TextReferenceShape};

type TestResult = Result<(), Box<dyn Error>>;

const MIN_COMMODITIES: usize = 100;
const MIN_BLUEPRINTS: usize = 1_000;
const MIN_LINKED: f64 = 0.95;

fn list<'c>(catalog: &'c Value, kind: &str) -> Result<&'c Vec<Value>, String> {
    catalog_v3_consumer::named_rows(catalog, kind).map_err(|error| error.to_string())
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a share of counts far below 2^52"
)]
fn ratio(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        1.0
    } else {
        part as f64 / whole as f64
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn builds_commodities_and_blueprints_of_a_real_archive() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let validator = jsonschema::validator_for(&catalog_schema())?;
    let errors: Vec<String> = validator
        .iter_errors(&catalog)
        .take(5)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "valid against the schema: {errors:?}");
    report(&built);
    assert!(
        built.summary().size() < DECOMPRESSED_LIMIT,
        "under the decompressed limit"
    );
    for (kind, min) in [
        (Kind::Commodities, MIN_COMMODITIES),
        (Kind::Blueprints, MIN_BLUEPRINTS),
    ] {
        let counts = built.summary().kind(kind).ok_or("the kind")?;
        assert!(counts.count >= min, "plausible {kind} count: {counts:?}");
        assert!(counts.complete, "{kind} complete without a Filter");
        let guids: Vec<&str> = list(&catalog, kind.name())?
            .iter()
            .map(|entity| catalog_v3_consumer::identity(entity, kind))
            .collect::<Result<_, _>>()?;
        assert!(guids.is_sorted(), "{kind} ordered by GUID");
    }
    check_commodities(&catalog)?;
    check_blueprints(&catalog)
}

fn check_commodities(catalog: &Value) -> TestResult {
    let commodities = catalog_v3_consumer::rows(catalog, Kind::Commodities)?;
    let choices = catalog_v3_consumer::presentations(catalog)?;
    let texts = TextIndex::new(catalog)?;
    let mut gold = None;
    for commodity in commodities {
        let name = catalog_v3_consumer::field(commodity, Kind::Commodities, "className")?
            .as_str()
            .ok_or("Commodity className")?;
        if gold.is_none() && name.eq_ignore_ascii_case("gold") {
            gold = Some(commodity);
        }
    }
    let id = catalog_v3_consumer::identity(gold.ok_or("the commodity Gold")?, Kind::Commodities)?;
    let choice = choices
        .iter()
        .find(|row| row.pointer("/commodityGuid/value").and_then(Value::as_str) == Some(id))
        .ok_or("Gold presentation")?;
    check_gold(choice, &texts)?;
    let described = described_count(choices, &texts)?;
    assert!(
        ratio(described, commodities.len()) > 0.9,
        "most commodities have a description: {described} of {}",
        commodities.len()
    );
    Ok(())
}

fn check_gold(choice: &Value, texts: &TextIndex<'_>) -> TestResult {
    let name = texts
        .reference(
            choice.get("name").ok_or("name carrier")?,
            TextDomain::PublicName,
            TextReferenceShape::Nullable,
        )?
        .ok_or("Gold name reference")?;
    assert_eq!(name.shown("en")?, &Value::from("Gold"), "English name");
    for field in ["typeGuid", "subtypeGuid"] {
        let carrier = choice.get(field).ok_or("Gold link carrier")?;
        assert!(
            catalog_v3_consumer::nullable_guid(carrier)?.is_some(),
            "Gold {field} reaches its selected group or ResourceType"
        );
    }
    Ok(())
}

fn described_count(choices: &[Value], texts: &TextIndex<'_>) -> Result<usize, Box<dyn Error>> {
    let mut described = 0;
    for choice in choices {
        let text = texts.reference(
            choice.get("description").ok_or("description carrier")?,
            TextDomain::GameText,
            TextReferenceShape::Nullable,
        )?;
        if let Some(text) = text
            && text
                .shown("en")?
                .as_str()
                .is_some_and(|text| !text.is_empty())
        {
            described += 1;
        }
    }
    Ok(described)
}

fn check_blueprints(catalog: &Value) -> TestResult {
    let blueprints = list(catalog, "blueprints")?;
    let items: HashSet<&str> = list(catalog, "items")?
        .iter()
        .filter_map(|item| item["guid"].as_str())
        .collect();
    let produced: Vec<&str> = blueprints
        .iter()
        .filter_map(|blueprint| blueprint["producedItemGuid"].as_str())
        .collect();
    let known = produced.iter().filter(|guid| items.contains(*guid)).count();
    assert!(
        ratio(known, produced.len()) >= MIN_LINKED,
        "{known} of {} produced items are catalog items",
        produced.len()
    );
    check_ingredients(catalog)?;
    check_craft_times(blueprints);
    Ok(())
}

fn check_ingredients(catalog: &Value) -> TestResult {
    let blueprints = list(catalog, "blueprints")?;
    let lines: Vec<&Value> = blueprints
        .iter()
        .filter_map(|blueprint| blueprint["ingredients"].as_array())
        .flatten()
        .collect();
    assert!(!lines.is_empty(), "ingredient lines");
    let resource_lines: HashSet<&str> = lines
        .iter()
        .filter_map(|line| line["resourceGuid"].as_str())
        .collect();
    check_resource_links(catalog, &resource_lines)?;
    assert!(
        lines.iter().any(|line| line["itemGuid"].is_string()),
        "some ingredient is an item"
    );
    assert!(
        lines
            .iter()
            .all(|line| line["resourceGuid"].is_string() != line["itemGuid"].is_string()),
        "a line is a resource or an item"
    );
    assert!(
        lines
            .iter()
            .any(|line| line["minQuality"].as_i64().is_some_and(|q| q > 0)),
        "some ingredient has a minimum quality"
    );
    Ok(())
}

/// Validates source resource identity independently of zero, one or many packaging alternatives.
pub(crate) fn check_resource_links(catalog: &Value, lines: &HashSet<&str>) -> TestResult {
    let resources: HashSet<_> = catalog_v3_consumer::rows(catalog, Kind::ResourceTypes)?
        .iter()
        .map(|row| catalog_v3_consumer::identity(row, Kind::ResourceTypes))
        .collect::<Result<_, _>>()?;
    let missing: Vec<_> = lines
        .iter()
        .filter(|guid| !resources.contains(**guid))
        .collect();
    if lines.is_empty() || !missing.is_empty() {
        return Err(
            format!("every resource line must join a canonical ResourceType: {missing:?}").into(),
        );
    }
    Ok(())
}

fn check_craft_times(blueprints: &[Value]) {
    let timed = blueprints
        .iter()
        .filter(|blueprint| {
            blueprint["craftTimeSeconds"]
                .as_i64()
                .is_some_and(|s| s > 0)
        })
        .count();
    assert!(
        ratio(timed, blueprints.len()) > 0.9,
        "most blueprints have a craft time: {timed} of {}",
        blueprints.len()
    );
}

#[expect(
    clippy::print_stdout,
    reason = "the sizes and counts are the measurement the upload limit is sized from"
)]
fn report(built: &p4kx_export::BuiltCatalog) {
    println!("catalog size: {} bytes", built.summary().size());
    for kind in built.summary().kinds() {
        println!("{kind:?}");
    }
    for text in built.summary().texts() {
        println!("{text:?}");
    }
}
