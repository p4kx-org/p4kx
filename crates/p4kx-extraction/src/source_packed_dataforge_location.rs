// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::OnceLock;

use super::{
    Coordinates, DataForgeSite, HeaderField, RetainedLocation, SURVEY_FIELD, StoredDataForgeField,
    StoredDataForgeSite, ValueField,
};
use crate::source_use::SourceUseRef;

/// A DataForge coordinate stored beside its original immutable source-use ID.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PackedDataForgeLocation {
    coordinate: PackedCoordinate,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum PackedCoordinate {
    Field {
        record_index: u32,
        field_start: usize,
    },
    Header {
        field: HeaderField,
        record_index: u32,
    },
    RecordDeclarations {
        record_index: u32,
    },
    Value {
        field: ValueField,
        record_index: u32,
        ordinal: u64,
    },
}

const _: () = assert!(
    size_of::<PackedDataForgeLocation>() <= 2 * size_of::<u64>(),
    "packed DataForge sites fit in 16 bytes"
);

impl RetainedLocation {
    pub(crate) fn pack_dataforge(&self) -> Option<PackedDataForgeLocation> {
        let coordinate = match self {
            Self::DataForge(stored) => PackedCoordinate::from_site(stored.site)?,
            Self::DataForgeField(stored) => PackedCoordinate::Field {
                record_index: stored.record_index,
                field_start: stored.field_start,
            },
            Self::Original(_) | Self::Xml(_) | Self::ReferenceSubresource(_) => return None,
        };
        Some(PackedDataForgeLocation { coordinate })
    }
}

impl PackedDataForgeLocation {
    /// Restores the coordinate with the immutable source-use ID it was packed beside.
    pub(crate) fn retain(&self, id: &SourceUseRef) -> RetainedLocation {
        let site = match self.coordinate {
            PackedCoordinate::Field {
                record_index,
                field_start,
            } => {
                return RetainedLocation::DataForgeField(StoredDataForgeField {
                    reference: id.clone(),
                    field_start,
                    record_index,
                    original: OnceLock::new(),
                });
            }
            PackedCoordinate::Header {
                record_index,
                field,
            } => DataForgeSite {
                coordinates: Coordinates::Header([u64::from(record_index)]),
                field: field.name(),
            },
            PackedCoordinate::RecordDeclarations { record_index } => DataForgeSite {
                coordinates: Coordinates::RecordDeclarations([u64::from(record_index)]),
                field: SURVEY_FIELD,
            },
            PackedCoordinate::Value {
                record_index,
                ordinal,
                field,
            } => DataForgeSite {
                coordinates: Coordinates::Value([u64::from(record_index), ordinal]),
                field: field.name(),
            },
        };
        RetainedLocation::DataForge(StoredDataForgeSite::new(site))
    }
}

impl PackedCoordinate {
    fn from_site(site: DataForgeSite) -> Option<Self> {
        match site.coordinates {
            Coordinates::Header([record_index]) => Some(Self::Header {
                record_index: u32::try_from(record_index).ok()?,
                field: HeaderField::of(site.field)?,
            }),
            Coordinates::RecordDeclarations([record_index]) => Some(Self::RecordDeclarations {
                record_index: u32::try_from(record_index).ok()?,
            }),
            Coordinates::Value([record_index, ordinal]) => Some(Self::Value {
                record_index: u32::try_from(record_index).ok()?,
                ordinal,
                field: ValueField::of(site.field)?,
            }),
        }
    }
}

#[cfg(test)]
#[path = "source_packed_dataforge_location_tests.rs"]
mod tests;
