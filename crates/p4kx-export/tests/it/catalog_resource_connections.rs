// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_resource_connections_fixture.rs"]
mod fixture;

#[test]
fn power_and_heat_connections_preserve_distinct_authored_fields_and_origin() -> TestResult {
    let database = fixture::database();
    let schema = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        schema
            .validate(&catalog)
            .map_err(|error| format!("catalog schema: {error}"))?;
        let [power, heat] = proof::rows(&catalog, "/configurations/resourceConnections/rows")?
        else {
            return Err("two physical connection rows".into());
        };
        compare_power(power, encrypted)?;
        compare_heat(heat, encrypted)?;
        compare_parameters(&catalog, encrypted)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn compare_power(row: &Value, encrypted: bool) -> TestResult {
    assert_eq!(row["kind"], "power", "the original electrical component");
    owner(row, fixture::POWER, 1, encrypted)?;
    singles(row, &fixture::POWER_SINGLES, encrypted)?;
    for (field, value) in [
        ("SafeguardPriority", json!(0)),
        ("DisplayedInPoweredItemList", json!(false)),
        ("IsThrottleable", json!(true)),
        ("IsOverclockable", json!(false)),
    ] {
        proof::present(row, field, &value, Some(encrypted))?;
    }
    let [key] = proof::rows(row, "/MisfireItemTypeLocID/value/values")? else {
        return Err("one original localized key".into());
    };
    proof::present(
        key,
        "value",
        &json!({
            "type":"locale", "value":{"type":"gameText", "sourceText":fixture::KEY},
        }),
        Some(encrypted),
    )?;
    proof::cell(row, "MisfireGenerationParams", "present")?;
    proof::cell(row, "MisfireEvents", "present")?;
    assert!(
        proof::rows(row, "/additionalFields/roots")?.is_empty(),
        "represented fields occur once"
    );
    Ok(())
}

fn compare_heat(row: &Value, encrypted: bool) -> TestResult {
    assert_eq!(row["kind"], "heat", "the original thermal component");
    owner(row, fixture::HEAT, 2, encrypted)?;
    singles(row, &fixture::HEAT_SINGLES, encrypted)?;
    let [raw] = proof::rows(row, "/additionalFields/roots")? else {
        return Err("the additional authored thermal field".into());
    };
    proof::present(raw, "name", &json!("fixtureThermalBias"), Some(encrypted))?;
    assert!(
        row.get("PowerToEM").is_none(),
        "heat has its own field shape"
    );
    Ok(())
}

fn compare_parameters(catalog: &Value, encrypted: bool) -> TestResult {
    let [row] = proof::rows(catalog, "/configurations/resourceComponents/rows")? else {
        return Err("one owning resource configuration".into());
    };
    let [network, empty, first, second, unknown] = proof::rows(row, "/controlParameters/value")?
    else {
        return Err("all five original control slots".into());
    };
    let network = proof::at(network, "/value")?;
    assert_eq!(
        network["kind"], "networkParameter",
        "original network parameter"
    );
    proof::present(network, "sourcePosition", &json!(0), Some(encrypted))?;
    assert_eq!(empty["state"], "null", "physical null slot survives");
    for (slot, position, label) in [(first, 2, "FixtureFirst"), (second, 3, "FixtureSecond")] {
        let external = proof::at(slot, "/value")?;
        assert_eq!(
            external["kind"], "externalPort",
            "original external port type"
        );
        proof::present(
            external,
            "sourcePosition",
            &json!(position),
            Some(encrypted),
        )?;
        proof::present(external, "parameter", &json!(label), Some(encrypted))?;
        proof::present(external, "name", &json!(""), Some(encrypted))?;
        proof::present(
            external,
            "portName",
            &json!("fixture_external"),
            Some(encrypted),
        )?;
    }
    let unknown = proof::at(unknown, "/value")?;
    assert_eq!(
        unknown["kind"], "unknownParameter",
        "an unrecognized subtype remains explicit"
    );
    let [field] = proof::rows(unknown, "/additionalFields/roots")? else {
        return Err("future parameter's original member".into());
    };
    proof::present(
        field,
        "name",
        &json!("fixtureExternalFact"),
        Some(encrypted),
    )?;
    Ok(())
}

fn owner(row: &Value, source: &str, slot: u32, encrypted: bool) -> TestResult {
    proof::present(row, "sourceType", &json!(source), Some(encrypted))?;
    proof::present(
        row,
        "ownerGuid",
        &json!(common::guid(fixture::OWNER)),
        Some(encrypted),
    )?;
    proof::present(row, "componentOrdinal", &json!(slot), Some(encrypted))?;
    Ok(())
}

fn singles(row: &Value, expected: &[(&str, f32)], encrypted: bool) -> TestResult {
    for &(field, value) in expected {
        proof::present(
            row,
            field,
            &json!({
                "rawBits":format!("{:08x}",value.to_bits()),
                "finite":value.is_finite().then_some(value),
            }),
            Some(encrypted),
        )?;
    }
    Ok(())
}
