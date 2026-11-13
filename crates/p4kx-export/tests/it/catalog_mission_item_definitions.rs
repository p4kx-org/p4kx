// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

const ITEM: u128 = 0x1075_0001;
const ENTITY: u128 = 0x1075_0002;
const TAG: u128 = 0x1075_0003;
const MISSING: u128 = 0x1075_0004;
const ROW: &str = "/definitions/missionItems/rows/0";

#[test]
fn mission_item_keeps_ordered_source_variants_weight_and_references() -> TestResult {
    let catalog = export(None)?;
    let row = at(&catalog, ROW)?;
    for (path, expected) in [
        ("/guid/value", json!(guid(ITEM))),
        ("/entityClass/value/guid/value", json!(guid(ENTITY))),
        ("/weighting/value/rawBits", json!("80000000")),
        ("/tags/value/0/value/guid/value", json!(guid(TAG))),
        ("/tags/value/1/state", json!("null")),
        ("/tags/value/2/value/guid/value", json!(guid(MISSING))),
        ("/tags/value/2/value/status", json!("dangling")),
    ] {
        assert_eq!(at(row, path)?, &expected, "original value at {path}");
    }
    let variants = at(row, "/stringVariants/value/variants/value")?
        .as_array()
        .ok_or("authored variant list")?;
    let [first, second] = variants.as_slice() else {
        return Err("both authored variant slots must be present".into());
    };
    assert_eq!(
        at(first, "/value/tag/value/guid/value")?,
        &json!(guid(TAG)),
        "first variant tag"
    );
    assert_eq!(
        at(second, "/value/tag/state")?,
        "null",
        "second variant tag state"
    );
    verify_text(&catalog, first, "FixtureMissionItemFirst")?;
    verify_text(&catalog, second, "FixtureMissionItemSecond")?;
    jsonschema::validator_for(&p4kx_export::catalog_schema())?
        .validate(&catalog)
        .map_err(|error| format!("catalog schema: {error}"))?;
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

#[test]
fn mission_item_entity_target_remains_when_its_catalog_row_is_filtered() -> TestResult {
    let filter = Filter::new("*/mission_fixture/item");
    let catalog = export(Some(&filter))?;
    let row = at(&catalog, ROW)?;
    assert_eq!(
        at(row, "/entityClass/value/guid/value")?,
        &json!(guid(ENTITY)),
        "stored entity link survives"
    );
    assert_eq!(
        at(&catalog, "/items")?,
        &json!([]),
        "the target row follows the filter"
    );
    assert_eq!(
        at(row, "/entityClass/containsEncryptedSource")?,
        true,
        "source remains encrypted"
    );
    Ok(())
}

fn verify_text(catalog: &Value, variant: &Value, expected: &str) -> TestResult {
    let cell = at(variant, "/value/string")?;
    let text = common::text_row(catalog, "gameTexts", at(cell, "/value")?)?;
    assert_eq!(
        at(text, "/key/value")?,
        expected,
        "the original Locale key is retained"
    );
    assert_eq!(
        at(cell, "/containsEncryptedSource")?,
        true,
        "the variant owns its original encryption proof"
    );
    Ok(())
}

fn export(filter: Option<&Filter>) -> TestResult<Value> {
    let (_folder, archive) = common::open(&database(), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    options.filter = filter;
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

fn database() -> DataForgeBuilder {
    let reference = |name: &str| TestType::ReferenceTo(name.to_owned());
    let class = |name: &str| TestType::Class(name.to_owned());
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Tag"))
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(
            TestStruct::new("MissionStringVariant")
                .field("tag", reference("Tag"))
                .field("string", TestType::Locale),
        )
        .structure(
            TestStruct::new("MissionStringVariants")
                .array("variants", class("MissionStringVariant")),
        )
        .structure(
            TestStruct::new("MissionItem")
                .array("tags", reference("Tag"))
                .field("stringVariants", class("MissionStringVariants"))
                .field("weighting", TestType::Single)
                .field("entityClass", reference("EntityClassDefinition")),
        )
        .record(TestRecord::new(
            TAG,
            "Tag",
            "Tag.Fixture",
            "libs/foundry/records/tags/fixture.xml",
        ))
        .record(TestRecord::new(
            ENTITY,
            "EntityClassDefinition",
            "EntityClassDefinition.FixtureMissionItem",
            "libs/foundry/records/entities/scitem/fixture/mission_item.xml",
        ))
        .record(item_record())
}

fn item_record() -> TestRecord {
    let variants = [
        ("FixtureMissionItemFirst", TestValue::Reference(TAG)),
        ("FixtureMissionItemSecond", TestValue::Null),
    ]
    .into_iter()
    .map(|(key, tag)| {
        TestValue::Struct(
            TestInstance::new("MissionStringVariant")
                .value("tag", tag)
                .value("string", TestValue::text(key)),
        )
    })
    .collect();
    TestRecord::new(
        ITEM,
        "MissionItem",
        "MissionItem.Fixture",
        "libs/foundry/records/mission_fixture/item.xml",
    )
    .value(
        "tags",
        TestValue::Array(vec![
            TestValue::Reference(TAG),
            TestValue::Null,
            TestValue::Reference(MISSING),
        ]),
    )
    .value(
        "stringVariants",
        TestValue::Struct(
            TestInstance::new("MissionStringVariants")
                .value("variants", TestValue::Array(variants)),
        ),
    )
    .value("weighting", TestValue::Single(-0.0))
    .value("entityClass", TestValue::Reference(ENTITY))
}
