// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::fmt::{self, Write as _};

use p4kx_extraction::Guid;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Serialize, Serializer};

const TEXT_LEN: usize = 36;

/// Canonical lower-case UUID text as sortable bytes.
pub(crate) type GuidText = [u8; TEXT_LEN];

/// A GUID written as its canonical lower-case UUID text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CatalogGuid(pub(crate) Guid);

impl CatalogGuid {
    /// Returns the original binary GUID.
    #[must_use]
    pub const fn get(self) -> Guid {
        self.0
    }

    /// Returns the canonical text as bytes, whose order is the order of the catalog.
    pub(crate) fn text(self) -> GuidText {
        let mut text = Text {
            bytes: [0; TEXT_LEN],
            len: 0,
        };
        let _complete = write!(text, "{}", self.0);
        text.bytes
    }
}

/// Orders `entities` by the text of the GUID `guid` returns.
pub(crate) fn sort_by_guid<T>(entities: &mut [T], guid: impl Fn(&T) -> CatalogGuid) {
    entities.sort_by_cached_key(|entity| guid(entity).text());
}

struct Text {
    bytes: GuidText,
    len: usize,
}

impl fmt::Write for Text {
    fn write_str(&mut self, part: &str) -> fmt::Result {
        let end = self.len.checked_add(part.len()).ok_or(fmt::Error)?;
        self.bytes
            .get_mut(self.len..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(part.as_bytes());
        self.len = end;
        Ok(())
    }
}

impl Serialize for CatalogGuid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl JsonSchema for CatalogGuid {
    fn schema_name() -> Cow<'static, str> {
        "Guid".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A GUID as canonical lower-case UUID text.",
            "type": "string",
            "format": "uuid",
            "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
        })
    }
}
