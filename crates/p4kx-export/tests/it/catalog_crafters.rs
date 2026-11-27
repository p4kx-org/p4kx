// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    BuiltCatalog, CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog,
};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

use crate::catalog_crafting_support::{blueprint_categories, commodity};
use crate::crafters_fixture::{
    BROKEN, CATEGORY_A, CATEGORY_B, FABRICATOR, HOST_SHIP, MODIFIED, MOUNTED, ODD, SELF_SHIP,
    write_archive,
};

#[path = "catalog_crafters_sdk.rs"]
mod sdk;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const STATIONS: &str = "/definitions/crafters/rows";
const HOSTS: &str = "/projections/crafterHosts/rows";

fn options(filter: Option<&Filter>) -> CatalogOptions<'_> {
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    options.filter = filter;
    options
}

fn build(filter: Option<&Filter>) -> TestResult<(BuiltCatalog, Value)> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let built = build_catalog(&archive, options(filter).with_full_provenance())?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}

use crate::commodity_v3_fixture::guid;

fn class_names<'a>(catalog: &'a Value, rows: &str) -> Vec<&'a str> {
    catalog
        .pointer(rows)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| row.pointer("/className/value").and_then(Value::as_str))
        .collect()
}

fn crafter(catalog: &Value, value: u128) -> TestResult<&Value> {
    let id = guid(value);
    catalog
        .pointer(STATIONS)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(id.as_str()))
        .ok_or_else(|| "the crafter".into())
}

#[test]
fn every_entity_with_a_crafter_component_is_a_crafter_whatever_its_kind() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        class_names(&catalog, STATIONS),
        [
            "bench",
            "fabricator",
            "crafting_item",
            "crafting_commodity",
            "acme_mounted",
            "acme_self",
            "modified",
            "odd",
            "broken"
        ],
        "ordered by GUID, with the template and unrelated entities excluded"
    );
    assert!(
        class_names(&catalog, "/items").contains(&"crafting_item"),
        "crafting item remains an item"
    );
    assert!(
        commodity::class_names(&catalog)?.contains(&"crafting_commodity"),
        "crafting commodity remains a commodity"
    );
    assert_eq!(
        built
            .summary()
            .kind(Kind::Crafters)
            .map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((9, 1, true)),
        "only the template that holds a component counts as skipped"
    );
    Ok(())
}

#[test]
fn authored_empty_queue_and_modifier_arrays_and_zero_are_distinct() -> TestResult {
    let (_, catalog) = build(None)?;
    let component = &crafter(&catalog, FABRICATOR)?["component"]["value"];
    assert_eq!(
        component["queues"]["value"],
        json!([]),
        "authored empty queues"
    );
    assert_eq!(
        component["modifiers"]["value"],
        json!([]),
        "authored empty modifiers"
    );
    assert_eq!(
        component["maxJobsInHistory"]["value"],
        json!({"type":"int32", "value":0}),
        "authored zero"
    );
    Ok(())
}

#[test]
fn a_filter_keeps_the_crafters_whose_path_it_matches() -> TestResult {
    let filter = Filter::new("*/basebuilding/*");
    let (built, catalog) = build(Some(&filter))?;
    assert_eq!(
        class_names(&catalog, STATIONS),
        ["bench", "fabricator", "modified", "odd", "broken"],
        "the filter selects the owning Entity Record"
    );
    assert_eq!(
        catalog["completeness"]["crafters"],
        json!({"status": "partial", "filter": "*/basebuilding/*"}),
        "catalog completeness records the filter"
    );
    assert!(
        built
            .summary()
            .kind(Kind::Crafters)
            .is_some_and(|kind| !kind.complete),
        "the summary agrees"
    );
    Ok(())
}

#[test]
fn a_ship_hosts_the_crafter_its_default_loadout_mounts_through_a_nested_port() -> TestResult {
    let (_, catalog) = build(None)?;
    let [host] = catalog
        .pointer(HOSTS)
        .and_then(Value::as_array)
        .ok_or("host projection")?
        .as_slice()
    else {
        return Err("exactly one resolved ship/crafter pair".into());
    };
    let mounted = crafter(&catalog, MOUNTED)?;
    assert_eq!(
        host["shipGuid"]["value"],
        guid(HOST_SHIP),
        "selected host ship"
    );
    assert_eq!(
        host["crafter"]["targetRecordIndex"]["value"], mounted["recordIndex"]["value"],
        "the resolved physical crafter identity"
    );
    let Some(position) = host.pointer("/mountPositions/0/value") else {
        return Err("original nested mount route".into());
    };
    assert_eq!(
        position["loadoutRecordIndex"], host["shipRecordIndex"]["value"],
        "position belongs to the selected physical ship"
    );
    assert!(
        position["portOrdinals"]
            .as_array()
            .is_some_and(|path| path.len() == 2),
        "both the empty parent and mounted child are addressed"
    );
    assert_eq!(
        host["mountPositions"][0]["containsEncryptedSource"], true,
        "mount selection retains its actual original inputs"
    );
    assert_eq!(
        mounted.pointer("/component/value/queues/value/0/value/name/value"),
        Some(&json!("Mounted queue")),
        "station remains independently readable"
    );
    assert!(
        class_names(&catalog, "/items").contains(&"acme_mounted"),
        "the mounted crafter is an item as well"
    );
    assert!(
        class_names(&catalog, "/ships").contains(&"acme_self"),
        "a ship with its own component remains a ship"
    );
    assert_eq!(
        crafter(&catalog, SELF_SHIP)?.pointer("/component/value/maxJobsInHistory/value/value"),
        Some(&json!(8)),
        "a ship with its own component remains a crafter without becoming a mounted host row"
    );
    Ok(())
}

#[test]
fn exact_time_category_and_process_flag_fields_stay_source_backed() -> TestResult {
    let (_, catalog) = build(None)?;
    let station = crafter(&catalog, MODIFIED)?;
    let component = &station["component"]["value"];
    let attach = &station["attachment"]["value"]["attachDef"]["value"];
    assert_eq!(
        attach["itemType"]["value"], "Crafter",
        "original attach type"
    );
    assert_eq!(
        attach["itemSubtype"]["value"], "Fabricator",
        "original attach subtype"
    );
    assert_eq!(
        attach["size"]["value"],
        json!({"type":"int32", "value":2}),
        "original attach size"
    );
    assert_eq!(
        component["baseCraftingTime"]["value"]["finite"], 2.5,
        "exact numeric time"
    );
    let first = &component["modifiers"]["value"][0]["value"];
    assert_eq!(
        first["modifierType"]["value"]["kind"], "time",
        "known modifier variant"
    );
    assert_eq!(
        first["modifierType"]["value"]["timeMultiplier"]["value"]["finite"], 0.75,
        "time modifier fraction"
    );
    assert_eq!(
        first.pointer("/condition/value/categories/value/0/value/guid/value"),
        Some(&json!(guid(CATEGORY_B))),
        "the modifier condition references the actual category"
    );
    assert_eq!(
        component.pointer("/modifiers/value/1/value/condition/state"),
        Some(&json!("null")),
        "the authored null condition stays distinct"
    );
    verify_queue_rules(component)?;
    assert_eq!(
        blueprint_categories::class_names(&catalog)?,
        ["category_a", "category_b"],
        "category definitions remain independently addressable"
    );
    Ok(())
}

fn verify_queue_rules(component: &Value) -> TestResult {
    let queue = component
        .pointer("/queues/value/0/value")
        .ok_or("first queue")?;
    assert_eq!(
        queue
            .pointer("/blueprintCategoryAvailability/value/kind")
            .and_then(Value::as_str),
        Some("whitelist"),
        "queue whitelist kind"
    );
    let flags = queue
        .pointer("/processTypeAvailability/value")
        .and_then(Value::as_array)
        .ok_or("availability flags")?;
    assert_eq!(
        flags
            .iter()
            .map(|flag| flag["value"].as_bool())
            .collect::<Vec<_>>(),
        [Some(true), Some(false), Some(true)],
        "all original positions and Boolean values"
    );
    assert_eq!(
        queue.pointer("/blueprintCategoryAvailability/value/categories/value/0/value/guid/value"),
        Some(&json!(guid(CATEGORY_A))),
        "the first whitelist category is preserved"
    );
    Ok(())
}

#[test]
fn unmodelled_shapes_and_original_invalid_or_null_values_are_retained() -> TestResult {
    let (_, catalog) = build(None)?;
    let odd = &crafter(&catalog, ODD)?["component"]["value"];
    let unknown = &odd["modifiers"]["value"][0]["value"]["modifierType"]["value"];
    assert_eq!(
        unknown["kind"], "original",
        "unclassified modifier remains original"
    );
    assert_eq!(
        unknown["sourceType"]["value"], "CraftingModifierType_Yield",
        "original variant type"
    );
    assert_eq!(
        unknown["additionalFields"]["roots"][0]["name"]["value"], "yieldMultiplier",
        "unknown field remains in its source arena"
    );
    let condition = &odd["modifiers"]["value"][1]["value"]["condition"]["value"];
    assert_eq!(
        condition["sourceType"]["value"], "CraftingModifierCondition_Daytime",
        "unknown condition type"
    );
    assert_eq!(
        condition["additionalFields"]["roots"][0]["name"]["value"], "hour",
        "unknown condition payload remains"
    );
    let availability =
        &odd["queues"]["value"][0]["value"]["blueprintCategoryAvailability"]["value"];
    assert_eq!(
        availability["sourceType"]["value"], "BlueprintCategoryAvailability_Blacklist",
        "unknown availability retains its type"
    );
    assert_eq!(
        availability["additionalFields"]["roots"][0]["name"]["value"], "categories",
        "unknown availability preserves original categories"
    );
    let broken = &crafter(&catalog, BROKEN)?["component"]["value"];
    assert_eq!(
        broken.pointer("/modifiers/value/0/value/modifierType/state"),
        Some(&json!("null")),
        "authored null modifier type"
    );
    assert_eq!(
        broken.pointer("/modifiers/value/1/value/modifierType/value/timeMultiplier/value/rawBits"),
        Some(&json!(format!("{:08x}", f32::NAN.to_bits()))),
        "the original nonfinite token retains its bits"
    );
    assert_eq!(broken.pointer("/queues/value/0/value/blueprintCategoryAvailability/value/categories/value/0/value/status"),
        Some(&json!("dangling")), "a dangling category stays in its authored slot");
    Ok(())
}
