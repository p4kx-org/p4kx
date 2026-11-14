// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::Arc;

use crate::HealthText;
use p4kx_extraction::{DecimalU64, SourcedValue};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

/// A report-local text-storage reference without original-input authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthTextRef {
    atom_index: DecimalU64,
}

impl HealthTextRef {
    pub(crate) const fn new(index: u64) -> Self {
        Self {
            atom_index: DecimalU64::new(index),
        }
    }

    /// Returns the document-local storage position.
    #[must_use]
    pub const fn atom_index(self) -> u64 {
        self.atom_index.get()
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "HealthMetadata")]
struct Metadata {
    #[schemars(with = "Vec<SourcedValue<String>>")]
    texts: Vec<SourcedValue<HealthText>>,
}

/// Immutable value storage shared by one Health report and its borrowed views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthMetadata {
    data: Arc<Metadata>,
}

impl HealthMetadata {
    pub(crate) fn new(texts: Vec<SourcedValue<HealthText>>) -> Self {
        Self {
            data: Arc::new(Metadata { texts }),
        }
    }

    /// Borrows shared text data without importing its aggregate origin into an occurrence.
    #[must_use]
    pub fn text(&self, reference: HealthTextRef) -> Option<&SourcedValue<HealthText>> {
        let index = usize::try_from(reference.atom_index()).ok()?;
        self.data.texts.get(index)
    }

    /// Borrows each stored aggregate text datum.
    #[must_use]
    pub fn texts(&self) -> &[SourcedValue<HealthText>] {
        &self.data.texts
    }
}

impl Serialize for HealthMetadata {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.data.as_ref().serialize(serializer)
    }
}

impl JsonSchema for HealthMetadata {
    fn inline_schema() -> bool {
        Metadata::inline_schema()
    }
    fn schema_name() -> Cow<'static, str> {
        Metadata::schema_name()
    }
    fn schema_id() -> Cow<'static, str> {
        Metadata::schema_id()
    }
    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        Metadata::json_schema(generator)
    }
}
