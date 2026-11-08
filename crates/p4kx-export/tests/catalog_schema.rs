// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The checked-in JSON Schema of the Basetool catalog matches the catalog types.
//!
//! Run with `P4KX_BLESS=1` to regenerate `schemas/basetool-catalog.schema.json`.

#![recursion_limit = "256"]

mod catalog_schema_support;
#[path = "catalog_schema/health.rs"]
mod health;
#[path = "catalog_schema/mission_values.rs"]
mod mission_values;
#[path = "catalog_schema/revision.rs"]
mod revision;

use std::error::Error;
use std::path::PathBuf;

use catalog_schema_support::minimal_catalog;
use jsonschema::Validator;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn schema_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/basetool-catalog.schema.json")
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
    unknown_class["items"][0]["itemClass"] = json!("Ballistic");
    assert!(validator.is_valid(&unknown_class), "a new value is valid");

    let mut class_number = catalog.clone();
    class_number["items"][0]["itemClass"] = json!(3);
    assert!(
        !validator.is_valid(&class_number),
        "the item class stays a string"
    );

    let mut no_class = catalog.clone();
    no_class["items"][0]["itemClass"] = json!(null);
    assert!(validator.is_valid(&no_class), "the item class may be null");

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
    no_playability["items"][0]["playability"] = json!(null);
    assert!(
        validator.is_valid(&no_playability),
        "the playability may be null"
    );
    Ok(())
}

#[test]
fn the_schema_accepts_unknown_fields_and_kinds_but_not_unknown_versions() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let catalog = minimal_catalog();
    assert!(validator.is_valid(&catalog), "a minimal catalog is valid");

    let mut version_one = catalog.clone();
    version_one["schemaVersion"] = json!(1);
    assert!(
        !validator.is_valid(&version_one),
        "only the current schema version"
    );

    let mut upper_case = catalog.clone();
    upper_case["manufacturers"][0]["guid"] = json!("CFC0122D-5275-415A-A656-3DCB3346FEB5");
    assert!(!validator.is_valid(&upper_case), "lower-case GUIDs only");

    for kind in [
        "manufacturers",
        "items",
        "ships",
        "blueprints",
        "resourceGroups",
        "blueprintCategories",
        "gameplayProperties",
        "qualityDistributions",
        "qualityQuantizations",
        "blueprintPools",
        "dismantleProcesses",
        "paints",
        "loadouts",
        "contracts",
    ] {
        let mut unknown = catalog.clone();
        unknown[kind][0]["extra"] = json!(true);
        assert!(validator.is_valid(&unknown), "unknown fields in {kind}");
    }

    assert_unknown_commodity_fields(&validator, &catalog)?;

    let mut unknown_kind = catalog.clone();
    unknown_kind["starmap"] = json!([{"guid": "x"}]);
    unknown_kind["completeness"]["starmap"] = json!({"status": "complete"});
    unknown_kind["source"]["extra"] = json!(true);
    unknown_kind["completeness"]["items"]["extra"] = json!(true);
    unknown_kind["items"][0]["name"]["extra"] = json!(true);
    assert!(
        validator.is_valid(&unknown_kind),
        "unknown kinds and fields everywhere"
    );

    let mut unknown_ingredient = catalog;
    unknown_ingredient["blueprints"][0]["ingredients"][0]["extra"] = json!(true);
    assert!(
        validator.is_valid(&unknown_ingredient),
        "unknown fields in an ingredient"
    );
    Ok(())
}

#[test]
fn the_stat_blocks_accept_additions_and_new_closed_values_but_stay_required() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let mut stats = minimal_catalog();
    stats["items"][0]["stats"] = json!({
        "weapon": {
            "fireActions": [{
                "kind": "SWeaponActionFireNewParams", "name": null, "fireRate": 600.0,
                "pellets": 1, "ammoCost": 1, "heatPerShot": null, "chargeTime": null,
                "overchargeTime": null, "cooldownTime": null, "hitType": "NewHit", "fullDamageRange": null,
                "zeroDamageRange": null, "damagePerSecond": null, "spread": null,
                "extra": true
            }],
            "ammoCapacity": null,
            "ammo": null,
            "extra": true
        },
        "shield": null, "quantumDrive": null, "powerPlant": null, "cooler": null,
        "armor": null, "clothing": null, "radar": null, "tractorBeam": null,
        "weaponModifier": null, "miningLaser": null, "miningModifier": null,
        "missile": {
            "damage": null, "explosionMinRadius": null, "explosionMaxRadius": null,
            "trackingSignalType": "NewSignal", "lockTime": null, "lockingAngle": null,
            "lockRangeMin": null, "lockRangeMax": null, "linearSpeed": null
        },
        "health": 1.5, "extra": true,
        "resourceStates": [{
            "name": null, "emSignature": null, "irSignature": {"nominal": 1.0, "decayRate": null},
            "flows": [{
                "kind": "ItemResourceDeltaNew", "direction": "generation", "resource": "New",
                "amount": 1.0, "unit": null
            }]
        }]
    });
    assert!(
        validator.is_valid(&stats),
        "stat blocks with additions and new values"
    );
    let mut no_block = stats.clone();
    if let Some(block) = no_block["items"][0]["stats"].as_object_mut() {
        block.remove("cooler");
    }
    assert!(!validator.is_valid(&no_block), "every block is present");
    let mut no_action_field = stats.clone();
    if let Some(action) =
        no_action_field["items"][0]["stats"]["weapon"]["fireActions"][0].as_object_mut()
    {
        action.remove("fireRate");
    }
    assert!(
        !validator.is_valid(&no_action_field),
        "every field of a fire mode is present"
    );
    let mut number_kind = stats;
    number_kind["items"][0]["stats"]["weapon"]["fireActions"][0]["kind"] = json!(3);
    assert!(
        !validator.is_valid(&number_kind),
        "the kind of a fire mode stays a string"
    );
    Ok(())
}

#[test]
fn every_field_the_schema_lists_is_required() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (kind, field, what) in [
        ("blueprints", "tiers", "a blueprint lists its tiers"),
        (
            "blueprints",
            "dismantle",
            "a blueprint states its dismantle",
        ),
        ("resourceGroups", "parentGuid", "a resource group"),
        ("ships", "crewSize", "a ship"),
        ("ships", "variantOf", "a ship states its variant base"),
        ("items", "variantOf", "an item states its variant base"),
        ("items", "stats", "an item states its stat blocks"),
        ("blueprintPools", "rewardSources", "a pool"),
    ] {
        let mut catalog = minimal_catalog();
        if let Some(entity) = catalog[kind][0].as_object_mut() {
            entity.remove(field);
        }
        assert!(!validator.is_valid(&catalog), "{what} has {field}");
    }
    let mut no_quantity = minimal_catalog();
    if let Some(ingredient) = no_quantity["blueprints"][0]["ingredients"][0].as_object_mut() {
        ingredient.remove("quantityScu");
    }
    assert!(
        !validator.is_valid(&no_quantity),
        "every field of an ingredient is present"
    );
    let mut no_modifiers = minimal_catalog();
    if let Some(line) = no_modifiers["blueprints"][0]["tiers"][0]["mandatory"].as_object_mut() {
        line.remove("modifiers");
    }
    assert!(
        !validator.is_valid(&no_modifiers),
        "every field of a cost line is present"
    );
    let mut no_global = minimal_catalog();
    if let Some(catalog) = no_global.as_object_mut() {
        catalog.remove("craftingGlobal");
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
fn the_crafting_fields_accept_additions_and_new_closed_values_but_stay_required() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let catalog = minimal_catalog();
    let mut unknown_line = catalog.clone();
    unknown_line["blueprints"][0]["tiers"][0]["mandatory"]["options"][0]["extra"] = json!(true);
    unknown_line["blueprints"][0]["dismantle"]["extra"] = json!(true);
    unknown_line["craftingGlobal"]["extra"] = json!(true);
    assert!(
        validator.is_valid(&unknown_line),
        "unknown fields in a cost line, a dismantle link and the global parameters"
    );

    let mut new_kind_value = catalog;
    new_kind_value["blueprints"][0]["tiers"][0]["mandatory"]["kind"] = json!("bundle");
    new_kind_value["blueprints"][0]["tiers"][0]["mandatory"]["options"][0]["modifiers"][0]["ranges"]
        [0]["kind"] = json!("curve");
    assert!(
        validator.is_valid(&new_kind_value),
        "a new line or range kind is additive: the closed sets stay strings"
    );

    Ok(())
}

fn assert_unknown_commodity_fields(validator: &Validator, catalog: &Value) -> TestResult {
    for pointer in [
        "/definitions/commodities/rows/0",
        "/projections/commodityPresentation/rows/0",
    ] {
        let mut unknown = catalog.clone();
        unknown
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .ok_or("the canonical Commodity row")?
            .insert("extra".to_owned(), json!(true));
        assert!(validator.is_valid(&unknown), "unknown fields in {pointer}");
    }

    Ok(())
}
