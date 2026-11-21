// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use schemars::Schema;
use serde_json::Value;

use super::super::super::schema_navigation::{resolve, schema_at};
use super::{Entry, Route, Shape, TestResult, row_schema};

pub(super) const NO_WITNESS: &str = "no query witness recorded by the audit";
const EXPORT_ATTEMPT_FIELDS: &[&str] = &["/schemaVersion", "/completeness", "/exportHealth"];

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
