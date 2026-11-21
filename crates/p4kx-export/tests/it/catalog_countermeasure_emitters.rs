// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Filter;
use serde_json::{Value, json};

use super::admission::assert_retained_ammo_dependencies;
use super::fixture::{Case, PRIMARY, SECONDARY, WEAPON};
use super::{ROWS as SELECTIONS, build};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const EMITTERS: &str = "/configurations/countermeasureEmitters/rows";
const AMMO: &str = "/definitions/ammoDefinitions/rows";
const CONTAINERS: &str = "/configurations/ammoContainers/rows";
const WEAPON_PATH: &str = "*/fixture_projectile_own_route";

#[test]
fn countermeasure_emitters_project_proved_roles_without_copying_ammo() -> TestResult {
    let catalog = build(Case::Countermeasure, DcbOrigin::Encrypted, None)?;
    let [primary, secondary] = proof::rows(&catalog, EMITTERS)? else {
        return Err("one emitter per original weapon, container and ammo role".into());
    };
    for (row, role, ammo, kind) in [
        (primary, "primary", PRIMARY, "chaff"),
        (secondary, "secondary", SECONDARY, "flare"),
    ] {
        proof::present(row, "ownerGuid", &json!(common::guid(WEAPON)), Some(true))?;
        proof::present(row, "componentOrdinal", &json!(1), Some(true))?;
        let route = proof::at(proof::cell(row, "route", "present")?, "/value")?;
        proof::present(route, "ammoRole", &json!(role), Some(true))?;
        proof::present(route, "containerComponentOrdinal", &json!(2), Some(true))?;
        let [container] = proof::rows(&catalog, CONTAINERS)? else {
            return Err("one canonical original container".into());
        };
        proof::present(container, "maxAmmoCount", &json!(3), Some(true))?;
        proof::present(container, "maxRestockCount", &json!(29), Some(true))?;
        assert_eq!(
            route["containerRecordIndex"]["value"], container["ownerRecordIndex"]["value"],
            "the route points at the physical configuration"
        );
        check_ammo(&catalog, route, ammo, kind)?;
    }
    let selections = proof::rows(&catalog, SELECTIONS)?;
    assert_eq!(selections.len(), 3, "two primary actions share one emitter");
    let schema = p4kx_export::catalog_schema();
    jsonschema::validator_for(&schema)?
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    proof::closure(&catalog)
}

#[test]
fn countermeasure_emitters_require_materialized_definition_and_proved_route() -> TestResult {
    let filter = Filter::parse(WEAPON_PATH)?;
    let filtered = build(Case::Countermeasure, DcbOrigin::Plain, Some(&filter))?;
    let [primary, secondary] = proof::rows(&filtered, EMITTERS)? else {
        return Err(
            "each proved route with readable countermeasure ammunition supplies an emitter".into(),
        );
    };
    assert_retained_ammo_dependencies(&filtered, [PRIMARY, SECONDARY])?;
    for (emitter, role, guid) in [
        (primary, "primary", PRIMARY),
        (secondary, "secondary", SECONDARY),
    ] {
        let route = proof::at(emitter, "/route/value")?;
        proof::present(route, "ammoRole", &json!(role), Some(false))?;
        let ammo = proof::rows(&filtered, AMMO)?
            .iter()
            .find(|row| row.pointer("/guid/value") == Some(&json!(common::guid(guid))))
            .ok_or("original dependency Ammo for the emitter")?;
        assert_eq!(
            proof::at(route, "/ammoRecordIndex/value")?,
            proof::at(ammo, "/recordIndex/value")?,
            "each emitter uses its own physical dependency"
        );
    }
    assert_eq!(
        proof::rows(&filtered, SELECTIONS)?.len(),
        3,
        "the authored projectile selections survive the filter"
    );
    let ordinary = build(Case::Unique, DcbOrigin::Plain, None)?;
    assert!(
        proof::rows(&ordinary, EMITTERS)?.is_empty(),
        "the same physical routing does not classify a bullet"
    );
    proof::closure(&filtered)?;
    proof::closure(&ordinary)
}

fn check_ammo(catalog: &Value, route: &Value, guid: u128, kind: &str) -> TestResult {
    let index = proof::at(route, "/ammoRecordIndex/value")?;
    let definition = proof::rows(catalog, AMMO)?
        .iter()
        .find(|row| row.pointer("/recordIndex/value") == Some(index))
        .ok_or("physical ammunition target")?;
    proof::present(definition, "guid", &json!(common::guid(guid)), Some(true))?;
    let projectile = proof::at(definition, "/projectileParams/value/kind")?;
    assert_eq!(
        projectile["type"], "countermeasure",
        "original projectile type"
    );
    let body = proof::at(projectile, "/value/typeParams/value")?;
    assert_eq!(body["type"], kind, "original signature subtype");
    proof::present(
        proof::at(body, "/value")?,
        "startInfrared",
        &json!({"rawBits": "40a00000", "finite": 5.0}),
        Some(true),
    )?;
    assert!(
        route.get("maxAmmoCount").is_none() && route.get("startInfrared").is_none(),
        "capacities and signatures have one canonical representation"
    );
    Ok(())
}
