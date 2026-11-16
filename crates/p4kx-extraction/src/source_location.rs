// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Serialize, Serializer};

use super::SourceUseRef;
use crate::source_details::SourceCoordinateOmission;
use crate::{DecimalU64, Language};

/// A retained source coordinate or an explicit omission inside an observed physical input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SourceLocation {
    /// An actual input use with explicitly omitted optional coordinates.
    Opaque {
        /// Why coordinate details were not retained.
        reason: SourceCoordinateOmission,
    },
    /// One field of the physical DataForge Record index.
    DataForge {
        /// Physical Record position, independent of nominal GUID aliases.
        record_index: u32,
        /// Exact owned-field path, including stored list positions.
        field: String,
    },
    /// A localization lookup addressed by its retained authored selector.
    LocalizationLookup {
        /// The language Entry used by the lookup.
        #[serde(serialize_with = "language_code")]
        #[schemars(with = "String")]
        language: Language,
        /// The source use holding the actual lookup key.
        selector: SourceUseRef,
    },
    /// Half-open admitted Entry positions in a parsed root Archive directory.
    ArchiveDirectoryPaths {
        /// First admitted Entry path position.
        start_entry_index: DecimalU64,
        /// Position after the last consulted Entry path.
        end_entry_index: DecimalU64,
    },
    /// Half-open Entry positions in an original root Archive's path-ordered folder tree.
    ArchiveDirectoryTreePaths {
        /// First path-order Entry position.
        start_entry_index: DecimalU64,
        /// Position after the last contributing path-order Entry.
        end_entry_index: DecimalU64,
    },
    /// One exact logical field inside an already observed structured source subresource.
    Subresource {
        /// Format-owned namespace for the original source object.
        namespace: String,
        /// Original source ordinals, including repeated declarations and unknown slots.
        #[serde(serialize_with = "subresource_indices")]
        #[schemars(with = "Vec<DecimalU64>")]
        indices: Vec<u64>,
        /// Original source field or token-range label within that object.
        field: String,
    },
    /// A directory or standalone input metadata field.
    Metadata {
        /// Exact metadata field, including stored occurrence positions.
        field: String,
    },
}

impl SourceLocation {
    pub(crate) fn key(&self) -> String {
        match self {
            Self::Opaque { reason } => format!("o:{}", reason.key()),
            Self::DataForge {
                record_index,
                field,
            } => DataForgeField {
                record_index: *record_index,
                field,
            }
            .key(),
            Self::LocalizationLookup { language, selector } => {
                let code = language.code();
                format!(
                    "q{}:{code}{}:{}",
                    code.len(),
                    selector.as_str().len(),
                    selector.as_str()
                )
            }
            Self::ArchiveDirectoryPaths {
                start_entry_index,
                end_entry_index,
            } => format!("a{}:{}", start_entry_index.get(), end_entry_index.get()),
            Self::ArchiveDirectoryTreePaths {
                start_entry_index,
                end_entry_index,
            } => format!("t{}:{}", start_entry_index.get(), end_entry_index.get()),
            Self::Subresource {
                namespace,
                indices,
                field,
            } => Subresource {
                namespace,
                indices,
                field,
            }
            .key(),
            Self::Metadata { field } => format!("m{}:{field}", field.len()),
        }
    }
}

fn subresource_indices<S: Serializer>(indices: &[u64], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(indices.iter().copied().map(DecimalU64::new))
}
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "Serde field serializers receive references"
)]
fn language_code<S: Serializer>(language: &Language, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(language.code())
}

pub(crate) enum LocationView<'location> {
    Original(&'location SourceLocation),
    DataForgeField(DataForgeField<'location>),
    Subresource(Subresource<'location>),
}

impl LocationView<'_> {
    pub(crate) fn key(&self) -> String {
        match self {
            Self::Original(location) => location.key(),
            Self::DataForgeField(location) => location.key(),
            Self::Subresource(location) => location.key(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct DataForgeField<'location> {
    pub(crate) record_index: u32,
    pub(crate) field: &'location str,
}

impl DataForgeField<'_> {
    fn key(&self) -> String {
        let Self {
            record_index,
            field,
        } = self;
        format!("d{record_index}:{}:{field}", field.len())
    }

    pub(crate) fn to_owned_location(self) -> SourceLocation {
        SourceLocation::DataForge {
            record_index: self.record_index,
            field: self.field.to_owned(),
        }
    }
}

pub(crate) struct Subresource<'location> {
    pub(crate) namespace: &'location str,
    pub(crate) indices: &'location [u64],
    pub(crate) field: &'location str,
}

impl Subresource<'_> {
    fn key(&self) -> String {
        use std::fmt::Write as _;

        let Self {
            namespace,
            indices,
            field,
        } = self;
        let mut key = format!("s{}:{namespace}{}:", namespace.len(), indices.len());
        for index in *indices {
            let _written = write!(&mut key, "{index}.");
        }
        let _written = write!(&mut key, "{}:{field}", field.len());
        key
    }

    pub(crate) fn to_owned_location(&self) -> SourceLocation {
        SourceLocation::Subresource {
            namespace: self.namespace.to_owned(),
            indices: self.indices.to_vec(),
            field: self.field.to_owned(),
        }
    }
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum LocationWire<'location> {
    DataForge {
        record_index: u32,
        field: &'location str,
    },
    Subresource {
        namespace: &'location str,
        #[serde(serialize_with = "subresource_indices")]
        indices: &'location [u64],
        field: &'location str,
    },
}

impl Serialize for DataForgeField<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        LocationWire::DataForge {
            record_index: self.record_index,
            field: self.field,
        }
        .serialize(serializer)
    }
}

impl Serialize for Subresource<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        LocationWire::Subresource {
            namespace: self.namespace,
            indices: self.indices,
            field: self.field,
        }
        .serialize(serializer)
    }
}
