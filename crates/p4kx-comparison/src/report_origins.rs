// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    EntryDirectoryOriginsError, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath,
    FieldPathError, SourceEvidenceError, SourceInventory, SourceUses,
};
use schemars::JsonSchema;
use serde::ser::Error as _;
use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = historical_optional)]
pub(crate) struct ReportOriginFields {
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

impl ReportOriginFields {
    pub(crate) fn finish(
        fields: FieldOrigins,
        uses: &SourceUses,
    ) -> Result<Self, ReportOriginsError> {
        Ok(Self {
            field_origins: fields,
            provenance: uses.freeze()?,
        })
    }
}

fn historical_optional(schema: &mut schemars::Schema) {
    if let Some(properties) = schema.as_object_mut() {
        properties.remove("required");
    }
}

#[derive(Debug, Error)]
pub(crate) enum ReportOriginsError {
    #[error(transparent)]
    Entry(#[from] EntryDirectoryOriginsError),
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
    #[error(transparent)]
    Path(#[from] FieldPathError),
    #[error(transparent)]
    Duplicate(#[from] FieldOriginsError),
}

#[derive(Debug)]
pub(crate) struct ReportOrigins(pub(crate) Result<ReportOriginFields, ReportOriginsError>);

impl Serialize for ReportOrigins {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0
            .as_ref()
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}

pub(crate) fn put(
    fields: &mut FieldOrigins,
    path: &str,
    origin: FieldOrigin,
) -> Result<(), ReportOriginsError> {
    Ok(fields.insert(FieldPath::parse(path)?, origin)?)
}
