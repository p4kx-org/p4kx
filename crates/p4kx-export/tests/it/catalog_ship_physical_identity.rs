// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeEntry, Field, Record, StructValue, TreeBuilder, Value as Raw};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_ship_physical_fixture.rs"]
mod fixture;

use fixture::Case;

#[test]
fn same_guid_ship_records_keep_their_physical_authored_values() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = fixture::open(&folder.path().join("Data.p4k"))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let records: Vec<_> = forge.records().collect();
    assert_eq!(
        records.len(),
        2,
        "the fixture has two actual physical owners"
    );
    assert_eq!(
        records[0].guid(),
        records[1].guid(),
        "the original GUIDs alias"
    );
    assert_eq!(
        records[0].record_path()?,
        records[1].record_path()?,
        "the paths alias"
    );
    assert_ne!(
        records[0].index(),
        records[1].index(),
        "the original owners differ"
    );
    let mut trees = TreeBuilder::new();
    for (record, case) in records.iter().zip(&fixture::CASES) {
        assert_eq!(
            record.name()?,
            format!("EntityClassDefinition.{}", case.class)
        );
        let tree = trees.build(record)?;
        check_originals(tree.fields(), case)?;
    }
    let options = common::options();
    let built = p4kx_export::build_catalog_from(&archive, &entry, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let rows = proof::rows(&catalog, "/ships")?;
    assert_eq!(rows.len(), records.len(), "same-GUID rows remain distinct");
    for ((row, record), case) in rows.iter().zip(&records).zip(&fixture::CASES) {
        check_ship(&catalog, row, record, case)?;
    }
    assert_ne!(rows[0]["crewSize"]["value"], rows[1]["crewSize"]["value"]);
    assert_ne!(rows[0]["insurance"]["value"], rows[1]["insurance"]["value"]);
    assert_ne!(
        rows[0]["vehicleDefinition"]["value"],
        rows[1]["vehicleDefinition"]["value"]
    );
    proof::closure(&catalog)
}

fn check_ship(catalog: &Value, row: &Value, record: &Record<'_>, case: &Case) -> TestResult {
    let index = proof::present(row, "recordIndex", &json!(record.index()), Some(true))?;
    proof::header(
        catalog,
        index,
        record.index(),
        "recordIndex",
        "selector",
        "archiveEntry",
    )?;
    proof::present(row, "guid", &json!(record.guid().to_string()), Some(true))?;
    proof::present(row, "className", &json!(case.class), Some(true))?;
    let crew = proof::present(
        row,
        "crewSize",
        &json!({"type": "int32", "value": case.crew}),
        Some(true),
    )?;
    let definition = proof::present(
        row,
        "vehicleDefinition",
        &json!(case.definition),
        Some(true),
    )?;
    body_owner(catalog, crew, record.index())?;
    body_owner(catalog, definition, record.index())?;
    check_insurance(catalog, row, record.index(), case)
}

fn check_originals(fields: &[Field<'_>], case: &Case) -> TestResult {
    let vehicle = selected(fields, "Components", "VehicleComponentParams")?;
    assert!(
        matches!(field(vehicle.fields(), "crewSize")?, Raw::Int32(value) if *value == case.crew),
        "the original crew size belongs to this case"
    );
    assert!(
        matches!(field(vehicle.fields(), "vehicleDefinition")?, Raw::String(value) if *value == case.definition),
        "the original vehicle selector retains exact whitespace"
    );
    let insurance = selected(
        fields,
        "StaticEntityClassData",
        "SEntityInsuranceProperties",
    )?;
    assert!(
        matches!(field(insurance.fields(), "restoreOnClaim")?, Raw::Bool(value) if *value == case.restore),
        "the restore flag belongs to this physical Ship"
    );
    let Raw::Struct(params) = field(insurance.fields(), "shipInsuranceParams")? else {
        return Err("the original insurance parameters are a Class value".into());
    };
    assert!(
        matches!(field(params.fields(), "shipEntityClassName")?, Raw::String(value) if *value == case.insured_class),
        "the original insurance class retains empty names"
    );
    for (name, expected) in [
        ("baseWaitTimeMinutes", case.wait),
        ("mandatoryWaitTimeMinutes", case.mandatory),
    ] {
        assert!(
            matches!(field(params.fields(), name)?, Raw::Single(value) if value.to_bits() == expected.to_bits()),
            "{name} retains original IEEE bits"
        );
    }
    assert!(
        matches!(field(params.fields(), "baseExpeditingFee")?, Raw::UInt32(value) if *value == case.fee),
        "the original fee retains its unsigned value"
    );
    Ok(())
}

fn field<'read, 'source>(
    fields: &'read [Field<'source>],
    name: &str,
) -> TestResult<&'read Raw<'source>> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(Field::value)
        .ok_or_else(|| format!("original field {name}").into())
}

fn selected<'read, 'source>(
    fields: &'read [Field<'source>],
    array: &str,
    kind: &str,
) -> TestResult<&'read StructValue<'source>> {
    let Raw::Array(values) = field(fields, array)? else {
        return Err(format!("original {array} array").into());
    };
    values
        .iter()
        .find_map(|value| match value {
            Raw::Struct(value) if value.type_name() == kind => Some(value),
            _ => None,
        })
        .ok_or_else(|| format!("original {kind} declaration").into())
}

fn check_insurance(catalog: &Value, row: &Value, owner: u32, case: &Case) -> TestResult {
    let insurance = proof::cell(row, "insurance", "present")?;
    body_owner(catalog, insurance, owner)?;
    let insurance = &insurance["value"];
    proof::present(insurance, "staticElementOrdinal", &json!(0), Some(true))?;
    let restore = proof::present(
        insurance,
        "restoreOnClaim",
        &json!(case.restore),
        Some(true),
    )?;
    body_owner(catalog, restore, owner)?;
    let params = proof::cell(insurance, "shipInsuranceParams", "present")?;
    let params = &params["value"];
    for (name, expected) in [
        ("shipEntityClassName", json!(case.insured_class)),
        ("baseWaitTimeMinutes", proof::single_number(case.wait)?),
        (
            "mandatoryWaitTimeMinutes",
            proof::single_number(case.mandatory)?,
        ),
        ("expediteFee", json!({"type": "uint32", "value": case.fee})),
    ] {
        let value = proof::present(params, name, &expected, Some(true))?;
        body_owner(catalog, value, owner)?;
    }
    Ok(())
}

fn body_owner(catalog: &Value, cell: &Value, owner: u32) -> TestResult {
    let owner = owner.to_string();
    let uses = proof::uses(catalog, cell)?;
    let uses: Vec<_> = uses
        .iter()
        .filter(|used| used.pointer("/location/namespace") == Some(&json!("dataforgeValue")))
        .collect();
    assert!(
        !uses.is_empty(),
        "the authored value retains actual body evidence"
    );
    assert!(
        uses.iter().all(
            |used| used.pointer("/location/indices/0").and_then(Value::as_str)
                == Some(owner.as_str())
        ),
        "same-GUID owners cannot share source body evidence"
    );
    Ok(())
}

#[path = "catalog_ship_source_physical_sdk.rs"]
mod source_sdk;
