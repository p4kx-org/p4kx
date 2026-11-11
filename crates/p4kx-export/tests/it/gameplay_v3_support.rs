// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::commodity_v3_fixture::{self as common, DcbOrigin};
pub(crate) use crate::commodity_v3_fixture::{TestResult, at};
pub(crate) use crate::quality_v3_support::{marker, state};
use p4kx_export::{BuiltCatalog, CatalogError, CatalogPublicationMode, Filter};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

pub(crate) const TABLE: &str = "/definitions/gameplayProperties";
pub(crate) const FIRST: &str = "/definitions/gameplayProperties/rows/0";
pub(crate) const DISPLAY: &str = "/displayTransformation/value/nodes/0/payload/value";
pub(crate) const SCALE: &str = "/displayTransformation/value/nodes/0/payload/value/scale";

pub(crate) fn raw(
    database: &DataForgeBuilder,
    origin: DcbOrigin,
    mode: CatalogPublicationMode,
    filter: Option<&Filter>,
) -> TestResult<BuiltCatalog> {
    let (_folder, archive) = common::open(database, origin)?;
    let mut options = common::options();
    options.publication_mode = mode;
    options.filter = filter;
    let result = p4kx_export::build_catalog(&archive, &options);
    assert!(
        result.is_ok(),
        "the requested original-source export must return a catalog: {:?}",
        result.as_ref().err()
    );
    Ok(result?)
}

pub(crate) fn build(
    database: &DataForgeBuilder,
    origin: DcbOrigin,
    mode: CatalogPublicationMode,
    filter: Option<&Filter>,
) -> TestResult<(BuiltCatalog, Value)> {
    let built = raw(database, origin, mode, filter)?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}

pub(crate) fn normal(database: &DataForgeBuilder) -> TestResult<(BuiltCatalog, Value)> {
    build(
        database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    )
}

pub(crate) fn diagnostic(database: &DataForgeBuilder) -> TestResult<(BuiltCatalog, Value)> {
    build(
        database,
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        None,
    )
}

pub(crate) fn rows(catalog: &Value) -> TestResult<&[Value]> {
    assert!(
        catalog.pointer(TABLE).is_some(),
        "schema v3 requires the canonical D.gameplayProperties table"
    );
    crate::gameplay_v3_consumer::rows(catalog, p4kx_export::Kind::GameplayProperties)
        .map(Vec::as_slice)
}

pub(crate) fn row(catalog: &Value, index: u64) -> TestResult<&Value> {
    rows(catalog)?
        .iter()
        .find(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(index))
        .ok_or_else(|| format!("original physical gameplay property {index}").into())
}

pub(crate) fn refused(database: &DataForgeBuilder, filter: Option<&Filter>) -> TestResult {
    let (_folder, archive) = common::open(database, DcbOrigin::Plain)?;
    let mut options = common::options();
    options.filter = filter;
    assert!(
        matches!(
            p4kx_export::build_catalog(&archive, &options),
            Err(CatalogError::IncompleteExport(_))
        ),
        "the actual source violation must refuse normal publication"
    );
    Ok(())
}

pub(crate) fn diagnosis(
    catalog: &Value,
    code: &str,
    record: u64,
    field: &str,
    owner: &str,
) -> TestResult {
    let diagnostics = at(catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("diagnostics")?;
    let mut found = false;
    for diagnostic in diagnostics {
        if diagnostic.get("kind").and_then(Value::as_str) != Some("gameplayProperties")
            || diagnostic.get("code").and_then(Value::as_str) != Some(code)
        {
            continue;
        }
        let examples = diagnostic["examples"]
            .as_array()
            .ok_or("gameplay examples")?;
        for example in examples {
            found |= example
                .pointer("/recordIndex/value")
                .and_then(Value::as_u64)
                == Some(record)
                && crate::health_origin_assertions::example_text(catalog, example, "field")?
                    == Some(&json!(field))
                && crate::health_origin_assertions::example_text(catalog, example, "sourceType")?
                    == Some(&json!(owner));
        }
    }
    assert!(
        found,
        "the exact {code} keeps physical Record {record}, original {owner} and {field}"
    );
    Ok(())
}

pub(crate) fn count(catalog: &Value, written: u64, skipped: u64) -> TestResult {
    let health = crate::quality_v3_support::kind_health(catalog, "gameplayProperties")?;
    assert_eq!(
        at(health, "/writtenRows")?,
        &json!(written),
        "physical emitted definition count"
    );
    assert_eq!(
        at(health, "/intentionallySkippedRows")?,
        &json!(skipped),
        "own leftover row count"
    );
    Ok(())
}
