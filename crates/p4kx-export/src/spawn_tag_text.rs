// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use crate::catalog_v3::tag_catalog::OriginalTagPath;

/// Content text storage; this value alone carries no original-source admission.
#[derive(Clone)]
pub(crate) enum SpawnTagText {
    Shared(OriginalTagPath),
    Owned(String),
}

impl SpawnTagText {
    pub(crate) const fn shared(text: OriginalTagPath) -> Self {
        Self::Shared(text)
    }
    pub(crate) const fn owned(text: String) -> Self {
        Self::Owned(text)
    }
}

impl fmt::Debug for SpawnTagText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SpawnTagText")
            .finish_non_exhaustive()
    }
}

impl Serialize for SpawnTagText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Shared(text) => text.serialize(serializer),
            Self::Owned(text) => text.serialize(serializer),
        }
    }
}

impl JsonSchema for SpawnTagText {
    fn inline_schema() -> bool {
        String::inline_schema()
    }
    fn schema_name() -> Cow<'static, str> {
        String::schema_name()
    }
    fn schema_id() -> Cow<'static, str> {
        String::schema_id()
    }
    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        String::json_schema(generator)
    }
}
