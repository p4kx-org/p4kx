// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, CatalogPublicationMode, Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as archive_fixture, DcbOrigin};
pub(crate) use crate::commodity_v3_fixture::{TestResult, at, guid};

pub(crate) const ROOT: u128 = 0x10;
pub(crate) const LEAF: u128 = 0x20;
pub(crate) const OTHER: u128 = 0x30;
pub(crate) const LOOSE: u128 = 0x40;
pub(crate) const DATABASE: u128 = 0x50;
pub(crate) const MISSING: u128 = 0x99;
pub(crate) const WRONG: u128 = 0x60;
pub(crate) const ITEM: u128 = 0x100;
pub(crate) const SHIP: u128 = 0x101;
pub(crate) const COMMODITY: u128 = 0x102;

pub(crate) fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .field("legacyGUID", TestType::UInt32)
                .array("children", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(TestStruct::new("FixtureWrong"))
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
}

pub(crate) fn references(values: &[u128]) -> TestValue {
    TestValue::Array(values.iter().copied().map(TestValue::Reference).collect())
}

pub(crate) fn tag(guid: u128, name: &str, children: TestValue) -> TestRecord {
    TestRecord::new(guid, "Tag", format!("Tag.{name}"), "synthetic/dev/tags.xml")
        .value("tagName", TestValue::text(name))
        .value("legacyGUID", TestValue::UInt32(u32::MAX))
        .value("children", children)
}

pub(crate) fn roots(values: TestValue) -> TestRecord {
    TestRecord::new(
        DATABASE,
        "TagDatabase",
        "TagDatabase.Roots",
        "synthetic/roots.xml",
    )
    .value("tags", values)
}

pub(crate) fn graph() -> DataForgeBuilder {
    schema()
        .record(tag(
            ROOT,
            "Root",
            TestValue::Array(vec![
                TestValue::Null,
                TestValue::Reference(WRONG),
                TestValue::Reference(MISSING),
                TestValue::Reference(LEAF),
                TestValue::Reference(LEAF),
            ]),
        ))
        .record(tag(LEAF, "Same", references(&[ROOT])))
        .record(tag(OTHER, "Same", references(&[LEAF])))
        .record(tag(LOOSE, "Loose", references(&[])))
        .record(roots(TestValue::Array(vec![
            TestValue::Null,
            TestValue::Reference(WRONG),
            TestValue::Reference(ROOT),
            TestValue::Reference(OTHER),
            TestValue::Reference(ROOT),
        ])))
        .record(tag(LEAF, "Shadow", references(&[])))
        .record(TestRecord::new(
            WRONG,
            "FixtureWrong",
            "FixtureWrong.NonTag",
            "synthetic/non-tag.xml",
        ))
}

pub(crate) fn clean() -> DataForgeBuilder {
    schema()
        .record(tag(ROOT, "Root", references(&[LEAF])))
        .record(tag(LEAF, "Same", references(&[])))
        .record(tag(OTHER, "Same", references(&[])))
        .record(roots(references(&[ROOT])))
}

pub(crate) fn entity_schema(
    source: DataForgeBuilder,
    tags: Option<TestType>,
    array: bool,
) -> DataForgeBuilder {
    let mut entity = TestStruct::new("EntityClassDefinition").array(
        "Components",
        TestType::StrongPointer("DataForgeComponentParams".to_owned()),
    );
    if let Some(ty) = tags {
        entity = if array {
            entity.array("tags", ty)
        } else {
            entity.field("tags", ty)
        };
    }
    source
        .structure(entity)
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", TestType::Class("TagFixtureItem".to_owned())),
        )
        .structure(
            TestStruct::new("TagFixtureItem")
                .field("Type", TestType::String)
                .field("Tags", TestType::String),
        )
        .structure(TestStruct::new("FixtureEmpty"))
}

pub(crate) fn entity(id: u128, name: &str, path: &str, tags: Option<TestValue>) -> TestRecord {
    let mut entity = TestRecord::new(
        id,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{name}"),
        format!("libs/foundry/records/entities/{path}.xml"),
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("SAttachableComponentParams").value(
                "AttachDef",
                TestValue::Struct(
                    TestInstance::new("TagFixtureItem")
                        .value("Type", TestValue::text("PowerPlant"))
                        .value("Tags", TestValue::text(" flightReady  $Small ")),
                ),
            ),
        )]),
    );
    if let Some(tags) = tags {
        entity = entity.value("tags", tags);
    }
    entity
}

pub(crate) fn entities() -> DataForgeBuilder {
    entity_schema(clean(), Some(TestType::ReferenceTo("Tag".to_owned())), true)
        .record(entity(
            ITEM,
            "Item",
            "scitem/ships/powerplant/included",
            Some(references(&[LEAF, LEAF, OTHER])),
        ))
        .record(entity(
            SHIP,
            "Ship",
            "spaceships/excluded",
            Some(references(&[ROOT])),
        ))
        .record(entity(
            COMMODITY,
            "Commodity",
            "commodities/excluded",
            Some(references(&[])),
        ))
}

pub(crate) fn build(
    database: &DataForgeBuilder,
    origin: DcbOrigin,
    mode: CatalogPublicationMode,
    filter: Option<&Filter>,
) -> TestResult<(BuiltCatalog, Value)> {
    let (_folder, archive) = archive_fixture::open(database, origin)?;
    let mut options = archive_fixture::options();
    options.publication_mode = mode;
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}

pub(crate) fn diagnostic(database: &DataForgeBuilder) -> TestResult<(BuiltCatalog, Value)> {
    build(
        database,
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        None,
    )
}

pub(crate) fn rows<'a>(catalog: &'a Value, table: &str) -> TestResult<&'a Vec<Value>> {
    at(catalog, &format!("/{table}/rows"))?
        .as_array()
        .ok_or_else(|| "typed rows".into())
}

pub(crate) fn row<'a>(catalog: &'a Value, table: &str, index: u64) -> TestResult<&'a Value> {
    rows(catalog, table)?
        .iter()
        .find(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(index))
        .ok_or_else(|| "physical row".into())
}
