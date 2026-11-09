// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, CatalogError, CatalogPublicationMode, Filter};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

pub(crate) use crate::canonical_header_fixture::build;
pub(crate) use crate::commodity_v3_fixture::at;

pub(crate) const KIND: &str = "blueprintCategoryLists";
pub(crate) const TYPE: &str = "BlueprintCategoryDatabaseRecord";
pub(crate) const CATEGORY_TYPE: &str = "BlueprintCategoryRecord";
pub(crate) const LIST: u128 = 0xf10;
pub(crate) const SECOND: u128 = 0xf20;
pub(crate) const LEFTOVER: u128 = 0xf30;
pub(crate) const ALPHA: u128 = 0x110;
pub(crate) const BETA: u128 = 0x120;
pub(crate) const OTHER: u128 = 0x220;
pub(crate) const MISSING: u128 = 0x990;
pub(crate) const LIST_FILE: &str = "synthetic/category_lists/included/Primary.XML";
pub(crate) const SECOND_FILE: &str = "unrelated/listing/Secondary.XML";
pub(crate) const CATEGORY_FOLDER: &str = "libs/foundry/records/crafting/blueprintcategories/";

pub(crate) fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Padding"))
        .structure(TestStruct::new(CATEGORY_TYPE))
        .structure(list_schema())
        .structure(TestStruct::new("Other"))
}

pub(crate) fn list_schema() -> TestStruct {
    TestStruct::new(TYPE).array(
        "categories",
        TestType::ReferenceTo(CATEGORY_TYPE.to_owned()),
    )
}

pub(crate) fn listing(guid: u128, name: &str, file: &str, slots: TestValue) -> TestRecord {
    TestRecord::new(guid, TYPE, format!("{TYPE}.{name}"), file)
        .tag(format!("{name}Domain"))
        .value("categories", slots)
}

pub(crate) fn category(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        CATEGORY_TYPE,
        format!("{CATEGORY_TYPE}.{name}"),
        format!("{CATEGORY_FOLDER}{name}.XML"),
    )
    .tag(format!("{name}Domain"))
}

pub(crate) fn clean() -> DataForgeBuilder {
    schema()
        .record(listing(LIST, "Primary", LIST_FILE, ordered(false)))
        .record(category(ALPHA, "Alpha"))
        .record(TestRecord::new(
            OTHER,
            "Other",
            "Other.Foreign",
            "synthetic/foreign.XML",
        ))
        .record(category(BETA, "Beta"))
        .record(listing(
            SECOND,
            "Secondary",
            SECOND_FILE,
            references(&[ALPHA]),
        ))
        .record(listing(
            LEFTOVER,
            "Test_Listing",
            "synthetic/category_lists/included/Unused.XML",
            references(&[BETA]),
        ))
}

pub(crate) fn graph() -> DataForgeBuilder {
    schema()
        .record(listing(LIST, "Primary", LIST_FILE, ordered(true)))
        .record(category(ALPHA, "Alpha"))
        .record(TestRecord::new(
            OTHER,
            "Other",
            "Other.Foreign",
            "synthetic/foreign.XML",
        ))
        .record(category(BETA, "Beta"))
}

fn ordered(with_errors: bool) -> TestValue {
    let mut slots = vec![
        TestValue::Reference(BETA),
        TestValue::Null,
        TestValue::Reference(ALPHA),
        TestValue::Reference(BETA),
    ];
    if with_errors {
        slots.push(TestValue::Reference(MISSING));
        slots.push(TestValue::Reference(OTHER));
    }
    TestValue::Array(slots)
}

pub(crate) fn references(guids: &[u128]) -> TestValue {
    TestValue::Array(guids.iter().copied().map(TestValue::Reference).collect())
}

pub(crate) fn rows(catalog: &Value) -> TestResult<&[Value]> {
    at(catalog, "/declarations/blueprintCategoryLists/rows")?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| "canonical category-list rows are absent".into())
}

pub(crate) fn row(catalog: &Value, index: u64) -> TestResult<&Value> {
    rows(catalog)?
        .iter()
        .find(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(index))
        .ok_or_else(|| format!("physical category list {index} is absent").into())
}

pub(crate) fn count(built: &BuiltCatalog, count: usize, skipped: usize) -> TestResult {
    let kind = built
        .summary()
        .kinds()
        .iter()
        .find(|row| row.kind.name() == KIND)
        .ok_or("independent category-list Kind summary is absent")?;
    assert_eq!(
        kind.count, count,
        "list counts describe physical database owners"
    );
    assert_eq!(
        kind.skipped, skipped,
        "only database leftovers affect the new Kind"
    );
    Ok(())
}

pub(crate) fn health(catalog: &Value) -> TestResult<&Value> {
    at(catalog, "/exportHealth/kinds")?
        .as_array()
        .ok_or("kind health")?
        .iter()
        .find(|row| row.get("kind").and_then(Value::as_str) == Some(KIND))
        .ok_or_else(|| "category-list health is absent".into())
}

pub(crate) fn refused(source: &DataForgeBuilder, filter: Option<&Filter>) -> TestResult {
    let (_folder, archive) = common::open(source, DcbOrigin::Plain)?;
    let mut options = common::options();
    options.filter = filter;
    assert!(
        matches!(
            p4kx_export::build_catalog(&archive, &options),
            Err(CatalogError::IncompleteExport { .. })
        ),
        "incomplete original source evidence must refuse normal publication"
    );
    Ok(())
}

pub(crate) fn diagnostic(source: &DataForgeBuilder) -> TestResult<(BuiltCatalog, Value)> {
    build(
        source,
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        None,
    )
}
