// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, CatalogError, CatalogPublicationMode, Filter, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, StructValue, Value as Raw};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::quality_v3_capture_fixture::{NAN_BITS, NAN_HEX};
use crate::quality_v3_support as support;

pub(crate) struct CapturedFixture {
    _folder: tempfile::TempDir,
    archive: Archive,
    input: DataForgeEntry,
}

impl CapturedFixture {
    pub(crate) fn new(database: &DataForgeBuilder) -> TestResult<Self> {
        let (folder, archive) = common::open(database, DcbOrigin::Plain)?;
        let input = DataForgeEntry::read(&archive)?;
        Ok(Self {
            _folder: folder,
            archive,
            input,
        })
    }

    pub(crate) fn resource_mean(&self, index: u32) -> TestResult {
        let forge = self.input.open()?;
        let tree = forge.record(index).ok_or("original ResourceType")?.tree()?;
        let Some(Raw::Array(properties)) = tree.field("properties") else {
            return Err("original property array".into());
        };
        let property = structure(properties.first())?;
        assert_original_mean(structure(property.field("qualityDistribution"))?)
    }

    pub(crate) fn spare_mean(&self) -> TestResult {
        let forge = self.input.open()?;
        let tree = forge.record(0).ok_or("original Quality owner")?.tree()?;
        assert_original_mean(structure(tree.field("spareCurve"))?)
    }

    pub(crate) fn refused_then_diagnostic(
        &self,
        filter: Option<&Filter>,
    ) -> TestResult<(BuiltCatalog, Value)> {
        let mut options = common::options();
        options.filter = filter;
        assert!(
            matches!(
                build_catalog_from(&self.archive, &self.input, &options),
                Err(CatalogError::IncompleteExport { .. })
            ),
            "the required original token diagnosis blocks actual normal publication"
        );
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        let built = build_catalog_from(&self.archive, &self.input, &options)?;
        let catalog = serde_json::from_slice(built.json())?;
        assert_eq!(
            support::at(&catalog, "/exportHealth/status")?,
            &json!("incomplete"),
            "diagnostic publication retains the actual source failure"
        );
        Ok((built, catalog))
    }
}

fn structure<'tree, 'source>(
    value: Option<&'tree Raw<'source>>,
) -> TestResult<&'tree StructValue<'source>> {
    match value {
        Some(Raw::Struct(source)) => Ok(source),
        _ => Err("actual owned source structure".into()),
    }
}

fn assert_original_mean(source: &StructValue<'_>) -> TestResult {
    let Some(Raw::Double(value)) = source.field("mean") else {
        return Err("original Double mean".into());
    };
    assert_eq!(
        value.to_bits(),
        NAN_BITS,
        "the synthetic original contains the exact nonfinite Double token before export"
    );
    Ok(())
}

pub(crate) fn assert_uncovered(
    catalog: &Value,
    kind: &str,
    role: &str,
    record: u64,
    field: &str,
) -> TestResult {
    let diagnostics = support::at(catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("source diagnostics")?;
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.get("kind").and_then(Value::as_str) == Some(kind)
                && diagnostic.get("sourceRole").and_then(Value::as_str) == Some(role)
                && diagnostic.get("stage").and_then(Value::as_str) == Some("projection")
                && diagnostic.get("code").and_then(Value::as_str) == Some("unsupportedValue")
                && diagnostic.get("severity").and_then(Value::as_str) == Some("error")
                && diagnostic
                    .get("examples")
                    .and_then(Value::as_array)
                    .is_some_and(|examples| {
                        examples.iter().any(|example| {
                            example.get("recordIndex").and_then(Value::as_u64) == Some(record)
                                && example.get("field").and_then(Value::as_str) == Some(field)
                                && example.get("sourceType").and_then(Value::as_str)
                                    == Some("CraftingQualityDistributionNormal")
                        })
                    })
        }),
        "the original physical token keeps its exact blocking diagnostic; a Kind, GUID alias, other field or unrelated refusal is not capture proof"
    );
    Ok(())
}

pub(crate) fn assert_projected(catalog: &Value, kind: &str) -> TestResult {
    let health = support::kind_health(catalog, kind)?;
    for field in ["selected", "readSuccessful", "projected"] {
        assert_eq!(
            support::at(health, &format!("/sourceRecords/{field}"))?,
            &json!(1),
            "the selected raw definition survives a separate token failure"
        );
    }
    assert_eq!(
        support::at(health, "/sourceRecords/omittedAfterProjectionFailure")?,
        &json!(0),
        "field diagnostics do not fabricate whole-owner omission"
    );
    Ok(())
}

pub(crate) fn assert_exported_mean(value: &Value) -> TestResult {
    support::state(value, "present")?;
    assert_eq!(
        support::at(value, "/value/type")?,
        &json!("f64"),
        "the positive raw fact retains its source width"
    );
    assert_eq!(
        support::at(value, "/value/rawBits")?,
        &json!(NAN_HEX),
        "the other captured field has identical bits and still cannot cover this token"
    );
    assert_eq!(
        support::at(value, "/value/finite")?,
        &Value::Null,
        "the finite subprojection alone is null"
    );
    support::marker(value, Some(false))
}
