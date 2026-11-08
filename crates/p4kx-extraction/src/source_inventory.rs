// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::SourceEncryption;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Serialize;

use crate::projection_schema;
use crate::source_use::SourceUseDescription;
use crate::source_value::SourcedValue;
use crate::{ArchiveByteRange, DecimalU64};

/// A document-local physical-source reference, without a machine address.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct PhysicalSourceRef(pub(crate) String);

impl PhysicalSourceRef {
    /// Returns the stable document-local reference text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Conservatively interpreted encryption of a stored physical source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum SourceEncryptionState {
    /// The source is proved encrypted.
    Encrypted,
    /// The source is proved unencrypted.
    Unencrypted,
    /// Stored evidence does not establish either state.
    Unknown,
}

impl SourceEncryptionState {
    pub(crate) const fn of(value: SourceEncryption) -> Self {
        match value {
            SourceEncryption::Encrypted => Self::Encrypted,
            SourceEncryption::Unencrypted => Self::Unencrypted,
            _ => Self::Unknown,
        }
    }

    pub(crate) const fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Encrypted, _) | (_, Self::Encrypted) => Self::Encrypted,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Unencrypted, Self::Unencrypted) => Self::Unencrypted,
        }
    }
}

/// The exact declaration-bearing directory record format.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum SourceRecordFormat {
    /// A ZIP central-directory record, including CIG extensions.
    ZipCentralDirectory,
    /// A fixed P4K v2 directory record.
    P4kV2,
    /// A future record format not interpreted by this exporter.
    Unknown,
}

/// The physical field supplying an encryption declaration.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum SourceDeclarationKind {
    /// The CIG 0x5002 extra block.
    CigExtraBlock,
    /// The P4K v2 encryption word.
    P4kV2Field,
    /// A future uninterpreted declaration kind.
    Unknown,
}

/// An observed encryption declaration whose values originate in directory metadata.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = projection_schema::require_properties)]
pub struct SourceEncryptionDeclaration {
    pub(crate) kind: SourceDeclarationKind,
    pub(crate) value_offset: DecimalU64,
    pub(crate) declared_block_length: SourcedValue<u16>,
    pub(crate) raw_word: SourcedValue<u16>,
    pub(crate) raw_bytes: SourcedValue<Vec<u8>>,
}

/// Physical Archive Entry facts, retaining all stored declaration occurrences.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = projection_schema::require_properties)]
pub struct ArchiveEntrySource {
    pub(crate) entry_index: usize,
    pub(crate) path: SourcedValue<String>,
    pub(crate) record_format: SourceRecordFormat,
    #[schemars(schema_with = "record_range_schema")]
    pub(crate) record_range: ArchiveByteRange,
    pub(crate) zip_flags: SourcedValue<u16>,
    pub(crate) declarations: SourcedValue<Vec<SourcedValue<SourceEncryptionDeclaration>>>,
}

/// Observed standalone-file facts without a claim of original P4K lineage.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = projection_schema::require_properties)]
pub struct FileSource {
    pub(crate) name: SourcedValue<String>,
    pub(crate) byte_length: SourcedValue<DecimalU64>,
}

/// The closed physical-source inventory payload.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PhysicalSourceKind {
    /// One physical Entry inside its observed root input.
    ArchiveEntry(ArchiveEntrySource),
    /// One observed file input, without publication of its host path.
    File(FileSource),
    /// A source or ancestor whose physical binding is not known.
    Unknown,
}

/// One physical source, independent of the roles in which data values use it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = projection_schema::require_properties)]
pub struct PhysicalSource {
    pub(crate) id: PhysicalSourceRef,
    pub(crate) ancestors: Vec<PhysicalSourceRef>,
    pub(crate) stored_encryption: SourceEncryptionState,
    #[serde(flatten)]
    pub(crate) source: PhysicalSourceKind,
}

impl PhysicalSource {
    /// Returns this source's document-local identifier.
    #[must_use]
    pub const fn id(&self) -> &PhysicalSourceRef {
        &self.id
    }
}

/// Immutable sorted physical sources and observed use inventory for one prepared result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = projection_schema::require_properties)]
pub struct SourceInventory {
    pub(crate) sources: Vec<PhysicalSource>,
    pub(crate) uses: Vec<SourceUseDescription>,
}

impl SourceInventory {
    /// Returns the physical sources in stable identifier order.
    #[must_use]
    pub fn sources(&self) -> &[PhysicalSource] {
        &self.sources
    }

    /// Returns all observed uses, including separately classified attempts and references.
    #[must_use]
    pub fn uses(&self) -> &[SourceUseDescription] {
        &self.uses
    }
}

fn record_range_schema(generator: &mut SchemaGenerator) -> Schema {
    let mut schema = ArchiveByteRange::json_schema(generator);
    schema.remove("additionalProperties");
    schema
}
