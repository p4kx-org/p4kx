// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The checked-in JSON Schema of the P4KX catalog matches the catalog types.
//!
//! Run with `P4KX_BLESS=1` to regenerate `schemas/p4kx-catalog.schema.json`.

#![recursion_limit = "256"]

#[path = "catalog_schema/additive.rs"]
mod additive;
mod catalog_schema_support;
#[path = "catalog_schema/dataset_ownership.rs"]
mod dataset_ownership;
#[path = "catalog_schema/gameplay_properties.rs"]
mod gameplay_properties;
#[path = "catalog_schema/health.rs"]
mod health;
#[path = "catalog_schema/legacy_v2_inventory.rs"]
mod legacy_v2_inventory;
#[path = "catalog_schema/legacy_v2_inventory_tests.rs"]
mod legacy_v2_inventory_tests;
#[path = "catalog_schema/legacy_v2_migration.rs"]
mod legacy_v2_migration;
#[path = "catalog_schema/mission_values.rs"]
mod mission_values;
#[path = "catalog_schema/quality_contract.rs"]
mod quality_contract;
#[path = "catalog_schema/resource_groups.rs"]
mod resource_groups;
#[path = "catalog_schema/revision.rs"]
mod revision;
#[path = "catalog_schema/schema_navigation.rs"]
mod schema_navigation;

use std::error::Error;
use std::path::PathBuf;

use catalog_schema_support::minimal_catalog;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn schema_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/p4kx-catalog.schema.json")
}

fn generated() -> Result<String, Box<dyn Error>> {
    let mut text = serde_json::to_string_pretty(&p4kx_export::catalog_schema())?;
    text.push('\n');
    Ok(text)
}

#[test]
fn the_checked_in_catalog_schema_is_current() -> TestResult {
    let generated = generated()?;
    let path = schema_file();
    if std::env::var_os("P4KX_BLESS").is_some_and(|value| value == "1") {
        std::fs::write(&path, &generated)?;
    }
    let checked_in = std::fs::read_to_string(&path)?;
    assert!(
        checked_in == generated,
        "{} is stale; regenerate it with P4KX_BLESS=1 cargo test -p p4kx-export --test catalog_schema",
        path.display()
    );
    Ok(())
}

#[test]
fn the_catalog_schema_is_draft_2020_12() {
    let schema = p4kx_export::catalog_schema();
    assert_eq!(
        schema.get("$schema").and_then(Value::as_str),
        Some("https://json-schema.org/draft/2020-12/schema"),
        "draft 2020-12"
    );
}

#[test]
fn a_new_value_of_a_closed_value_set_is_additive() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let catalog = minimal_catalog();

    let mut unknown_class = catalog.clone();
    unknown_class["items"][0]["itemClass"]["value"] = json!("Ballistic");
    assert!(validator.is_valid(&unknown_class), "a new value is valid");

    let mut class_number = catalog.clone();
    class_number["items"][0]["itemClass"]["value"] = json!(3);
    assert!(
        !validator.is_valid(&class_number),
        "the item class stays a string"
    );

    let mut no_class = catalog.clone();
    no_class["items"][0]["itemClass"]["state"] = json!("absent");
    no_class["items"][0]["itemClass"]["value"] = json!(null);
    assert!(
        validator.is_valid(&no_class),
        "the item class may be absent"
    );

    let mut unknown_playability = catalog.clone();
    unknown_playability["ships"][0]["playability"] = json!("event");
    assert!(
        validator.is_valid(&unknown_playability),
        "a new playability is valid"
    );

    let mut unknown_kind = catalog.clone();
    unknown_kind["ships"][0]["vehicleKind"] = json!("hoverVehicle");
    unknown_kind["ships"][0]["massBasis"] = json!("estimate");
    assert!(
        validator.is_valid(&unknown_kind),
        "a new vehicle kind and mass basis are valid"
    );

    let mut kind_number = catalog.clone();
    kind_number["ships"][0]["vehicleKind"] = json!(1);
    assert!(
        !validator.is_valid(&kind_number),
        "the vehicle kind stays a string"
    );

    let mut unknown_values = catalog.clone();
    for field in [
        "vehicleKind",
        "size",
        "mass",
        "massBasis",
        "hullHealth",
        "cargoScu",
    ] {
        unknown_values["ships"][0][field] = json!(null);
    }
    assert!(
        validator.is_valid(&unknown_values),
        "every ship base value may be null"
    );

    let mut no_playability = catalog;
    no_playability["items"][0]["playability"]["state"] = json!("absent");
    no_playability["items"][0]["playability"]["value"] = json!(null);
    assert!(
        validator.is_valid(&no_playability),
        "the playability may be absent"
    );
    Ok(())
}

#[test]
fn the_schema_accepts_unknown_fields_and_kinds_but_not_unknown_versions() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let catalog = minimal_catalog();
    let errors: Vec<_> = validator
        .iter_errors(&catalog)
        .map(|error| format!("{:?} at {}", error.kind(), error.instance_path()))
        .collect();
    assert!(errors.is_empty(), "invalid synthetic catalog: {errors:?}");

    let mut version_one = catalog.clone();
    version_one["schemaVersion"] = json!(1);
    assert!(
        !validator.is_valid(&version_one),
        "only the current schema version"
    );

    let mut upper_case = catalog.clone();
    upper_case["definitions"]["manufacturers"]["rows"][0]["guid"]["value"] =
        json!("CFC0122D-5275-415A-A656-3DCB3346FEB5");
    assert!(!validator.is_valid(&upper_case), "lower-case GUIDs only");

    for kind in ["items", "ships", "dismantleProcesses", "paints", "loadouts"] {
        let mut unknown = catalog.clone();
        unknown[kind][0]["extra"] = json!(true);
        assert!(validator.is_valid(&unknown), "unknown fields in {kind}");
    }

    let mut unknown_contract = catalog.clone();
    unknown_contract["projections"]["contracts"]["rows"][0]["extra"] = json!(true);
    assert!(
        validator.is_valid(&unknown_contract),
        "unknown fields on canonical contracts"
    );
    quality_contract::assert_unknown_quality_fields(&validator, &catalog)?;
    let mut unknown_manufacturer = catalog.clone();
    unknown_manufacturer["definitions"]["manufacturers"]["rows"][0]["extra"] = json!(true);
    assert!(
        validator.is_valid(&unknown_manufacturer),
        "unknown manufacturer fields"
    );
    for pointer in [
        "/definitions/blueprintPools/rows/0",
        "/projections/blueprintPoolWeights/rows/0",
        "/declarations/blueprintRewardSources/rows/0",
    ] {
        let mut unknown = catalog.clone();
        unknown
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .ok_or("canonical pool contract fixture row")?
            .insert("extra".into(), json!(true));
        assert!(validator.is_valid(&unknown), "unknown fields in {pointer}");
    }
    additive::assert_unknown_canonical_fields(&validator, &catalog)?;

    additive::assert_unknown_nested_fields(&validator, catalog)
}

#[test]
fn items_publish_canonical_component_tables_without_a_flat_stat_field() -> TestResult {
    let schema = serde_json::to_value(p4kx_export::catalog_schema())?;
    let item = schema
        .pointer("/$defs/Item")
        .ok_or("catalog Item definition")?;
    for field in [
        "stats",
        "name",
        "description",
        "displayType",
        "type",
        "subType",
        "size",
        "grade",
        "mass",
        "manufacturerGuid",
        "tags",
    ] {
        assert!(
            item.pointer(&format!("/properties/{field}")).is_none(),
            "the Item projection omits canonical data {field}"
        );
    }
    assert!(item.pointer("/properties/recordIndex").is_some());
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        validator.is_valid(&minimal_catalog()),
        "canonical tables validate independently"
    );
    Ok(())
}

#[test]
fn every_field_the_schema_lists_is_required() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (kind, field, what) in [
        ("ships", "crewSize", "a ship"),
        ("ships", "variantOf", "a ship states its variant base"),
        ("items", "variantOf", "an item states its variant base"),
    ] {
        let mut catalog = minimal_catalog();
        if let Some(entity) = catalog[kind][0].as_object_mut() {
            entity.remove(field);
        }
        assert!(!validator.is_valid(&catalog), "{what} has {field}");
    }
    for (pointer, field) in [
        ("/definitions/blueprints/rows/0", "source"),
        ("/definitions/blueprintPools/rows/0", "blueprintRewards"),
        (
            "/definitions/blueprintPools/rows/0/blueprintRewards/value/0/value",
            "blueprintRecord",
        ),
        ("/projections/blueprintPoolWeights/rows/0", "totalWeight"),
        (
            "/declarations/blueprintRewardSources/rows/0",
            "sourceOrdinal",
        ),
        ("/declarations/blueprintRewardSources/rows/0", "poolStorage"),
        ("/declarations/blueprintRewardSources/rows/0", "pool"),
        ("/configurations/blueprintTiers/rows/0", "costs"),
        (
            "/configurations/blueprintTiers/rows/0/costs/0/meaning",
            "quantityScu",
        ),
        (
            "/configurations/blueprintTiers/rows/0/costs/0/context",
            "modifiers",
        ),
        ("/projections/blueprintPresentation/rows/0", "dismantle"),
    ] {
        let mut catalog = minimal_catalog();
        let row = catalog
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .ok_or("the blueprint schema fixture row")?;
        assert!(row.remove(field).is_some(), "fixture {pointer}/{field}");
        assert!(
            !validator.is_valid(&catalog),
            "{pointer}/{field} is required"
        );
    }
    let mut no_global = minimal_catalog();
    if let Some(configurations) = no_global["configurations"].as_object_mut() {
        configurations.remove("craftingGlobal");
    }
    assert!(!validator.is_valid(&no_global), "the global parameters");
    Ok(())
}

#[test]
fn the_loadout_fields_accept_additions_and_new_closed_values_but_stay_required() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let catalog = minimal_catalog();
    let mut unknown_port = catalog.clone();
    unknown_port["loadouts"][0]["ports"][0]["ports"][0]["extra"] = json!(true);
    unknown_port["loadouts"][0]["ports"][0]["types"] = json!([{"type": "Turret", "extra": 1,
        "subTypes": []}]);
    assert!(
        validator.is_valid(&unknown_port),
        "unknown fields in a nested port and a port type"
    );

    let mut new_values = catalog.clone();
    new_values["loadouts"][0]["entityKind"] = json!("vehicle");
    new_values["loadouts"][0]["ports"][0]["source"] = json!("modification");
    new_values["loadouts"][0]["ports"][0]["visibility"] = json!("hidden");
    assert!(
        validator.is_valid(&new_values),
        "a new entity kind, source or visibility is additive: the closed sets stay strings"
    );

    for (path, field) in [
        ("entityKind", None),
        ("issues", None),
        ("ports", Some("itemGuid")),
        ("ports", Some("dockedVehicle")),
        ("ports", Some("unresolved")),
        ("ports", Some("ports")),
        ("ports", Some("requiredTags")),
    ] {
        let mut missing = catalog.clone();
        let target = match field {
            None => missing["loadouts"][0].as_object_mut(),
            Some(_) => missing["loadouts"][0][path][0].as_object_mut(),
        };
        if let Some(target) = target {
            target.remove(field.unwrap_or(path));
        }
        assert!(
            !validator.is_valid(&missing),
            "{path} {field:?} stays required"
        );
    }
    Ok(())
}

#[test]
fn crafting_fields_allow_additions_but_cost_meanings_are_closed() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let catalog = minimal_catalog();
    let mut unknown_field = catalog.clone();
    unknown_field["configurations"]["blueprintTiers"]["rows"][0]["costs"][0]["extra"] = json!(true);
    unknown_field["projections"]["blueprintPresentation"]["rows"][0]["dismantle"]["value"]["extra"] =
        json!(true);
    unknown_field["configurations"]["craftingGlobal"]["rows"][0]["extra"] = json!(true);
    assert!(
        validator.is_valid(&unknown_field),
        "unknown fields are additive"
    );

    let mut unknown_cost = catalog;
    unknown_cost["configurations"]["blueprintTiers"]["rows"][0]["costs"][0]["meaning"]["type"] =
        json!("bundle");
    assert!(
        !validator.is_valid(&unknown_cost),
        "cost meanings have a closed shape"
    );

    Ok(())
}
