// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{HEALTH, HEALTH_ONLY, database_with_health, item};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const RESISTANCE_BASE: &str = "DamageResistanceBase";
const RESISTANCE_REFERENCE: &str = "DamageResistanceReference";
const MACRO_TYPE: &str = "DamageResistanceMacro";
const MACRO_PATH: &str = "libs/foundry/records/fixture/resistance_macro.xml";
const MACRO: u128 = 0x7320;
const MISSING_MACRO: u128 = 0x7321;

#[test]
fn durability_resistance_reference_keeps_the_original_macro_link() -> TestResult {
    let catalog = catalog()?;
    let row = proof::at(&catalog, "/configurations/durability/rows/0")?;
    let resistance = reference(row, 1)?;
    let link = proof::cell(resistance, "damageResistanceRecord", "present")?;
    let target = proof::at(link, "/value")?;
    assert_eq!(proof::at(target, "/status")?, "present", "admitted macro");
    proof::present(target, "targetRecordIndex", &json!(1), Some(true))?;
    proof::present(target, "guid", &json!(common::guid(MACRO)), Some(true))?;
    proof::present(
        target,
        "recordPath",
        &json!(MACRO_PATH.trim_end_matches(".xml")),
        Some(true),
    )?;
    proof::present(target, "targetType", &json!(MACRO_TYPE), Some(true))?;
    assert!(
        proof::uses(&catalog, link)?.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("selector")
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && used.pointer("/location/indices/0").and_then(Value::as_str) == Some("0")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
        }),
        "the macro choice retains the actual calling item's Reference"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    proof::closure(&catalog)
}

#[test]
fn durability_resistance_reference_keeps_dangling_guid_and_null_distinct() -> TestResult {
    let catalog = catalog()?;
    let row = proof::at(&catalog, "/configurations/durability/rows/1")?;
    let resistance = reference(row, 2)?;
    let target = proof::at(resistance, "/damageResistanceRecord/value")?;
    assert_eq!(proof::at(target, "/status")?, "dangling", "missing macro");
    let guid = proof::present(
        target,
        "guid",
        &json!(common::guid(MISSING_MACRO)),
        Some(true),
    )?;
    assert!(
        proof::uses(&catalog, guid)?.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("payload")
                && used.pointer("/location/indices/0").and_then(Value::as_str) == Some("0")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$error.guid")
        }),
        "the unresolved GUID is the authored caller error parameter"
    );
    proof::cell(target, "targetRecordIndex", "absent")?;
    proof::cell(target, "recordPath", "absent")?;
    proof::cell(target, "targetType", "absent")?;
    let row = proof::at(&catalog, "/configurations/durability/rows/2")?;
    proof::cell(reference(row, 3)?, "damageResistanceRecord", "null")?;
    Ok(())
}

fn reference(row: &Value, ordinal: u32) -> TestResult<&Value> {
    proof::present(row, "componentOrdinal", &json!(ordinal), Some(true))?;
    let values = proof::at(row, "/damageResistances/value")?;
    assert_eq!(
        proof::at(values, "/type")?,
        "reference",
        "the actual concrete Resistance branch"
    );
    Ok(values)
}

fn catalog() -> TestResult<Value> {
    let (_folder, archive) = common::open(&database(), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn database() -> DataForgeBuilder {
    database_with_health(
        TestStruct::new(HEALTH)
            .parent("DataForgeComponentParams")
            .field(
                "DamageResistances",
                TestType::StrongPointer(RESISTANCE_BASE.to_owned()),
            ),
    )
    .structure(TestStruct::new(RESISTANCE_BASE))
    .structure(
        TestStruct::new(RESISTANCE_REFERENCE)
            .parent(RESISTANCE_BASE)
            .field(
                "damageResistanceRecord",
                TestType::ReferenceTo(MACRO_TYPE.to_owned()),
            ),
    )
    .structure(TestStruct::new(MACRO_TYPE))
    .record(item(
        HEALTH_ONLY,
        "durability_resistance_references",
        "Misc",
        vec![
            component(TestValue::Reference(MACRO)),
            component(TestValue::Reference(MISSING_MACRO)),
            component(TestValue::Null),
        ],
    ))
    .record(TestRecord::new(
        MACRO,
        MACRO_TYPE,
        "DamageResistanceMacro.Fixture",
        MACRO_PATH,
    ))
}

fn component(target: TestValue) -> TestValue {
    let reference = TestInstance::new(RESISTANCE_REFERENCE).value("damageResistanceRecord", target);
    TestValue::Struct(
        TestInstance::new(HEALTH).value("DamageResistances", TestValue::Struct(reference)),
    )
}
