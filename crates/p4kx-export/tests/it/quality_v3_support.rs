// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, CatalogError, CatalogPublicationMode, Filter};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

pub(crate) const DISTRIBUTIONS: &str = "definitions/qualityDistributions";
pub(crate) const QUANTIZATIONS: &str = "definitions/qualityQuantizations";
pub(crate) const INVERSE: &str = "projections/qualityQuantizationResources";
pub(crate) const CURVE: &str = "/qualityDistribution/value/payload/value/value/value";
pub(crate) const BANDS: &str = "/qualityQuantization/value/payload/value/value/value/bands/value";

pub(crate) fn build(
    database: &DataForgeBuilder,
    origin: DcbOrigin,
    mode: CatalogPublicationMode,
    filter: Option<&Filter>,
) -> TestResult<(BuiltCatalog, Value)> {
    let (_folder, archive) = common::open(database, origin)?;
    let mut options = common::options();
    options.publication_mode = mode;
    options.filter = filter;
    let built = p4kx_export::build_catalog(&archive, &options)?;
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

pub(crate) fn assert_normal_refusal(database: &DataForgeBuilder) -> TestResult {
    let (_folder, archive) = common::open(database, DcbOrigin::Plain)?;
    assert!(
        matches!(
            p4kx_export::build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "the actual required field violation must block normal publication"
    );
    Ok(())
}

pub(crate) fn at<'value>(value: &'value Value, pointer: &str) -> TestResult<&'value Value> {
    common::at(value, pointer)
}

pub(crate) fn rows<'catalog>(
    catalog: &'catalog Value,
    dataset: &str,
) -> TestResult<&'catalog [Value]> {
    at(catalog, &format!("/{dataset}/rows"))?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("canonical {dataset} rows are absent").into())
}

pub(crate) fn row<'catalog>(
    catalog: &'catalog Value,
    dataset: &str,
    index: u64,
) -> TestResult<&'catalog Value> {
    rows(catalog, dataset)?
        .iter()
        .find(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(index))
        .ok_or_else(|| format!("physical {dataset} row {index} is absent").into())
}

pub(crate) fn state(value: &Value, expected: &str) -> TestResult {
    assert_eq!(
        at(value, "/state")?,
        &json!(expected),
        "the original field state must remain explicit"
    );
    Ok(())
}

pub(crate) fn marker(value: &Value, expected: Option<bool>) -> TestResult {
    let expected = expected.map_or(Value::Null, Value::Bool);
    assert_eq!(
        at(value, "/containsEncryptedSource")?,
        &expected,
        "the immediate marker must be explicitly present, including unknown"
    );
    assert!(
        at(value, "/sourceUsesRefs")?.is_array(),
        "the value carries actual source-use references"
    );
    assert!(
        at(value, "/sourceSetComplete")?.is_boolean(),
        "contributor completeness has its own explicit key"
    );
    Ok(())
}

pub(crate) fn kind_health<'catalog>(
    catalog: &'catalog Value,
    kind: &str,
) -> TestResult<&'catalog Value> {
    at(catalog, "/exportHealth/kinds")?
        .as_array()
        .ok_or("kind health")?
        .iter()
        .find(|row| row.get("kind").and_then(Value::as_str) == Some(kind))
        .ok_or_else(|| format!("health of {kind} is absent").into())
}

pub(crate) fn assert_field_error(catalog: &Value, field: &str, source_type: &str) -> TestResult {
    let diagnostics = at(catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("diagnostics")?;
    let health = at(catalog, "/exportHealth")?;
    let mut found = false;
    for diagnostic in diagnostics {
        if diagnostic.get("code").and_then(Value::as_str) != Some("malformedSource") {
            continue;
        }
        let examples = at(diagnostic, "/examples")?
            .as_array()
            .ok_or("diagnostic examples")?;
        for example in examples {
            let actual_field = at(example, "/field")?;
            let actual_type = at(example, "/sourceType")?;
            if actual_field.get("state").and_then(Value::as_str) == Some("present")
                && actual_type.get("state").and_then(Value::as_str) == Some("present")
            {
                found |= crate::health_origin_assertions::text_value(health, actual_field)?
                    == &json!(field)
                    && crate::health_origin_assertions::text_value(health, actual_type)?
                        == &json!(source_type);
            }
        }
    }
    assert!(
        found,
        "the field violation keeps its original source type and exact field location"
    );
    Ok(())
}

pub(crate) fn guid(value: u128) -> String {
    common::guid(value)
}
