// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, CatalogError, CatalogPublicationMode, Kind};
use p4kx_extraction::{DataForge, RecordText};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};
use serde_json::Value;

pub(crate) use crate::canonical_header_fixture::build;
pub(crate) use crate::commodity_v3_fixture::at;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

pub(crate) const CATEGORY: u128 = 0x702;
pub(crate) const LOWER: u128 = 0x701;
pub(crate) const LEFTOVER: u128 = 0x703;
pub(crate) const DATABASE: u128 = 0x704;
pub(crate) const SOURCE_TYPE: &str = "BlueprintCategoryRecord";
pub(crate) const RAW_NAME: &str = "BlueprintCategoryRecord.RawCategory";
pub(crate) const FILE_NAME: &str =
    "libs/foundry/records/crafting/blueprintcategories/included/StoredCategory.XML";
pub(crate) const PATH: &str =
    "libs/foundry/records/crafting/blueprintcategories/included/StoredCategory";
pub(crate) const TAG: &str = "CategoryDomain";
pub(crate) const FIELDS: [&str; 8] = [
    "guid",
    "recordName",
    "className",
    "recordPath",
    "recordIndex",
    "sourceType",
    "fileName",
    "metadataTag",
];

pub(crate) fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(SOURCE_TYPE))
}

pub(crate) fn category(guid: u128, name: &str, file_name: &str) -> TestRecord {
    TestRecord::new(guid, SOURCE_TYPE, name, file_name).tag(TAG)
}

pub(crate) fn single() -> DataForgeBuilder {
    schema().record(category(CATEGORY, RAW_NAME, FILE_NAME))
}

pub(crate) fn original(
    source: &DataForgeBuilder,
    index: u32,
    guid: u128,
    name: &str,
    file_name: &str,
    tag: &str,
) -> TestResult {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(index).ok_or("original physical category")?;
    assert_eq!(
        record.index(),
        index,
        "the original physical index is explicit"
    );
    assert_eq!(
        record.guid().to_string(),
        common::guid(guid),
        "original GUID"
    );
    assert_eq!(record.type_name()?, SOURCE_TYPE, "original declared type");
    assert_eq!(record.stored_name()?, RecordText::Present(name), "raw name");
    assert_eq!(
        record.stored_file_name()?,
        file_name,
        "exact stored filename"
    );
    assert_eq!(
        record.stored_tag()?,
        RecordText::Present(tag),
        "raw metadata tag"
    );
    Ok(())
}

pub(crate) fn fieldless(source: &DataForgeBuilder, index: u32) -> TestResult {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(index).ok_or("original category")?.tree()?;
    assert_eq!(
        tree.record_index(),
        index,
        "the tree has the same physical owner"
    );
    assert_eq!(
        tree.root().fields().len(),
        0,
        "this source category has no body fields"
    );
    Ok(())
}

pub(crate) fn rows(catalog: &Value) -> TestResult<&[Value]> {
    at(catalog, "/definitions/blueprintCategories/rows")?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| "canonical BlueprintCategory rows are absent".into())
}

pub(crate) fn row(catalog: &Value, index: u64) -> TestResult<&Value> {
    rows(catalog)?
        .iter()
        .find(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(index))
        .ok_or_else(|| format!("canonical physical category {index} is absent").into())
}

pub(crate) fn counts(built: &BuiltCatalog, count: usize, skipped: usize) -> TestResult {
    let summary = built
        .summary()
        .kind(Kind::BlueprintCategories)
        .ok_or("category summary")?;
    assert_eq!(
        summary.count, count,
        "retained physical categories are counted"
    );
    assert_eq!(
        summary.skipped, skipped,
        "only actual category leftovers are skipped"
    );
    Ok(())
}

pub(crate) fn health(catalog: &Value) -> TestResult<&Value> {
    at(catalog, "/exportHealth/kinds")?
        .as_array()
        .ok_or("kind health")?
        .iter()
        .find(|row| row.get("kind").and_then(Value::as_str) == Some("blueprintCategories"))
        .ok_or_else(|| "BlueprintCategory health is absent".into())
}

pub(crate) fn refused(source: &DataForgeBuilder) -> TestResult {
    let (_folder, archive) = common::open(source, DcbOrigin::Plain)?;
    assert!(
        matches!(
            p4kx_export::build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "a genuine category source/coverage failure must refuse normal publication"
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
