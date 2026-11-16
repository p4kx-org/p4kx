// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter, build_catalog, catalog_schema};
use p4kx_extraction::{DataForge, Value as Raw};
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
const PURCHASE_NAME: &str = "@later_name";
const MAKER: &str = "SCItemManufacturer";
const ROWS: &str = "/definitions/items/rows";
const CONSTRAINTS: &str = "/configurations/itemAttachmentConstraints/rows";

#[test]
fn item_definition_keeps_duplicate_guid_and_each_authored_component_slot() -> TestResult {
    let (_, catalog) = manufacturer_v3_fixture::build(
        &database(true, Some(PURCHASE_NAME)),
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
    let text = common::text_row(&catalog, "gameTexts", display)?;
    proof::present(text, "key", &json!("@itemDisplay"), Some(true))?;
    let name = proof::at(purchase, "/value/displayName/value")?;
    let text = common::text_row(&catalog, "publicNames", name)?;
    proof::present(text, "key", &json!(PURCHASE_NAME), Some(true))?;
    Ok(())
}

#[test]
fn filtered_item_definitions_exclude_duplicate_siblings_and_dependencies() -> TestResult {
    let filter = Filter::new("included");
    let (_, catalog) = manufacturer_v3_fixture::build(
        &database(false, None),
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        Some(&filter),
    )?;
    let [row] = proof::rows(&catalog, ROWS)? else {
        return Err("one physical selected item remains".into());
    };
    proof::present(row, "recordIndex", &json!(0), Some(false))?;
    let purchase = proof::at(row, "/components/value/3/value")?;
    proof::cell(purchase, "displayName", "absent")?;
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

#[test]
fn purchase_only_names_preserve_order_empty_keys_and_language_origins() -> TestResult {
    let bytes = structured_database(true)
        .record(item_record(
            "purchase_only",
            "purchase_only",
            vec![purchase(Some(PURCHASE_NAME)), purchase(Some(""))],
        ))
        .build();
    let ordinals = original_purchase_names(&bytes)?;
    let (_folder, archive) = common::open_bytes(bytes, DcbOrigin::Plain)?;
    let built = build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        proof::at(&catalog, "/schemaVersion")?,
        3,
        "the purchase name is additive to Catalog v3"
    );
    jsonschema::validator_for(&catalog_schema())?
        .validate(&catalog)
        .map_err(|error| format!("item purchase name catalog schema: {error}"))?;

    let [definition] = proof::rows(&catalog, ROWS)? else {
        return Err("one item definition with purchase-only components".into());
    };
    let [first, second] = proof::rows(definition, "/components/value")? else {
        return Err("both authored purchase slots".into());
    };
    for (slot, ((component, ordinal), key)) in [first, second]
        .into_iter()
        .zip(ordinals)
        .zip([PURCHASE_NAME, ""])
        .enumerate()
    {
        proof::present(component, "componentOrdinal", &json!(slot), Some(false))?;
        let purchase = proof::at(component, "/value")?;
        let name = proof::cell(purchase, "displayName", "present")?;
        check_purchase_name_source(&catalog, name, ordinal)?;
        let text = common::text_row(&catalog, "publicNames", proof::at(name, "/value")?)?;
        proof::present(text, "key", &json!(key), Some(false))?;
        if !key.is_empty() {
            check_purchase_translations(text)?;
        }
    }
    assert_eq!(
        proof::at(&catalog, "/items/0/playability")?,
        "player",
        "the first purchase name still decides playability without attachable localization"
    );
    proof::closure(&catalog)
}

fn original_purchase_names(bytes: &[u8]) -> TestResult<[usize; 2]> {
    let forge = DataForge::open(bytes)?;
    let tree = forge.record(0).ok_or("physical purchase Record")?.tree()?;
    let mut ordinals = Vec::new();
    tree.root().visit_owned_values(|visit| {
        if [Raw::Locale(PURCHASE_NAME), Raw::Locale("")].contains(visit.value()) {
            ordinals.push(visit.ordinal());
        }
    });
    let [first, second] = ordinals.as_slice() else {
        return Err("the exact two original authored name occurrences".into());
    };
    Ok([*first, *second])
}

fn check_purchase_name_source(catalog: &Value, name: &Value, ordinal: usize) -> TestResult {
    assert_eq!(
        proof::at(name, "/containsEncryptedSource")?,
        false,
        "only the unencrypted DataForge field contributes to the purchase reference"
    );
    assert_eq!(
        proof::at(name, "/sourceSetComplete")?,
        true,
        "the exact name selection is complete"
    );
    let conversions: Vec<_> = proof::uses(catalog, name)?
        .into_iter()
        .filter(|used| used.get("role").and_then(Value::as_str) == Some("conversion"))
        .collect();
    let [conversion] = conversions.as_slice() else {
        return Err("one original Locale conversion".into());
    };
    assert_eq!(
        proof::at(conversion, "/location")?,
        &json!({"type": "subresource", "namespace": "dataforgeValue", "indices": ["0", &ordinal.to_string()], "field": "$value"}),
        "the reference uses its own physical component's Locale"
    );
    assert_eq!(
        proof::at(proof::source(catalog, conversion)?, "/path/value")?,
        "Data\\Game2.dcb",
        "the Locale reference names its actual DataForge Entry"
    );
    Ok(())
}

fn check_purchase_translations(text: &Value) -> TestResult {
    let english = proof::at(text, "/translations/value/en")?;
    for field in ["sourceText", "displayText"] {
        proof::present(english, field, &json!("Later name"), Some(true))?;
    }
    let german = proof::at(text, "/translations/value/de")?;
    proof::cell(german, "sourceText", "absent")?;
    proof::cell(german, "displayText", "absent")?;
    assert!(
        text.pointer("/translations/value/fr").is_none(),
        "an unavailable language has no translation member"
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

fn database(shared_path: bool, purchase_name: Option<&str>) -> DataForgeBuilder {
    structured_database(purchase_name.is_some())
        .record(item("included", "included", 0, 2, purchase_name))
        .record(item(
            "excluded",
            if shared_path { "included" } else { "excluded" },
            3,
            4,
            purchase_name,
        ))
        .record(TestRecord::new(
            GUID + 1,
            MAKER,
            "SCItemManufacturer.supplier",
            "libs/foundry/records/scitemmanufacturer/fixture/supplier.xml",
        ))
}

fn structured_database(has_purchase_name: bool) -> DataForgeBuilder {
    structures(has_purchase_name).into_iter().fold(
        DataForgeBuilder::new().version(8),
        DataForgeBuilder::structure,
    )
}

fn structures(has_purchase_name: bool) -> [TestStruct; 9] {
    let purchase = TestStruct::new(PURCHASE)
        .parent(COMPONENT)
        .field("displayType", TestType::Locale);
    let purchase = if has_purchase_name {
        purchase.field("displayName", TestType::Locale)
    } else {
        purchase
    };
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
        purchase,
        TestStruct::new(MAKER),
    ]
}

fn item(
    name: &str,
    path: &str,
    first: i32,
    second: i32,
    purchase_name: Option<&str>,
) -> TestRecord {
    item_record(
        name,
        path,
        vec![
            TestValue::Null,
            attachment(first),
            structure(
                PHYSICS,
                "PhysType",
                structure(RIGID, "Mass", TestValue::Single(-0.0)),
            ),
            attachment(second),
            purchase(purchase_name),
        ],
    )
}

fn item_record(name: &str, path: &str, components: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        GUID,
        OWNER,
        format!("{OWNER}.{name}"),
        format!("libs/foundry/records/entities/scitem/fixture/{path}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn purchase(name: Option<&str>) -> TestValue {
    let mut component =
        TestInstance::new(PURCHASE).value("displayType", TestValue::text("@itemDisplay"));
    if let Some(name) = name {
        component = component.value("displayName", TestValue::text(name));
    }
    TestValue::Struct(component)
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
