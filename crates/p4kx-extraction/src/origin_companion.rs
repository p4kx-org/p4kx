// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use schemars::JsonSchema;
use serde::Serialize;

use crate::{
    DecimalU64, FieldOrigin, FieldPath, OriginArtifactDigest, OriginArtifactName, PhysicalSource,
    SourceInventory, SourceUseDescription,
};

/// Format name of the portable standalone field-origin companion.
pub const SOURCE_ORIGIN_FORMAT: &str = "p4kx-source-origin";
/// Current format version of the standalone field-origin companion.
pub const SOURCE_ORIGIN_VERSION: &str = "1.0";

/// A file artifact or the raw stdout stream addressed by a companion.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub enum OriginArtifactBinding {
    /// A sibling file identified by its relative basename and measured bytes.
    File {
        /// Exact relative artifact basename.
        path: OriginArtifactName,
        /// Actual written byte length and digest.
        #[serde(flatten)]
        digest: OriginArtifactDigest,
    },
    /// A raw stdout stream identified by its measured bytes.
    Stdout {
        /// Actual written byte length and digest.
        #[serde(flatten)]
        digest: OriginArtifactDigest,
    },
}

/// The XML value represented by one emitted byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum OriginXmlField {
    /// Opening element name.
    ElementName,
    /// Closing element name.
    ClosingElementName,
    /// Attribute name.
    AttributeName,
    /// Attribute value.
    AttributeValue,
    /// Element text.
    Text,
}

/// The exact datum within a bound standalone artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[non_exhaustive]
pub enum OriginOutputAddress {
    /// The entire payload of an opaque byte copy or output.
    OpaquePayload,
    /// One typed field in the emitted XML.
    XmlBytes {
        /// First byte in the emitted field.
        start: DecimalU64,
        /// First byte past the emitted field.
        end_exclusive: DecimalU64,
        /// Category of the emitted field.
        field: OriginXmlField,
    },
    /// A typed field at an exact JSON Pointer in the original artifact schema.
    JsonField {
        /// Exact field path.
        path: FieldPath,
    },
}

/// One JSONL row in a portable standalone source-origin companion.
#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[non_exhaustive]
pub enum OriginCompanionRow<'receipt> {
    /// Format and measured artifact binding.
    Header {
        /// Fixed format identifier.
        #[schemars(regex(pattern = "^p4kx-source-origin$"))]
        format: &'static str,
        /// Fixed wire version.
        #[schemars(regex(pattern = "^1[.]0$"))]
        format_version: &'static str,
        /// Exact bound artifact.
        binding: &'receipt OriginArtifactBinding,
    },
    /// One original physical source.
    Source {
        /// Shared physical evidence.
        source: &'receipt PhysicalSource,
    },
    /// One observed use of a physical source.
    SourceUse {
        /// Shared source-use evidence.
        source_use: &'receipt SourceUseDescription,
    },
    /// One emitted field with its immediate origin.
    Field {
        /// Exact artifact address.
        address: &'receipt OriginOutputAddress,
        /// Actual uses and derived immediate marker.
        #[serde(flatten)]
        origin: &'receipt FieldOrigin,
    },
    /// Explicit completion and observed row counts.
    End {
        /// Number of original source rows.
        sources: DecimalU64,
        /// Number of original use rows.
        source_uses: DecimalU64,
        /// Number of addressed output fields.
        fields: DecimalU64,
    },
}

impl<'receipt> OriginCompanionRow<'receipt> {
    /// Constructs the fixed wire header for the measured artifact.
    #[must_use]
    pub const fn header(binding: &'receipt OriginArtifactBinding) -> Self {
        Self::Header {
            format: SOURCE_ORIGIN_FORMAT,
            format_version: SOURCE_ORIGIN_VERSION,
            binding,
        }
    }
}

/// Writes a complete versioned field-origin companion to a prepared sink.
///
/// # Errors
/// Returns an output or serialization error and never reports completion for a partial stream.
pub fn write_origin_companion<'receipt>(
    mut writer: impl Write,
    binding: &'receipt OriginArtifactBinding,
    inventory: &'receipt SourceInventory,
    fields: impl IntoIterator<Item = (&'receipt OriginOutputAddress, &'receipt FieldOrigin)>,
) -> io::Result<()> {
    row(&mut writer, &OriginCompanionRow::header(binding))?;
    for source in inventory.sources() {
        row(&mut writer, &OriginCompanionRow::Source { source })?;
    }
    for source_use in inventory.uses() {
        row(&mut writer, &OriginCompanionRow::SourceUse { source_use })?;
    }
    let mut count = 0_u64;
    for (address, origin) in fields {
        row(&mut writer, &OriginCompanionRow::Field { address, origin })?;
        count = count
            .checked_add(1)
            .ok_or_else(|| io::Error::other("origin field count exceeds u64"))?;
    }
    row(
        &mut writer,
        &OriginCompanionRow::End {
            sources: DecimalU64::new(inventory.sources().len() as u64),
            source_uses: DecimalU64::new(inventory.uses().len() as u64),
            fields: DecimalU64::new(count),
        },
    )?;
    writer.flush()
}

fn row(writer: &mut impl Write, row: &OriginCompanionRow<'_>) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, row)?;
    writer.write_all(b"\n")
}
