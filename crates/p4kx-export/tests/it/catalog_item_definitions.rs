// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::manufacturer_v3_fixture;

const GUID: u128 = 0x8700;
const OWNER: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SAttachableComponentParams";
const DEFINITION: &str = "SItemDefinition";
const LOCALIZATION: &str = "SCItemLocalization";
const PHYSICS: &str = "SEntityPhysicsControllerParams";
const RIGID: &str = "SEntityRigidPhysicsControllerParams";
const PURCHASE: &str = "SCItemPurchasableParams";
const MAKER: &str = "SCItemManufacturer";
const ROWS: &str = "/definitions/items/rows";
const CONSTRAINTS: &str = "/configurations/itemAttachmentConstraints/rows";

#[test]
fn item_definition_keeps_duplicate_guid_and_each_authored_component_slot() -> TestResult {
    let (_, catalog) = manufacturer_v3_fixture::build(
        &database(true),
        DcbOrigin::Encrypted,
        CatalogPublicationMode::DiagnosticPartial,
        None,
    )?;
    let [a, b] = proof::rows(&catalog, ROWS)? else {
        return Err("distinct selected physical records with the same GUID".into());
    };
    for row in [a, b] {
        proof::present(row, "guid", &json!(common::guid(GUID)), Some(true))?;
    }
    proof::present(a, "recordIndex", &json!(0), Some(true))?;
    proof::present(b, "recordIndex", &json!(1), Some(true))?;
    let [first, second] = proof::rows(&catalog, "/items")? else {
        return Err("distinct Item projections with the same GUID and path".into());
    };
    for original in [a, b] {
        assert_eq!(
            [first, second]
                .into_iter()
                .filter(|row| row.get("recordIndex") == original.get("recordIndex"))
                .count(),
            1,
            "the sourced physical ordinal joins exactly one projection and definition"
        );
    }
    let [first, mass, second, purchase] = proof::rows(a, "/components/value")? else {
        return Err(
            "the original attachment, mass, repeated attachment and purchasable slots".into(),
        );
    };
    for (ordinal, component) in [first, mass, second, purchase].into_iter().enumerate() {
        proof::present(
            component,
            "componentOrdinal",
            &json!(ordinal + 1),
            Some(true),
        )?;
    }
    assert_attachment(&catalog, first, 1, 0)?;
    assert_attachment(&catalog, second, 3, 2)?;
    assert_eq!(
        proof::at(mass, "/value/value/mass/value/rawBits")?,
        "80000000",
        "signed zero mass stays exact"
    );
    let display = proof::at(purchase, "/value/displayType/value")?;
    assert!(
        proof::rows(&catalog, "/texts/gameTexts")?
            .iter()
            .any(|row| {
                row.pointer("/id/value") == Some(display)
                    && row.pointer("/key/value") == Some(&json!("@itemDisplay"))
            }),
        "the purchase text resolves only in the protected domain"
    );
    Ok(())
}

#[test]
fn filtered_item_definitions_exclude_duplicate_siblings_and_dependencies() -> TestResult {
    let filter = Filter::new("included");
    let (_, catalog) = manufacturer_v3_fixture::build(
        &database(false),
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        Some(&filter),
    )?;
    let [row] = proof::rows(&catalog, ROWS)? else {
        return Err("one physical selected item remains".into());
    };
    proof::present(row, "recordIndex", &json!(0), Some(false))?;
    let maker = proof::at(
        row,
        "/components/value/0/value/value/manufacturer/value/value",
    )?;
    assert_eq!(
        proof::at(maker, "/targetRecordIndex/value")?,
        &json!(2),
        "resolved physical manufacturer dependency"
    );
    assert!(
        proof::rows(&catalog, "/definitions/manufacturers/rows")?.is_empty(),
        "manufacturer selection stays independent"
    );
    assert!(
        proof::rows(&catalog, CONSTRAINTS)?
            .iter()
            .all(|item| { item.pointer("/ownerRecordIndex/value") == Some(&json!(0)) }),
        "filter cannot admit the duplicate item's unrelated constraints"
    );
    Ok(())
}

fn assert_attachment(catalog: &Value, row: &Value, ordinal: u32, grade: i32) -> TestResult {
    let attachment = proof::at(row, "/value/value")?;
    proof::present(
        attachment,
        "grade",
        &json!({"type": "int32", "value": grade}),
        Some(true),
    )?;
    let localization = proof::at(attachment, "/localization/value")?;
    for (domain, field, expected) in [
        ("publicNames", "name", "@itemName"),
        ("gameTexts", "description", "@itemDescription"),
    ] {
        let reference = proof::at(localization, &format!("/{field}/value"))?;
        assert!(
            proof::rows(catalog, &format!("/texts/{domain}"))?
                .iter()
                .any(|row| {
                    row.pointer("/id/value") == Some(reference)
                        && row.pointer("/key/value") == Some(&json!(expected))
                }),
            "canonical text reference for {field}"
        );
    }
    assert!(
        attachment.get("type").is_none(),
        "classification remains singular in constraints"
    );
    assert!(
        proof::rows(catalog, CONSTRAINTS)?.iter().any(|row| {
            row.pointer("/ownerRecordIndex/value") == Some(&json!(0))
                && row.pointer("/componentOrdinal/value") == Some(&json!(ordinal))
                && row.pointer("/definition/value/itemType/value") == Some(&json!("FixtureType"))
        }),
        "the authored classification joins by physical record and slot"
    );
    Ok(())
}

fn database(shared_path: bool) -> DataForgeBuilder {
    let builder = DataForgeBuilder::new().version(8);
    let builder = structures()
        .into_iter()
        .fold(builder, DataForgeBuilder::structure);
    builder
        .record(item("included", "included", 0, 2))
        .record(item(
            "excluded",
            if shared_path { "included" } else { "excluded" },
            3,
            4,
        ))
        .record(TestRecord::new(
            GUID + 1,
            MAKER,
            "SCItemManufacturer.supplier",
            "libs/foundry/records/scitemmanufacturer/fixture/supplier.xml",
        ))
}

fn structures() -> [TestStruct; 9] {
    [
        TestStruct::new(OWNER).array("Components", TestType::StrongPointer(COMPONENT.into())),
        TestStruct::new(COMPONENT),
        TestStruct::new(ATTACHMENT)
            .parent(COMPONENT)
            .field("AttachDef", class(DEFINITION)),
        TestStruct::new(DEFINITION)
            .field("Type", TestType::String)
            .field("Size", TestType::Int32)
            .field("Grade", TestType::Int32)
            .field("Manufacturer", TestType::ReferenceTo(MAKER.into()))
            .field("Localization", class(LOCALIZATION)),
        TestStruct::new(LOCALIZATION)
            .field("Name", TestType::Locale)
            .field("Description", TestType::Locale),
        TestStruct::new(PHYSICS)
            .parent(COMPONENT)
            .field("PhysType", class(RIGID)),
        TestStruct::new(RIGID).field("Mass", TestType::Single),
        TestStruct::new(PURCHASE)
            .parent(COMPONENT)
            .field("displayType", TestType::Locale),
        TestStruct::new(MAKER),
    ]
}

fn item(name: &str, path: &str, first: i32, second: i32) -> TestRecord {
    TestRecord::new(
        GUID,
        OWNER,
        format!("{OWNER}.{name}"),
        format!("libs/foundry/records/entities/scitem/fixture/{path}.xml"),
    )
    .value(
        "Components",
        TestValue::Array(vec![
            TestValue::Null,
            attachment(first),
            structure(
                PHYSICS,
                "PhysType",
                structure(RIGID, "Mass", TestValue::Single(-0.0)),
            ),
            attachment(second),
            structure(PURCHASE, "displayType", TestValue::text("@itemDisplay")),
        ]),
    )
}

fn attachment(grade: i32) -> TestValue {
    let localization = TestInstance::new(LOCALIZATION)
        .value("Name", TestValue::text("@itemName"))
        .value("Description", TestValue::text("@itemDescription"));
    let value = TestInstance::new(DEFINITION)
        .value("Type", TestValue::text("FixtureType"))
        .value("Size", TestValue::Int32(3))
        .value("Grade", TestValue::Int32(grade))
        .value("Manufacturer", TestValue::Reference(GUID + 1))
        .value("Localization", TestValue::Struct(localization));
    structure(ATTACHMENT, "AttachDef", TestValue::Struct(value))
}

fn structure(name: &str, field: &str, value: TestValue) -> TestValue {
    TestValue::Struct(TestInstance::new(name).value(field, value))
}

fn class(name: &str) -> TestType {
    TestType::Class(name.into())
}
