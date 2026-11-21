// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{Fixture, TestResult, replace_value};
use crate::catalog_v3_consumer;
use crate::real_archive_canonical_texts::ItemPresentationIndex;

const ENTITY: &str = "EntityClassDefinition";
const BASE: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SAttachableComponentParams";
const DEFINITION: &str = "SItemDefinition";
const PHYSICS: &str = "SEntityPhysicsControllerParams";
const RIGID: &str = "SEntityRigidPhysicsControllerParams";
const MANUFACTURER: &str = "SCItemManufacturer";

#[test]
fn existing_installed_readers_follow_actual_original_source_families() -> TestResult {
    let fixture = Fixture::from_database(&database())?;
    let index = ItemPresentationIndex::new(&fixture.catalog)?;
    let [item] = catalog_v3_consumer::rows(&fixture.catalog, Kind::Items)?.as_slice() else {
        return Err("one actual synthetic producer Item".into());
    };
    assert_eq!(
        index.item_type(item)?,
        Some("First"),
        "first physical Attachment"
    );
    assert_eq!(
        index.grade(item)?,
        Some(9),
        "the actual integer is not clipped to 1..7"
    );
    assert_eq!(
        index.mass(item)?,
        Some(&json!(125.5)),
        "original finite mass"
    );
    assert_eq!(
        index.manufacturer(item)?,
        Some(crate::commodity_v3_fixture::guid(2).as_str()),
        "the authored Reference keeps its nominal target"
    );
    let mut bad = item.clone();
    replace_value(
        &mut bad,
        "/selection/attachmentComponentOrdinal/value",
        json!(999),
    )?;
    assert!(
        index.item_type(&bad).is_err(),
        "an absent physical selected slot never falls back to another component"
    );
    Ok(())
}

#[test]
fn an_original_null_first_payload_never_uses_a_later_attachment() -> TestResult {
    let fixture = Fixture::from_database(&database())?;
    let mut document = fixture.catalog;
    let path = "/definitions/items/rows/0/components/value/0/value";
    replace_value(&mut document, &format!("{path}/state"), json!("null"))?;
    replace_value(&mut document, &format!("{path}/value"), Value::Null)?;
    let index = ItemPresentationIndex::new(&document)?;
    let [item] = catalog_v3_consumer::rows(&document, Kind::Items)?.as_slice() else {
        return Err("one selected physical source Item".into());
    };
    assert_eq!(index.item_type(item)?, None, "null is not a later type");
    assert_eq!(index.grade(item)?, None, "null is not a later grade");
    assert_eq!(
        index.manufacturer(item)?,
        None,
        "null is not a later Reference"
    );
    assert!(
        document.pointer("/definitions/items/rows/0/components/value/1/value/value/itemType/value")
            == Some(&json!("Later")),
        "an independently readable later source exists"
    );
    Ok(())
}

fn database() -> DataForgeBuilder {
    declarations()
        .record(item_record())
        .record(manufacturer_record())
}

fn declarations() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(ENTITY).array("Components", TestType::StrongPointer(BASE.into())),
        )
        .structure(TestStruct::new(BASE))
        .structure(
            TestStruct::new(ATTACHMENT)
                .parent(BASE)
                .field("AttachDef", TestType::Class(DEFINITION.into())),
        )
        .structure(
            TestStruct::new(DEFINITION)
                .field("Type", TestType::String)
                .field("Grade", TestType::Int64)
                .field("Manufacturer", TestType::ReferenceTo(MANUFACTURER.into())),
        )
        .structure(
            TestStruct::new(PHYSICS)
                .parent(BASE)
                .field("PhysType", TestType::Class(RIGID.into())),
        )
        .structure(TestStruct::new(RIGID).field("Mass", TestType::Single))
        .structure(TestStruct::new(MANUFACTURER).field("Code", TestType::String))
}

fn item_record() -> TestRecord {
    TestRecord::new(
        1,
        ENTITY,
        "EntityClassDefinition.SourceItem",
        "libs/foundry/records/entities/scitem/fixture/source_item.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![
            attachment("First", 9),
            attachment("Later", 2),
            physics(),
        ]),
    )
}

fn manufacturer_record() -> TestRecord {
    TestRecord::new(
        2,
        MANUFACTURER,
        "SCItemManufacturer.Source",
        "libs/foundry/records/scitemmanufacturer/fixture/source.xml",
    )
    .value("Code", TestValue::text("Fixture"))
}

fn attachment(label: &str, grade: i64) -> TestValue {
    let definition = TestInstance::new(DEFINITION)
        .value("Type", TestValue::text(label))
        .value("Grade", TestValue::Int64(grade))
        .value("Manufacturer", TestValue::Reference(2));
    TestValue::Struct(
        TestInstance::new(ATTACHMENT).value("AttachDef", TestValue::Struct(definition)),
    )
}

fn physics() -> TestValue {
    TestValue::Struct(TestInstance::new(PHYSICS).value(
        "PhysType",
        TestValue::Struct(TestInstance::new(RIGID).value("Mass", TestValue::Single(125.5))),
    ))
}
