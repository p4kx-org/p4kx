// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, btree_map};

use serde_json::Value;

use super::schema_navigation::{escape, resolve};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[path = "dataset_owners.rs"]
mod dataset_owners;
#[path = "sdk_dataset_coverage.rs"]
mod sdk_dataset_coverage;

use dataset_owners::{Entry, Owner, Shape};

const SECTIONS: &[&str] = &[
    "configurations",
    "declarations",
    "definitions",
    "placements",
    "projections",
    "texts",
];

#[test]
fn every_catalog_v3_dataset_has_one_classified_contract_owner() -> TestResult {
    let schema = p4kx_export::catalog_schema();
    let registered = dataset_owners::entries();
    let drift = compare(&schema, &registered)?;
    assert!(drift.is_empty(), "catalog ownership drift: {drift:?}");
    assert_eq!(
        registered["/declarations/blueprintCategoryLists"].owner,
        Owner::Declaration
    );
    assert_eq!(
        registered["/definitions/gameplayProperties"].owner,
        Owner::Definition
    );
    Ok(())
}

#[test]
fn adding_or_removing_a_table_requires_an_ownership_decision() -> TestResult {
    let mut schema = p4kx_export::catalog_schema();
    let registered = dataset_owners::entries();
    let namespace = schema
        .pointer_mut("/$defs/CatalogDefinitions")
        .ok_or("generated definitions namespace")?;
    let fields = namespace["properties"]
        .as_object_mut()
        .ok_or("definition properties")?;
    let existing = fields
        .remove("gameplayProperties")
        .ok_or("existing table")?;
    fields.insert("fixtureNewTable".to_owned(), existing);
    let required = namespace["required"]
        .as_array_mut()
        .and_then(|fields| {
            fields
                .iter_mut()
                .find(|field| **field == "gameplayProperties")
        })
        .ok_or("required existing table")?;
    *required = Value::String("fixtureNewTable".to_owned());

    let drift = compare(&schema, &registered)?;
    assert_eq!(drift.missing, ["/definitions/fixtureNewTable"]);
    assert_eq!(drift.stale, ["/definitions/gameplayProperties"]);
    assert_eq!(
        drift.changed,
        [],
        "the other dataset shapes stay classified"
    );
    Ok(())
}

#[derive(Debug, Default)]
struct Drift {
    missing: Vec<String>,
    stale: Vec<String>,
    changed: Vec<(String, Shape, Shape)>,
}

impl Drift {
    const fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.stale.is_empty() && self.changed.is_empty()
    }
}

fn compare(schema: &Value, registered: &BTreeMap<String, Entry>) -> TestResult<Drift> {
    let mut observed = BTreeMap::new();
    let root = schema["properties"]
        .as_object()
        .ok_or("generated catalog properties")?;
    for (name, node) in root {
        let path = format!("/{}", escape(name));
        if SECTIONS.contains(&name.as_str()) {
            namespace(schema, node, &path, registered, &mut observed)?;
        } else {
            observe(schema, node, path, registered, &mut observed)?;
        }
    }
    let mut drift = Drift::default();
    for (path, &shape) in &observed {
        match registered.get(path) {
            None => drift.missing.push(path.clone()),
            Some(entry) if entry.shape != shape => {
                drift.changed.push((path.clone(), entry.shape, shape));
            }
            Some(_) => {}
        }
    }
    drift.stale.extend(
        registered
            .keys()
            .filter(|path| !observed.contains_key(*path))
            .cloned(),
    );
    Ok(drift)
}

fn namespace(
    schema: &Value,
    node: &Value,
    path: &str,
    registered: &BTreeMap<String, Entry>,
    observed: &mut BTreeMap<String, Shape>,
) -> TestResult {
    let node = resolve(schema, node)?;
    let fields = node["properties"]
        .as_object()
        .ok_or_else(|| format!("object namespace at {path}"))?;
    if fields.is_empty() {
        return Err(format!("empty dataset namespace at {path}").into());
    }
    for (name, child) in fields {
        let child_path = format!("{path}/{}", escape(name));
        observe(schema, child, child_path, registered, observed)?;
    }
    Ok(())
}

fn observe(
    schema: &Value,
    node: &Value,
    path: String,
    registered: &BTreeMap<String, Entry>,
    observed: &mut BTreeMap<String, Shape>,
) -> TestResult {
    let node = resolve(schema, node)?;
    let fields = node["properties"].as_object();
    let shape = if let Some(fields) = fields
        && fields.contains_key("availability")
        && let Some(rows) = fields.get("rows")
    {
        if resolve(schema, rows)?["type"] != "array" {
            return Err(format!("{path} has a non-array rows field").into());
        }
        Shape::Table
    } else if node["type"] == "array" {
        Shape::Array
    } else if let Some(fields) = fields
        && !fields.is_empty()
        && !registered.contains_key(&path)
    {
        return namespace(schema, node, &path, registered, observed);
    } else if fields.is_some() || node["type"] == "object" {
        Shape::Object
    } else {
        Shape::Scalar
    };
    match observed.entry(path) {
        btree_map::Entry::Vacant(slot) => {
            slot.insert(shape);
            Ok(())
        }
        btree_map::Entry::Occupied(slot) => {
            Err(format!("generated dataset pointer repeated: {}", slot.key()).into())
        }
    }
}
