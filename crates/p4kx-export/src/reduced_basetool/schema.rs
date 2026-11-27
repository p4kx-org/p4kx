// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::SourceValueState;
use schemars::generate::SchemaSettings;
use serde_json::{Map, Value, json};

use super::plan;
use super::selected_tables::{Selection, is_staged_dataset};
use crate::contract::{SCHEMA_VERSION, Source};
use crate::ship_store_pledge::ShipStorePledges;

#[path = "schema_texts.rs"]
mod texts;

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;

const DRAFT: &str = "https://json-schema.org/draft/2020-12/schema";
const TABLE: &str = "#/$defs/BasetoolDatasetTable";

/// Returns the structural Draft 2020-12 schema of the reduced Basetool Catalog.
/// Dataset row interiors are governed by the selected-field contract.
#[must_use]
pub fn basetool_catalog_schema() -> Value {
    let mut generator = SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator();
    let source = generator.subschema_for::<Source<'static>>();
    let pledges = generator.subschema_for::<ShipStorePledges>();
    let state = generator.subschema_for::<SourceValueState>();
    let mut definitions = generator.definitions().clone();
    definitions.insert("BasetoolDatasetTable".to_owned(), dataset_table());

    let catalog = catalog([
        ("schemaVersion", json!({"const": SCHEMA_VERSION})),
        (
            "source",
            json!({"allOf": [source], "unevaluatedProperties": false}),
        ),
        (
            "shipStorePledges",
            json!({"allOf": [pledges], "unevaluatedProperties": false}),
        ),
        ("texts", texts::schema(&state)),
        ("exportHealth", health()),
        ("completeness", completeness()),
    ]);
    json!({
        "$schema": DRAFT,
        "$defs": definitions,
        "title": "P4KX reduced Basetool Catalog structure",
        "description": "Validates the versioned envelope, source metadata, required selected datasets, coverage shape and selected text containers. Dataset row fields are not validated.",
        "type": "object",
        "additionalProperties": false,
        "required": ["format", "formatVersion", "catalog"],
        "properties": {
            "format": {"const": plan::FORMAT},
            "formatVersion": {"const": plan::FORMAT_VERSION},
            "catalog": catalog,
        }
    })
}

fn catalog(metadata: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    let mut properties = BTreeMap::from_iter(metadata);
    let mut sections = BTreeMap::<&str, BTreeMap<&str, Value>>::new();
    for path in plan::SELECTION
        .into_iter()
        .filter(|path| is_staged_dataset(path))
    {
        if let Some((section, dataset)) = path.split_once('.') {
            sections
                .entry(section)
                .or_default()
                .insert(dataset, json!({"$ref": TABLE}));
        } else {
            properties.insert(path, rows());
        }
    }
    for (section, datasets) in sections {
        properties.insert(section, closed_object(datasets));
    }
    closed_object(properties)
}

fn health() -> Value {
    closed_object([
        ("status", json!({"const": "complete"})),
        ("scope", json!({"const": plan::SELECTION.as_slice()})),
    ])
}

fn completeness() -> Value {
    let entries = plan::SELECTION
        .into_iter()
        .filter(|path| is_staged_dataset(path));
    let coverage: Vec<_> = entries
        .map(|path| {
            closed_object([
                ("path", json!({"const": path})),
                ("rows", json!({"type": "integer", "minimum": 0})),
                ("selection", json!({"const": Selection::for_path(path)})),
            ])
        })
        .collect();
    json!({
        "type": "array",
        "minItems": coverage.len(),
        "prefixItems": coverage,
        "items": false
    })
}

fn dataset_table() -> Value {
    json!({
        "allOf": [
            closed_object([
                ("availability", json!({"enum": ["available", "absent"]})),
                ("rows", rows()),
            ]),
            {
                "if": {"properties": {"availability": {"const": "absent"}}, "required": ["availability"]},
                "then": {"properties": {"rows": {"maxItems": 0}}}
            }
        ]
    })
}

fn rows() -> Value {
    json!({"type": "array", "items": {"type": "object"}})
}

fn closed_object(properties: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    let properties: Map<_, _> = properties
        .into_iter()
        .map(|(name, property)| (name.to_owned(), property))
        .collect();
    let required: Vec<_> = properties.keys().collect();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}
