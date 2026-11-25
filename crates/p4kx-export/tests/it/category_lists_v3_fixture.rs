// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, CatalogError, CatalogPublicationMode, Filter};
use p4kx_testkit::{DataForgeBuilder, TestRecord};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

pub(crate) use crate::canonical_header_fixture::build;
pub(crate) use crate::commodity_v3_fixture::at;

pub(crate) const KIND: &str = "blueprintCategoryLists";
#[path = "category_lists_v3_source_fixture.rs"]
mod source;

pub(crate) use source::{
    ALPHA, BETA, CATEGORY_FOLDER, CATEGORY_TYPE, LIST, LIST_FILE, MISSING, OTHER, SECOND,
    SECOND_FILE, TYPE, category, list_schema, listing, references,
};

pub(crate) fn schema() -> DataForgeBuilder {
    source::with_schema(DataForgeBuilder::new().version(8))
}

pub(crate) fn clean() -> DataForgeBuilder {
    source::with_clean_lists(DataForgeBuilder::new().version(8))
}

pub(crate) fn graph() -> DataForgeBuilder {
    schema()
        .record(listing(LIST, "Primary", LIST_FILE, source::ordered(true)))
        .record(category(ALPHA, "Alpha"))
        .record(TestRecord::new(
            OTHER,
            "Other",
            "Other.Foreign",
            "synthetic/foreign.XML",
        ))
        .record(category(BETA, "Beta"))
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
            p4kx_export::build_catalog(&archive, options.with_full_provenance()),
            Err(CatalogError::IncompleteExport(_))
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
