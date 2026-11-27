// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_extraction::{
    DataForgeError, DataForgeLoadError, FieldOriginsError, FieldPathError, SourceEvidenceError,
    SourceInventorySelection,
};
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};
use serde_json::Value;
use thiserror::Error;

use super::{BuiltShipBase, SHIP_BASE_VERSION, ShipBase, ShipBaseEvidence};
use crate::CatalogError;

/// The ship base data cannot be built or written.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ShipBaseError {
    /// The DataForge Entry cannot be found or read.
    #[error(transparent)]
    Load(DataForgeLoadError),
    /// The DataForge has an unsupported version or is corrupt.
    #[error(transparent)]
    DataForge(DataForgeError),
    /// The data cannot be written as JSON.
    #[error("cannot write the ship base data as JSON")]
    Json(#[from] serde_json::Error),
    /// The output cannot be written.
    #[error("cannot write the ship base data")]
    Io(#[from] io::Error),
    /// The export was cancelled.
    #[error("the ship base export was cancelled")]
    Cancelled,
    /// An original input or contribution could not be bound reliably.
    #[error(transparent)]
    Original(Box<CatalogError>),
    /// A standalone field address cannot be represented.
    #[error(transparent)]
    FieldPath(#[from] FieldPathError),
    /// A standalone field already has an origin.
    #[error(transparent)]
    FieldOrigin(#[from] FieldOriginsError),
    /// A retained reference could not be resolved.
    #[error(transparent)]
    SourceEvidence(#[from] SourceEvidenceError),
}

impl From<CatalogError> for ShipBaseError {
    fn from(error: CatalogError) -> Self {
        match error {
            CatalogError::Cancelled => Self::Cancelled,
            other => Self::Original(Box::new(other)),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document<'a> {
    schema_version: u32,
    source: &'a Value,
    filter: Option<&'a str>,
    ships: Rows<'a>,
    provenance: SourceInventorySelection<'a>,
}

#[derive(Serialize)]
struct Row<'a> {
    #[serde(flatten)]
    ship: &'a ShipBase,
    #[serde(flatten)]
    evidence: &'a ShipBaseEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    provenance: Option<SourceInventorySelection<'a>>,
}

struct Rows<'a>(&'a BuiltShipBase);

impl Serialize for Rows<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let rows = self.0.rows_with_evidence();
        let mut sequence = serializer.serialize_seq(Some(rows.len()))?;
        for (ship, evidence) in rows {
            sequence.serialize_element(&Row {
                ship,
                evidence,
                provenance: None,
            })?;
        }
        sequence.end()
    }
}

impl BuiltShipBase {
    /// Writes one JSON document with the source-use closure of all emitted rows.
    ///
    /// # Errors
    /// Returns an error if a source reference cannot be resolved or writing fails.
    pub fn write_json(&self, out: &mut impl Write) -> Result<(), ShipBaseError> {
        serde_json::to_writer(
            &mut *out,
            &Document {
                schema_version: SHIP_BASE_VERSION,
                source: &self.source,
                filter: self.filter.as_deref(),
                ships: Rows(self),
                provenance: self.provenance()?,
            },
        )?;
        out.write_all(b"\n")?;
        Ok(())
    }

    /// Writes one ship and its own source-use closure per line in [`Self::ships`] order.
    ///
    /// # Errors
    /// Returns an error if a source reference cannot be resolved or writing fails.
    pub fn write_ndjson(&self, out: &mut impl Write) -> Result<(), ShipBaseError> {
        for (ship, evidence) in self.rows_with_evidence() {
            let provenance = Some(self.source_inventory.select_uses(evidence.references())?);
            serde_json::to_writer(
                &mut *out,
                &Row {
                    ship,
                    evidence,
                    provenance,
                },
            )?;
            out.write_all(b"\n")?;
        }
        Ok(())
    }
}
