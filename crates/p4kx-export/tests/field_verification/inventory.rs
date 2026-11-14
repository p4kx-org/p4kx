// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::error::{Error, Kind};
use super::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FieldKey {
    pub(crate) owner: String,
    pub(crate) field: String,
    pub(crate) array_element: bool,
}

impl FieldKey {
    pub(crate) fn new(owner: &str, field: &str) -> Self {
        Self {
            owner: owner.to_owned(),
            field: field.to_owned(),
            array_element: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Disposition {
    Exported { group: String, path: Path },
    ReferenceOnly { reason: String },
    Metadata { reason: String },
    Unsupported { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Mapping {
    pub(crate) source: FieldKey,
    pub(crate) declared_target: Option<String>,
    pub(crate) disposition: Disposition,
    pub(crate) previously_empty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Inventory {
    pub(crate) schema_version: u32,
    pub(crate) roots: Vec<String>,
    pub(crate) fields: Vec<Mapping>,
}

impl Inventory {
    pub(crate) fn reviewed(&self) -> super::Result<BTreeMap<FieldKey, &Mapping>> {
        if self.schema_version != 1
            || self.roots.is_empty()
            || self.fields.len() > 1_000_000
            || self.roots.len() > 10_000
        {
            return Err(Error::new(Kind::Bounds, "invalid inventory version or size").into());
        }
        let mut fields = BTreeMap::new();
        for mapping in &self.fields {
            if mapping.source.owner.is_empty() || mapping.source.field.is_empty() {
                return Err(Error::new(Kind::Mapping, "empty owning field identity").into());
            }
            match &mapping.disposition {
                Disposition::Exported { group, path } => {
                    if group.is_empty() {
                        return Err(Error::new(Kind::Mapping, "empty catalog group").into());
                    }
                    path.validate()?;
                }
                Disposition::ReferenceOnly { reason }
                | Disposition::Metadata { reason }
                | Disposition::Unsupported { reason }
                    if reason.trim().is_empty() =>
                {
                    return Err(Error::new(
                        Kind::Mapping,
                        "an exclusion requires a reviewed reason",
                    )
                    .into());
                }
                _ => {}
            }
            if fields.insert(mapping.source.clone(), mapping).is_some() {
                return Err(Error::new(Kind::Mapping, "duplicate owning field mapping").into());
            }
        }
        Ok(fields)
    }
}
