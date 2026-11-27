// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceLocation;

use crate::catalog_read::original::OriginalValuePart;
use crate::record_header_field::RecordHeaderField;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReferenceSite {
    Value {
        record_index: u32,
        ordinal: u64,
        part: OriginalValuePart,
    },
    Header {
        record_index: u32,
        field: RecordHeaderField,
    },
}

impl ReferenceSite {
    fn location(self) -> SourceLocation {
        match self {
            Self::Value {
                record_index,
                ordinal,
                part,
            } => part.location(record_index, ordinal),
            Self::Header {
                record_index,
                field,
            } => field.location(record_index),
        }
    }

    fn matches(self, location: &SourceLocation) -> bool {
        let SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } = location
        else {
            return false;
        };
        match self {
            Self::Value {
                record_index,
                ordinal,
                part,
            } => {
                namespace == OriginalValuePart::NAMESPACE
                    && indices.as_slice() == [u64::from(record_index), ordinal]
                    && field == part.label()
            }
            Self::Header {
                record_index,
                field: header,
            } => {
                namespace == RecordHeaderField::NAMESPACE
                    && indices.as_slice() == [u64::from(record_index)]
                    && field == header.name()
            }
        }
    }
}

/// An exact coordinate stored until its Reference receipt is materialized.
#[derive(Debug, Clone)]
pub(super) enum ReferenceCoordinate {
    Site(ReferenceSite),
    Other(Box<SourceLocation>),
}

impl ReferenceCoordinate {
    pub(super) fn into_location(self) -> SourceLocation {
        match self {
            Self::Site(site) => site.location(),
            Self::Other(location) => *location,
        }
    }
}

impl PartialEq for ReferenceCoordinate {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Site(left), Self::Site(right)) => left == right,
            (Self::Other(left), Self::Other(right)) => left == right,
            (Self::Site(site), Self::Other(location))
            | (Self::Other(location), Self::Site(site)) => site.matches(location),
        }
    }
}

impl Eq for ReferenceCoordinate {}
