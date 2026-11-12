// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{
    DeclaredEncryption, EncryptionDeclarationSource, EntryRecordFormat, SourceEncryption,
};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Serialize;

use crate::projection_schema;
use crate::source_archive_binding::ArchiveSourceBinding;
use crate::source_details::SourceDetails;
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
    /// Reconciles descriptive declaration states with directory format and ZIP-flag facts.
    #[must_use]
    pub fn from_declarations(
        record_format: SourceRecordFormat,
        zip_flags: Option<u16>,
        declarations: impl IntoIterator<Item = Self>,
    ) -> Self {
        let format = match record_format {
            SourceRecordFormat::ZipCentralDirectory => Some(EntryRecordFormat::ZipCentralDirectory),
            SourceRecordFormat::P4kV2 => Some(EntryRecordFormat::P4kV2),
            SourceRecordFormat::Unknown => None,
        };
        let declared =
            DeclaredEncryption::reconcile(declarations.into_iter().map(Self::as_declared));
        Self::of(SourceEncryption::from_declarations(
            declared, format, zip_flags,
        ))
    }

    const fn as_declared(self) -> DeclaredEncryption {
        match self {
            Self::Encrypted => DeclaredEncryption::Encrypted,
            Self::Unencrypted => DeclaredEncryption::Unencrypted,
            Self::Unknown => DeclaredEncryption::Unknown,
        }
    }

    const fn of_declared(value: DeclaredEncryption) -> Self {
        match value {
            DeclaredEncryption::Encrypted => Self::Encrypted,
            DeclaredEncryption::Unencrypted => Self::Unencrypted,
            _ => Self::Unknown,
        }
    }

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
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

impl SourceDeclarationKind {
    /// Interprets descriptive raw field facts without establishing an original source binding.
    #[must_use]
    pub const fn interpret(
        self,
        declared_block_length: Option<u16>,
        raw_word: Option<u16>,
    ) -> SourceEncryptionState {
        let source = match self {
            Self::CigExtraBlock => EncryptionDeclarationSource::CigExtraBlock,
            Self::P4kV2Field => EncryptionDeclarationSource::P4kV2Field,
            Self::Unknown => return SourceEncryptionState::Unknown,
        };
        SourceEncryptionState::of_declared(DeclaredEncryption::interpret(
            source,
            declared_block_length,
            raw_word,
        ))
    }
}

/// An observed encryption declaration whose values originate in directory metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
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

/// Minimal original Entry proof when optional stored details were not retained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = projection_schema::require_properties)]
pub struct ArchiveEntryProof {
    pub(crate) entry_index: DecimalU64,
    #[schemars(extend("const" = "captureLimited"))]
    pub(crate) details: SourceDetails,
}

/// Admitted Entry positions from the original root Archive directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = projection_schema::require_properties)]
pub struct ArchiveDirectorySource {
    pub(crate) admitted_entry_count: DecimalU64,
}

/// Measured whole-Archive file facts independently of its stored Entry declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = projection_schema::require_properties)]
pub struct ArchiveFileSource {
    pub(crate) byte_length: SourcedValue<DecimalU64>,
}

/// Observed standalone-file facts without a claim of original P4K lineage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = projection_schema::require_properties)]
pub struct FileSource {
    pub(crate) name: SourcedValue<String>,
    pub(crate) byte_length: SourcedValue<DecimalU64>,
}

/// The closed physical-source inventory payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PhysicalSourceKind {
    /// One physical Entry inside its observed root input.
    ArchiveEntry(ArchiveEntrySource),
    /// Original Entry identity and encryption with explicitly limited stored details.
    ArchiveEntryProof(ArchiveEntryProof),
    /// The original root Archive's parsed directory.
    ArchiveDirectory(ArchiveDirectorySource),
    /// The completed whole-file read of an observed Archive input.
    ArchiveFile(ArchiveFileSource),
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
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) binding: Option<ArchiveSourceBinding>,
    #[serde(flatten)]
    pub(crate) source: PhysicalSourceKind,
}

impl PartialEq for PhysicalSource {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.ancestors == other.ancestors
            && self.stored_encryption == other.stored_encryption
            && self.source == other.source
    }
}

impl Eq for PhysicalSource {}
impl PhysicalSource {
    /// Returns this source's document-local identifier.
    #[must_use]
    pub const fn id(&self) -> &PhysicalSourceRef {
        &self.id
    }
}

/// Immutable sorted physical sources and observed use inventory for one prepared result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
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
