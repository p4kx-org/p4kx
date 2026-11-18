// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::JsonSchema;
use serde_json::{Value, json};

use super::super::schema_navigation::{resolve, schema_at};
use super::{Entry, Owner, Shape, TestResult, dataset_owners};

#[path = "sdk_dataset_routes.rs"]
mod routes;
#[path = "sdk_row_schema.rs"]
mod row_schema;

const AMMUNITION: &str = "/definitions/ammoDefinitions";
const CONTAINERS: &str = "/placements/containerInstances";
const UNRESOLVED: &str = "/placements/unresolvedContainerLinks";
const NO_WITNESS: &str = "no query witness recorded by the audit";
const EXPORT_ATTEMPT_FIELDS: &[&str] = &["/schemaVersion", "/completeness", "/exportHealth"];

struct Route {
    dataset: &'static str,
    schema: Value,
    remaining: Option<&'static str>,
}

impl Route {
    fn queried_rows<Row: JsonSchema + serde::Serialize>(
        dataset: &'static str,
        rows: &[Row],
        remaining: &'static str,
    ) -> TestResult<Self> {
        if rows.is_empty() {
            return Err(format!("SDK witness returned no row for {dataset}").into());
        }
        Ok(Self {
            dataset,
            schema: schemars::generate::SchemaSettings::draft2020_12()
                .for_serialize()
                .into_generator()
                .into_root_schema_for::<Vec<Row>>()
                .to_value(),
            remaining: Some(remaining),
        })
    }
}

#[test]
fn sdk_query_witnesses_keep_their_canonical_dataset_contract() -> TestResult {
    let schema = p4kx_export::catalog_schema();
    let registered = dataset_owners::entries();
    let witnesses = routes::read()?;
    let gaps = audit(&schema, &registered, &witnesses)?;
    for path in [AMMUNITION, CONTAINERS, UNRESOLVED] {
        assert_ne!(
            gaps.get(path),
            Some(&NO_WITNESS),
            "the public query supplies a typed row for {path}"
        );
    }
    Ok(())
}

#[test]
fn acknowledged_new_dataset_still_needs_a_query() -> TestResult {
    let mut schema = p4kx_export::catalog_schema();
    let mut registered = dataset_owners::entries();
    let properties = schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .ok_or("catalog root properties")?;
    properties.insert("fixtureRows".to_owned(), json!({"type":"array","items":{}}));
    let gaps = audit(&schema, &registered, &[]);
    assert!(
        gaps.is_err(),
        "a dataset without ownership fails the strategy audit"
    );
    registered.insert(
        "/fixtureRows".to_owned(),
        Entry {
            owner: Owner::RootDataset,
            shape: Shape::Array,
        },
    );
    let gaps = audit(&schema, &registered, &[])?;
    assert_eq!(
        gaps.get("/fixtureRows"),
        Some(&NO_WITNESS),
        "assigning ownership does not establish an SDK route"
    );
    Ok(())
}

#[test]
fn a_public_query_cannot_discharge_a_different_dataset_or_value_shape() -> TestResult {
    let mut schema = p4kx_export::catalog_schema();
    let registered = dataset_owners::entries();
    let route = routes::ammunition()?;
    let incorrect = Route {
        dataset: "/definitions/inventoryContainers",
        ..route
    };
    assert!(
        audit(&schema, &registered, &[incorrect]).is_err(),
        "a different physical row type cannot satisfy a table-shaped route"
    );
    let float = schema
        .pointer_mut("/$defs/ResourceFloat32/properties/finite")
        .ok_or("referenced numeric field schema")?;
    *float = json!({"type":"string"});
    assert!(
        audit(&schema, &registered, &[routes::ammunition()?]).is_err(),
        "a changed reachable field shape cannot use the old query witness"
    );
    Ok(())
}

#[test]
#[ignore = "strict SDK dataset gate: run explicitly to list outstanding routes"]
fn every_catalog_v3_dataset_has_a_verified_sdk_query() -> TestResult {
    let schema = p4kx_export::catalog_schema();
    let gaps = audit(&schema, &dataset_owners::entries(), &routes::read()?)?;
    if gaps.is_empty() {
        return Ok(());
    }
    Err(CoverageGaps(gaps).into())
}

struct CoverageGaps(BTreeMap<String, &'static str>);

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

fn audit(
    schema: &Value,
    registered: &BTreeMap<String, Entry>,
    witnesses: &[Route],
) -> TestResult<BTreeMap<String, &'static str>> {
    let drift = super::compare(schema, registered)?;
    if !drift.is_empty() {
        return Err(format!("catalog ownership drift: {drift:?}").into());
    }
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
        let catalog_row = canonical_row(schema, route.dataset, entry.shape)?;
        let query_row = resolve(
            &route.schema,
            route.schema.get("items").ok_or("SDK row list")?,
        )?;
        row_schema::compare(schema, catalog_row, &route.schema, query_row)
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

fn canonical_row<'s>(schema: &'s Value, dataset: &str, shape: Shape) -> TestResult<&'s Value> {
    let path = match shape {
        Shape::Table => format!("{dataset}/rows/[]"),
        Shape::Array => format!("{dataset}/[]"),
        Shape::Object | Shape::Scalar => {
            return Err(format!("rows do not represent dataset {dataset}").into());
        }
    };
    schema_at(schema, &path)
}
