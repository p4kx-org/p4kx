// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, Field, Value as Raw, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::json;

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{Input, TestResult};
use crate::health_reference_assertions as references;
use crate::health_reference_fixture as fixture;
use crate::health_reference_origins as origins;

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(fixture::TYPE)
                .field("Code", TestType::String)
                .field("First", TestType::StrongPointer("ReferenceLeaf".to_owned()))
                .array("Classes", TestType::Class("ReferenceLeaf".to_owned())),
        )
        .structure(TestStruct::new("ReferenceLeaf").field("Probe", TestType::UInt32))
        .record(
            TestRecord::new(
                fixture::TARGET,
                fixture::TYPE,
                "SCItemManufacturer.Target",
                "synthetic/other.xml",
            )
            .value("Code", TestValue::text("TARGET")),
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
                "First",
                TestValue::Struct(
                    TestInstance::new("ReferenceLeaf").value("Probe", TestValue::UInt32(91)),
                ),
            )
            .value(
                "Classes",
                TestValue::Raw([1_u32.to_le_bytes(), 0_u32.to_le_bytes()].concat()),
            ),
        )
        .build()
}

#[test]
fn health_reference_class_array_error_uses_original_declaration_and_index_conversion() -> TestResult
{
    let bytes = database();
    original(&bytes)?;
    let catalog = fixture::document(&bytes, Input::Encrypted)?;
    let row = references::rows(&catalog)?
        .iter()
        .find(|row| row.get("status") == Some(&json!("ownedTwice")))
        .ok_or("actual class-array ownership error")?;
    let pointer = check::at(row, "/sourcePointer")?;
    let structure = check::present(pointer, "structIndex", &json!(1), Some(true))?;
    origins::exact(
        &catalog,
        structure,
        &[origins::body(3, "$target", "selector")],
    )?;
    let instance = check::present(pointer, "instanceIndex", &json!(0), Some(true))?;
    origins::exact(
        &catalog,
        instance,
        &[origins::body(4, "$error.instanceIndex", "conversion")],
    )?;
    check::present_text(&catalog, row, "field", &json!("Classes[0]"), Some(true))?;
    references::absent(pointer, "declaredTypeIndex")?;
    check::closure(&catalog)
}

fn original(bytes: &[u8]) -> TestResult {
    let forge = DataForge::open(bytes)?;
    let owner = forge.record(1).ok_or("actual original class-array owner")?;
    let tree = owner.tree()?;
    let fields: Vec<_> = tree.root().fields().iter().map(Field::name).collect();
    assert_eq!(
        fields,
        ["Code", "First", "Classes"],
        "the genuine declaration and original DFS order"
    );
    let Some(Raw::Struct(first)) = tree.field("First") else {
        return Err("actual first owning pointer".into());
    };
    assert!(
        matches!(first.field("Probe"), Some(Raw::UInt32(91))),
        "the exact original instance exists and is owned first"
    );
    let field = tree
        .root()
        .fields()
        .iter()
        .find(|field| field.name() == "Classes")
        .ok_or("actual class-array declaration")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::Class,
        "the original source has no per-element struct-index word"
    );
    assert!(field.storage().is_array(), "actual Class-array storage");
    assert_eq!(
        field.storage().target(),
        1,
        "the original schema target is exactly the error's struct index"
    );
    let Some(Raw::Array(values)) = tree.field("Classes") else {
        return Err("actual original Class array".into());
    };
    assert_eq!(
        values.len(),
        1,
        "the original count/first header reaches its one physical slot"
    );
    assert!(
        matches!(
            values.first(),
            Some(Raw::Error(ValueError::OwnedTwice {
                struct_index: 1,
                instance_index: 0
            }))
        ),
        "the actual canonical reader retains declaration-selected struct and computed first-plus-position"
    );
    Ok(())
}
