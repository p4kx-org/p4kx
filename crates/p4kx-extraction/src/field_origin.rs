// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::Arc;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use crate::projection_schema;
use crate::source_inventory::SourceEncryptionState;
use crate::source_use::{SourceUse, SourceUseRef};

/// Shared authoritative uses and the immediate filter index for one data value.
#[derive(Debug, Clone)]
pub struct FieldOrigin {
    fields: Arc<OriginFields>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = projection_schema::require_properties)]
#[schemars(extend("allOf" = [{
    "if": {
        "properties": {"containsEncryptedSource": {"const": false}},
        "required": ["containsEncryptedSource"]
    },
    "then": {"properties": {"sourceSetComplete": {"const": true}}}
}]))]
struct OriginFields {
    contains_encrypted_source: Option<bool>,
    source_uses_refs: Vec<SourceUseRef>,
    source_set_complete: bool,
}

impl FieldOrigin {
    pub(crate) fn new<'a>(uses: impl IntoIterator<Item = &'a SourceUse>, complete: bool) -> Self {
        let mut refs = Vec::new();
        let mut encrypted = false;
        let mut unknown = !complete;
        let contributing = uses
            .into_iter()
            .filter(|source| source.role().contributes());
        for source in contributing {
            refs.push(source.reference().clone());
            match source.encryption() {
                SourceEncryptionState::Encrypted => encrypted = true,
                SourceEncryptionState::Unknown => unknown = true,
                SourceEncryptionState::Unencrypted => {}
            }
        }
        refs.sort();
        refs.dedup();
        let contains_encrypted_source = if encrypted {
            Some(true)
        } else if unknown {
            None
        } else {
            Some(false)
        };
        Self {
            fields: Arc::new(OriginFields {
                contains_encrypted_source,
                source_uses_refs: refs,
                source_set_complete: complete,
            }),
        }
    }

    /// Returns true for any proved encrypted contributor, false only for a complete unencrypted set.
    #[must_use]
    pub fn contains_encrypted_source(&self) -> Option<bool> {
        self.fields.contains_encrypted_source
    }

    /// Returns the exact contributing use references, excluding attempts and untouched targets.
    #[must_use]
    pub fn source_uses_refs(&self) -> &[SourceUseRef] {
        &self.fields.source_uses_refs
    }

    /// Returns completeness of this contribution set, independently of source or export health.
    #[must_use]
    pub fn source_set_complete(&self) -> bool {
        self.fields.source_set_complete
    }
}

impl Serialize for FieldOrigin {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.fields.as_ref().serialize(serializer)
    }
}

impl JsonSchema for FieldOrigin {
    fn schema_name() -> Cow<'static, str> {
        "FieldOrigin".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        OriginFields::json_schema(generator)
    }
}

impl FieldOrigin {
    pub(crate) fn from_wire(
        contains_encrypted_source: Option<bool>,
        source_uses_refs: Vec<SourceUseRef>,
        source_set_complete: bool,
    ) -> Result<Self, &'static str> {
        if contains_encrypted_source == Some(false) && !source_set_complete {
            return Err("unencrypted field origin requires complete contributors");
        }
        let empty_marker = if source_set_complete {
            Some(false)
        } else {
            None
        };
        if source_uses_refs.is_empty() && contains_encrypted_source != empty_marker {
            return Err("empty contributor origin has an inconsistent marker");
        }
        Ok(Self {
            fields: Arc::new(OriginFields {
                contains_encrypted_source,
                source_uses_refs,
                source_set_complete,
            }),
        })
    }
}
