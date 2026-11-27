// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::quality_v3_support as support;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Domain {
    Body,
    Header,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Location {
    Body {
        record: u64,
        field: String,
    },
    Header {
        record: u64,
        field: String,
    },
    Metadata {
        field: String,
    },
    Subresource {
        namespace: String,
        indices: Vec<String>,
        field: String,
    },
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Use {
    location: Location,
    role: String,
}

impl Use {
    pub(crate) fn new(domain: Domain, record: u64, field: &str, role: &str) -> Self {
        let field = field.to_owned();
        let location = match domain {
            Domain::Body => Location::Body { record, field },
            Domain::Header => Location::Header { record, field },
        };
        Self {
            location,
            role: role.to_owned(),
        }
    }
}

pub(crate) fn uses(catalog: &Value, carrier: &Value) -> TestResult<BTreeSet<Use>> {
    let references = support::at(carrier, "/sourceUsesRefs")?
        .as_array()
        .ok_or("immediate uses")?;
    let inventory = support::at(catalog, "/provenance/uses")?
        .as_array()
        .ok_or("use inventory")?;
    let sources = support::at(catalog, "/provenance/sources")?
        .as_array()
        .ok_or("source inventory")?;
    let mut result = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for reference in references {
        let id = reference.as_str().ok_or("source-use reference")?;
        assert!(
            seen.insert(id),
            "immediate source references must be distinct"
        );
        let mut matches = inventory
            .iter()
            .filter(|used| used.get("id").and_then(Value::as_str) == Some(id));
        let used = matches.next().ok_or("closed use reference")?;
        assert!(matches.next().is_none(), "use ids are unique");
        let source_id = support::at(used, "/source")?;
        let source = sources
            .iter()
            .find(|source| source.get("id") == Some(source_id))
            .ok_or("closed physical source")?;
        assert_eq!(
            support::at(source, "/kind")?,
            &json!("archiveEntry"),
            "the contribution comes from the original Archive Entry"
        );
        assert_eq!(
            support::at(source, "/path/value")?,
            &json!("Data\\Game2.dcb"),
            "the physical input is the original DCB rather than a locale or host file"
        );
        assert_eq!(
            support::at(used, "/scope")?,
            &json!("payload"),
            "the original DCB payload domain is explicit"
        );
        let role = support::at(used, "/role")?.as_str().ok_or("source role")?;
        assert!(
            matches!(role, "payload" | "selector" | "conversion"),
            "attempted and untouched references are observations, not contributors"
        );
        result.insert(Use {
            location: location(used)?,
            role: role.to_owned(),
        });
    }
    Ok(result)
}

fn location(used: &Value) -> TestResult<Location> {
    let field = support::at(used, "/location/field")?
        .as_str()
        .ok_or("source field")?
        .to_owned();
    match support::at(used, "/location/type")?.as_str() {
        Some("dataForge") => Ok(Location::Body {
            record: support::at(used, "/location/recordIndex")?
                .as_u64()
                .ok_or("body Record index")?,
            field,
        }),
        Some("metadata") => Ok(Location::Metadata { field }),
        Some("subresource") => {
            let namespace = support::at(used, "/location/namespace")?
                .as_str()
                .ok_or("subresource namespace")?;
            let encoded = support::at(used, "/location/indices")?
                .as_array()
                .ok_or("source indices")?;
            let mut indices = Vec::new();
            for index in encoded {
                indices.push(
                    index
                        .as_str()
                        .ok_or("canonical decimal source index")?
                        .to_owned(),
                );
            }
            if namespace == "dataforgeRecordHeader" {
                assert_eq!(
                    indices.len(),
                    1,
                    "the header belongs to one original physical Record"
                );
                let record = indices.first().ok_or("header index")?.parse()?;
                Ok(Location::Header { record, field })
            } else {
                Ok(Location::Subresource {
                    namespace: namespace.to_owned(),
                    indices,
                    field,
                })
            }
        }
        _ => Err(
            "a complete quality contribution needs an actual structured source coordinate".into(),
        ),
    }
}

pub(crate) fn marker_tree(value: &Value, expected: Option<bool>) -> TestResult {
    if value.get("state").is_some() {
        support::marker(value, expected)?;
    }
    match value {
        Value::Object(object) => {
            for child in object.values() {
                marker_tree(child, expected)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                marker_tree(child, expected)?;
            }
        }
        _ => {}
    }
    Ok(())
}
