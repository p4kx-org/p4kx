// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, Value as Raw, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::json;

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{Input, TestResult};
use crate::health_reference_assertions as references;
use crate::health_reference_errors as errors;
use crate::health_reference_fixture as fixture;
use crate::health_reference_origins as origins;

#[derive(Clone, Copy)]
enum Target {
    Record,
    Instance,
}

fn database(target: Target) -> Vec<u8> {
    let (structure, declared) = match target {
        Target::Record => (0_u32, fixture::TYPE),
        Target::Instance => (1, "ReferenceLeaf"),
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(fixture::TYPE)
                .field("Code", TestType::String)
                .field("Owned", TestType::StrongPointer("ReferenceLeaf".to_owned()))
                .field("Weak", TestType::WeakPointer(declared.to_owned())),
        )
        .structure(TestStruct::new("ReferenceLeaf").field("Probe", TestType::UInt32))
        .record(
            TestRecord::new(
                fixture::TARGET,
                fixture::TYPE,
                "SCItemManufacturer.Target",
                format!("{}.xml", fixture::target_path()),
            )
            .value("Code", TestValue::text("TARGET"))
            .value(
                "Owned",
                TestValue::Struct(
                    TestInstance::new("ReferenceLeaf").value("Probe", TestValue::UInt32(91)),
                ),
            ),
        )
        .record(
            TestRecord::new(
                fixture::OWNER,
                fixture::TYPE,
                "SCItemManufacturer.Probe",
                format!("{}.xml", fixture::OWNER_PATH),
            )
            .value("Code", TestValue::text("OWNER"))
            .value(
                "Weak",
                TestValue::Raw([structure.to_le_bytes(), 0_u32.to_le_bytes()].concat()),
            ),
        )
        .build()
}

#[test]
fn health_reference_pointer_derived_guid_keeps_header_selector_and_conversion_instead_of_authored_payload()
-> TestResult {
    let bytes = database(Target::Record);
    record_source(&bytes)?;
    let catalog = fixture::document(&bytes, Input::Encrypted)?;
    let rows = references::rows(&catalog)?;
    assert_eq!(rows.len(), 1, "one original weak-to-Record occurrence");
    let row = rows.first().ok_or("actual weak Reference row")?;
    assert_eq!(
        row.get("status"),
        Some(&json!("excludedByFilter")),
        "the genuine weak Record target is filtered"
    );
    let guid = check::present(
        row,
        "targetGuid",
        &json!(crate::health_origin_fixture::guid(fixture::TARGET)),
        Some(true),
    )?;
    origins::exact(
        &catalog,
        guid,
        &[
            origins::header(0, "guid", "payload"),
            origins::body(2, "$value", "selector"),
            origins::body(2, "$value", "conversion"),
        ],
    )?;
    check::present_text(
        &catalog,
        row,
        "targetRecordPath",
        &json!(fixture::target_path()),
        Some(true),
    )?;
    check::closure(&catalog)
}

fn record_source(bytes: &[u8]) -> TestResult {
    let forge = DataForge::open(bytes)?;
    let owner = forge.record(1).ok_or("actual weak owner")?;
    let tree = owner.tree()?;
    let Some(Raw::Reference(reference)) = tree.field("Weak") else {
        return Err("actual weak-to-Record Reference".into());
    };
    assert_eq!(
        reference.record_index(),
        0,
        "the actual weak pointer selects a different original Record"
    );
    assert!(
        reference.location().is_none(),
        "the decoder selected a real Record root"
    );
    assert!(
        reference.original_target().shares_source_with(&owner),
        "actual admitted target shares this Forge"
    );
    let field = tree
        .root()
        .fields()
        .iter()
        .find(|field| field.name() == "Weak")
        .ok_or("actual weak declaration")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::WeakPointer,
        "no authored GUID storage is invented"
    );
    Ok(())
}

#[test]
fn health_reference_outside_owner_retains_actual_pointer_error_without_fabricated_target_metadata()
-> TestResult {
    let bytes = database(Target::Instance);
    let forge = DataForge::open(&bytes)?;
    let target = forge.record(0).ok_or("actual owning target Record")?;
    let target_tree = target.tree()?;
    let Some(Raw::Struct(external)) = target_tree.field("Owned") else {
        return Err("actual externally owned instance".into());
    };
    assert!(
        matches!(external.field("Probe"), Some(Raw::UInt32(91))),
        "the referenced external instance genuinely exists"
    );
    let owner = forge.record(1).ok_or("actual separate weak owner")?;
    let tree = owner.tree()?;
    assert!(
        matches!(
            tree.field("Weak"),
            Some(Raw::Error(ValueError::OutsideRecord {
                struct_index: 1,
                instance_index: 0
            }))
        ),
        "the real canonical decoder refuses the external owned instance with exact original words"
    );
    let catalog = fixture::document(&bytes, Input::Encrypted)?;
    errors::pointer(&catalog, "outsideOwner", (1, 0), 2, Some(true))?;
    check::closure(&catalog)
}
