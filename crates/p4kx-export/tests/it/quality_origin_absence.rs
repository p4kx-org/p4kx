// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Filter;
use p4kx_extraction::{DataForge, DataForgeEntry, StructValue, Value as Raw};
use p4kx_testkit::{TestInstance, TestValue};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::quality_v3_fixture_schema as schema;
use crate::quality_v3_fixture_values as values;
use crate::quality_v3_support as support;

const WRAPPER: &str = "qualityQuantization";
const REFERENCE: &str = "qualityQuantizationRecord";
const ABSENT_PATH: &str = "properties[0].qualityQuantization.qualityQuantizationRecord";
const NULL_PATH: &str = "properties[1].qualityQuantization.qualityQuantizationRecord";
const TYPE_PATH: &str = "properties[0].qualityQuantization.$type";

#[test]
fn inverse_absence_uses_the_actual_owning_declaration_survey() -> TestResult {
    let base = TestValue::Struct(
        TestInstance::new("ResourceTypeCraftingData")
            .value("name", TestValue::text("knownEmptyBase"))
            .value(
                WRAPPER,
                TestValue::Struct(TestInstance::new("CraftingQualityQuantization_Base")),
            ),
    );
    let database = schema::schema()
        .record(values::quantization(
            values::QUANTIZATION,
            "Quantizer",
            "quantizer",
            values::bands(vec![values::band(0, 9, 3)]),
        ))
        .record(values::resource(
            values::RESOURCE_A,
            "resource",
            vec![
                base,
                values::property(None),
                values::property(Some(values::QUANTIZATION)),
            ],
        ));
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let (record, owner) = original(&forge)?;
    let filter = Filter::new("*/quantizer");
    let mut options = common::options();
    options.filter = Some(&filter);
    let built = p4kx_export::build_catalog_from(&archive, &entry, &options)?;
    assert!(built.summary().is_complete());
    let catalog: Value = serde_json::from_slice(built.json())?;
    let inverse = support::row(&catalog, support::INVERSE, 0)?;
    let list = support::at(inverse, "/resourceTypeGuids")?;
    let members = list["value"].as_array().ok_or("actual inverse members")?;
    let [member] = members.as_slice() else {
        return Err("one actual positive source".into());
    };
    assert_eq!(member["value"], common::guid(values::RESOURCE_A));
    assert_eq!(list["sourceSetComplete"], true);
    support::marker(list, Some(true))?;
    assert_origins(&catalog, list, record, owner)
}

fn original(forge: &DataForge<'_>) -> TestResult<(u32, u64)> {
    let record = forge.record(1).ok_or("actual ResourceType Record")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the declaration survey belongs to the actual Resource Record"
    );
    assert_eq!(
        tree.root().fields().len(),
        1,
        "properties is the only original root slot"
    );
    let Some(Raw::Array(properties)) = tree.root().field("properties") else {
        return Err("original properties".into());
    };
    let [Raw::Struct(first), Raw::Struct(second), Raw::Struct(third)] = properties.as_ref() else {
        return Err("three actual ordered properties".into());
    };
    absent_reference_slot(first)?;
    let null = structure(second)?;
    assert_eq!(
        null.type_name(),
        "CraftingQualityQuantization_RecordRef",
        "the null belongs to a concrete reference declaration"
    );
    assert_eq!(
        null.field(REFERENCE),
        Some(&Raw::Null),
        "the second Struct declares an explicit Null slot"
    );
    let Some(Raw::Reference(target)) = structure(third)?.field(REFERENCE) else {
        return Err("the positive actual Reference".into());
    };
    assert_eq!(
        target.record_index(),
        0,
        "the third authored Reference creates the actual member"
    );
    let wrapper = first
        .fields()
        .iter()
        .position(|field| field.name() == WRAPPER)
        .ok_or("original wrapper declaration")?;
    for field in first.fields().iter().take(wrapper) {
        assert!(
            !matches!(field.value(), Raw::Struct(_) | Raw::Array(_)),
            "all original fields preceding the wrapper occupy one source occurrence"
        );
    }
    // The root array and its first element precede this element's fields.
    Ok((record.index(), u64::try_from(wrapper)? + 2))
}

fn absent_reference_slot(first: &StructValue<'_>) -> TestResult {
    let absent = structure(first)?;
    assert_eq!(
        absent.type_name(),
        "CraftingQualityQuantization_Base",
        "the original absence belongs to the base Struct"
    );
    assert!(
        absent.fields().is_empty(),
        "the selected Struct declares no Reference field"
    );
    assert_eq!(
        absent.field(REFERENCE),
        None,
        "the original base Struct has no declared reference field"
    );
    Ok(())
}

fn structure<'read, 'source>(
    value: &'read StructValue<'source>,
) -> TestResult<&'read StructValue<'source>> {
    match value.field(WRAPPER) {
        Some(Raw::Struct(wrapper)) => Ok(wrapper),
        _ => Err("actual original quantization Struct".into()),
    }
}

fn assert_origins(catalog: &Value, list: &Value, record: u32, owner: u64) -> TestResult {
    let references = list["sourceUsesRefs"]
        .as_array()
        .ok_or("actual immediate source uses")?;
    let inventory = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("actual source inventory")?;
    let record_text = record.to_string();
    let owner_text = owner.to_string();
    let mut declared_null = false;
    let mut concrete = false;
    let mut survey = false;
    let mut nonexistent = false;
    for used in inventory
        .iter()
        .filter(|used| references.contains(&used["id"]))
    {
        assert_eq!(
            used["scope"], "payload",
            "the actual owner declaration is observed in source payload scope"
        );
        let location = &used["location"];
        if location["recordIndex"] == record {
            match location["field"].as_str() {
                Some(ABSENT_PATH) => nonexistent = true,
                Some(NULL_PATH) => declared_null = used["role"] == "selector",
                Some(TYPE_PATH) => concrete = used["role"] == "selector",
                _ => {}
            }
        }
        if location["namespace"] == "dataforgeValue"
            && location.pointer("/indices/0").and_then(Value::as_str) == Some(record_text.as_str())
            && location.pointer("/indices/1").and_then(Value::as_str) == Some(owner_text.as_str())
            && location["field"] == "$fields"
        {
            survey = used["role"] == "selector";
        }
    }
    assert!(
        declared_null,
        "the authored Null keeps its actual exact Reference slot"
    );
    assert!(
        concrete,
        "the actual readable Struct type retains its distinct selector"
    );
    assert!(
        !nonexistent,
        "absence cannot invent an original field occurrence"
    );
    assert!(
        survey,
        "the actual owning Struct's complete declaration survey proves absence"
    );
    Ok(())
}
