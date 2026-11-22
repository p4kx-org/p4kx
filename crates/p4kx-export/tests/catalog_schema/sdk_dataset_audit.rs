// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::Schema;
use serde_json::Value;

use super::super::super::schema_navigation::{resolve, schema_at};
use super::{Entry, Route, Shape, TestResult, row_schema};

pub(super) const NO_WITNESS: &str = "no query witness recorded by the audit";
const EXPORT_ATTEMPT_FIELDS: &[&str] = &[
    "/schemaVersion",
    "/completeness",
    "/exportHealth",
    "/source/archiveKey",
    "/source/encryptedEntriesUnread",
    "/source/exportedAt",
    "/source/toolVersion",
];

pub(super) fn audit(
    schema: Value,
    registered: &BTreeMap<String, Entry>,
    witnesses: &[Route],
) -> TestResult<BTreeMap<String, &'static str>> {
    let drift = super::super::compare(&schema, registered)?;
    if !drift.is_empty() {
        return Err(format!("catalog ownership drift: {drift:?}").into());
    }
    let mut schema = Schema::try_from(schema)?;
    row_schema::remove_descriptions(&mut schema);
    let schema = schema.to_value();
    let mut gaps: BTreeMap<_, _> = registered
        .keys()
        .filter(|path| !EXPORT_ATTEMPT_FIELDS.contains(&path.as_str()))
        .map(|path| (path.clone(), NO_WITNESS))
        .collect();
    let mut seen = BTreeSet::new();
    for route in witnesses {
        let entry = registered
            .get(route.dataset)
            .ok_or_else(|| format!("SDK query claims an unknown dataset {}", route.dataset))?;
        if !seen.insert(route.dataset) || EXPORT_ATTEMPT_FIELDS.contains(&route.dataset) {
            return Err(format!("invalid SDK route claim for {}", route.dataset).into());
        }
        let catalog_row = canonical_value(&schema, route.dataset, entry.shape)?;
        let query_row = resolve(
            &route.schema,
            route.schema.get("items").ok_or("SDK row list")?,
        )?;
        row_schema::compare(&schema, catalog_row, &route.schema, query_row)
            .map_err(|error| format!("SDK route {}: {error}", route.dataset))?;
        match route.remaining {
            Some(reason) => {
                gaps.insert(route.dataset.to_owned(), reason);
            }
            None => {
                gaps.remove(route.dataset);
            }
        }
    }
    Ok(gaps)
}

fn canonical_value<'s>(schema: &'s Value, dataset: &str, shape: Shape) -> TestResult<&'s Value> {
    let path = match shape {
        Shape::Table => format!("{dataset}/rows/[]"),
        Shape::Array => format!("{dataset}/[]"),
        Shape::Object | Shape::Scalar => return schema_at(schema, dataset),
    };
    schema_at(schema, &path)
}

pub(super) struct CoverageGaps(pub(super) BTreeMap<String, &'static str>);

impl fmt::Display for CoverageGaps {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(output, "{} unfinished SDK dataset routes", self.0.len())?;
        for (path, reason) in &self.0 {
            writeln!(output, "{path} {reason}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for CoverageGaps {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, output)
    }
}

impl std::error::Error for CoverageGaps {}

#[test]
fn an_unclassified_source_member_still_fails_the_coverage_audit() -> TestResult {
    let mut schema = p4kx_export::catalog_schema();
    let fields = schema
        .pointer_mut("/$defs/BuildSource/properties")
        .and_then(Value::as_object_mut)
        .ok_or("catalog build metadata")?;
    fields.insert(
        "fixtureNewSourceField".to_owned(),
        serde_json::json!({"type": "string"}),
    );
    let registered = super::super::dataset_owners::entries();
    let error = audit(schema, &registered, &[])
        .err()
        .ok_or("a new source member requires an ownership decision")?;
    assert!(
        error
            .to_string()
            .contains("/source/build/fixtureNewSourceField"),
        "the audit reports the exact new member without an export-metadata wildcard",
    );
    Ok(())
}
