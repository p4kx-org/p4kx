// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use serde::Deserialize;

use crate::origin_fact_read::{Declaration, Fact};
use crate::origin_source_shape::{
    CoordinateIndices, CoordinateText, DecimalCoordinate, EntryIndex, ProofDetails, RecordFormat,
    RecordRange,
};
use crate::{MediaError, origin_decimal};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Encryption {
    Encrypted,
    Unencrypted,
    Unknown,
}

impl Encryption {
    pub(crate) const fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Encrypted, _) | (_, Self::Encrypted) => Self::Encrypted,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Unencrypted, Self::Unencrypted) => Self::Unencrypted,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SourceKind {
    ArchiveEntry,
    ArchiveEntryProof,
    ArchiveFile,
    ArchiveDirectory,
    File,
    Unknown,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceRow {
    pub(crate) id: String,
    pub(crate) ancestors: Vec<String>,
    pub(crate) stored_encryption: Encryption,
    pub(crate) kind: SourceKind,
    pub(crate) entry_index: Option<EntryIndex>,
    pub(crate) admitted_entry_count: Option<DirectoryOrdinal>,
    pub(crate) details: Option<ProofDetails>,
    pub(crate) record_format: Option<RecordFormat>,
    pub(crate) record_range: Option<RecordRange>,
    pub(crate) path: Option<Fact<String>>,
    pub(crate) zip_flags: Option<Fact<u16>>,
    pub(crate) name: Option<Fact<String>>,
    pub(crate) byte_length: Option<Fact<DecimalCoordinate>>,
    pub(crate) declarations: Option<Fact<Vec<Fact<Declaration>>>>,
}

impl SourceRow {
    pub(crate) fn require_shape(&self) -> Result<(), MediaError> {
        let valid = match self.kind {
            SourceKind::ArchiveEntry => {
                self.entry_index == Some(EntryIndex::Numeric)
                    && self.path.is_some()
                    && self.zip_flags.is_some()
                    && self.record_format.is_some()
                    && self.record_range.is_some()
                    && self.declarations.is_some()
            }
            SourceKind::ArchiveEntryProof => {
                self.entry_index == Some(EntryIndex::Decimal) && self.details.is_some()
            }
            SourceKind::ArchiveFile => self.byte_length.is_some(),
            SourceKind::ArchiveDirectory => self.admitted_entry_count.is_some(),
            SourceKind::File => self.name.is_some() && self.byte_length.is_some(),
            SourceKind::Unknown => true,
        };
        if !valid {
            return Err(MediaError::Destination(
                "physical source lacks its current typed description".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Scope {
    Payload,
    ArchiveDirectoryMetadata,
    FileMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Role {
    Payload,
    Selector,
    Conversion,
    Attempted,
    Reference,
}

impl Role {
    pub(crate) const fn contributes(self) -> bool {
        matches!(self, Self::Payload | Self::Selector | Self::Conversion)
    }
}

#[derive(Deserialize)]
pub(crate) struct UseRow {
    pub(crate) id: String,
    pub(crate) source: String,
    pub(crate) scope: Scope,
    pub(crate) role: Role,
    #[serde(rename = "effectiveEncryption")]
    pub(crate) effective_encryption: Encryption,
    pub(crate) location: Location,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Location {
    #[serde(rename = "type")]
    pub(crate) kind: LocationKind,
    pub(crate) selector: Option<String>,
    pub(crate) reason: Option<CoordinateOmission>,
    namespace: Option<CoordinateText>,
    indices: Option<CoordinateIndices>,
    field: Option<CoordinateText>,
    language: Option<CoordinateText>,
    record_index: Option<u32>,
    start_entry_index: Option<DirectoryOrdinal>,
    end_entry_index: Option<DirectoryOrdinal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub(crate) struct DirectoryOrdinal(
    #[serde(deserialize_with = "origin_decimal::deserialize")] DecimalU64,
);

impl DirectoryOrdinal {
    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }
}

impl Location {
    pub(crate) fn directory_paths(&self) -> Option<[DirectoryOrdinal; 2]> {
        if matches!(
            self.kind,
            LocationKind::ArchiveDirectoryPaths | LocationKind::ArchiveDirectoryTreePaths
        ) {
            Some([self.start_entry_index?, self.end_entry_index?])
        } else {
            None
        }
    }

    pub(crate) fn require_shape(&self) -> Result<(), MediaError> {
        let valid = match self.kind {
            LocationKind::Opaque => self.reason.is_some(),
            LocationKind::Metadata => self.field.is_some(),
            LocationKind::DataForge => self.record_index.is_some() && self.field.is_some(),
            LocationKind::LocalizationLookup => self.language.is_some() && self.selector.is_some(),
            LocationKind::ArchiveDirectoryPaths | LocationKind::ArchiveDirectoryTreePaths => {
                self.directory_paths().is_some()
            }
            LocationKind::Subresource => {
                self.namespace.is_some() && self.indices.is_some() && self.field.is_some()
            }
        };
        if !valid {
            return Err(MediaError::Destination(
                "source use lacks its current typed coordinate description".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LocationKind {
    Opaque,
    DataForge,
    LocalizationLookup,
    Subresource,
    ArchiveDirectoryPaths,
    ArchiveDirectoryTreePaths,
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CoordinateOmission {
    CaptureLimit,
    Unavailable,
}

pub(crate) struct TypedSeed<T>(pub(crate) std::marker::PhantomData<T>);

impl<'de, T: Deserialize<'de>> serde::de::DeserializeSeed<'de> for TypedSeed<T> {
    type Value = T;

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<T, D::Error> {
        T::deserialize(deserializer)
    }
}

pub(crate) const fn seed<T>() -> TypedSeed<T> {
    TypedSeed(std::marker::PhantomData)
}

#[cfg(test)]
#[path = "origin_opaque_reason_tests.rs"]
mod opaque_reason_tests;
