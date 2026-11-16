// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::OnceLock;

use serde::{Serialize, Serializer};

use super::{LocationView, SourceLocation, Subresource};

#[path = "source_xml_location_storage.rs"]
mod xml;

use self::xml::StoredXmlSite;

const VALUE_FIELDS: &[&str] = &[
    "$value",
    "$fieldName",
    "$dataType",
    "$isArray",
    "$target",
    "$type",
    "$error.offset",
    "$error.byte",
    "$error.structIndex",
    "$error.instanceIndex",
    "$error.first",
    "$error.count",
    "$error.len",
    "$error.guid",
];

const HEADER_FIELDS: &[&str] = &[
    "guid",
    "recordName",
    "recordIndex",
    "sourceType",
    "fileName",
    "metadataTag",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Coordinates {
    Header([u64; 1]),
    Value([u64; 2]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DataForgeSite {
    coordinates: Coordinates,
    field: &'static str,
}

impl DataForgeSite {
    fn from_original(location: &SourceLocation) -> Option<Self> {
        let SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } = location
        else {
            return None;
        };
        let (coordinates, known) = match (namespace.as_str(), indices.as_slice()) {
            ("dataforgeValue", [record, ordinal]) => {
                (Coordinates::Value([*record, *ordinal]), VALUE_FIELDS)
            }
            ("dataforgeRecordHeader", [record]) => (Coordinates::Header([*record]), HEADER_FIELDS),
            _ => return None,
        };
        let [record, ..] = indices.as_slice() else {
            return None;
        };
        u32::try_from(*record).ok()?;
        let field = known.iter().copied().find(|name| *name == field)?;
        Some(Self { coordinates, field })
    }

    const fn subresource(&self) -> Subresource<'_> {
        let (namespace, indices) = match &self.coordinates {
            Coordinates::Value(indices) => ("dataforgeValue", indices.as_slice()),
            Coordinates::Header(indices) => ("dataforgeRecordHeader", indices.as_slice()),
        };
        Subresource {
            namespace,
            indices,
            field: self.field,
        }
    }
}

#[derive(Debug)]
pub(crate) enum RetainedLocation {
    Original(SourceLocation),
    DataForge(StoredDataForgeSite),
    Xml(StoredXmlSite),
}

#[derive(Debug)]
pub(crate) struct StoredDataForgeSite {
    site: DataForgeSite,
    original: OnceLock<Box<SourceLocation>>,
}

impl StoredDataForgeSite {
    const fn new(site: DataForgeSite) -> Self {
        Self {
            site,
            original: OnceLock::new(),
        }
    }
}

impl RetainedLocation {
    pub(crate) fn new(location: SourceLocation) -> Self {
        if let Some(site) = DataForgeSite::from_original(&location) {
            return Self::DataForge(StoredDataForgeSite::new(site));
        }
        StoredXmlSite::from_original(location).map_or_else(Self::Original, Self::Xml)
    }

    pub(crate) fn view(&self) -> LocationView<'_> {
        match self {
            Self::Original(location) => LocationView::Original(location),
            Self::DataForge(stored) => LocationView::Subresource(stored.site.subresource()),
            Self::Xml(stored) => LocationView::Subresource(stored.subresource()),
        }
    }

    pub(crate) fn original(&self) -> &SourceLocation {
        match self {
            Self::Original(location) => location,
            Self::DataForge(stored) => stored
                .original
                .get_or_init(|| Box::new(stored.site.subresource().to_owned_location())),
            Self::Xml(stored) => stored.original(),
        }
    }

    pub(crate) const fn is_opaque(&self) -> bool {
        matches!(self, Self::Original(SourceLocation::Opaque { .. }))
    }

    #[cfg(test)]
    pub(crate) fn compact_is_materialized(&self) -> Option<bool> {
        match self {
            Self::Original(_) => None,
            Self::DataForge(stored) => Some(stored.original.get().is_some()),
            Self::Xml(stored) => Some(stored.is_materialized()),
        }
    }
}

impl Clone for RetainedLocation {
    fn clone(&self) -> Self {
        match self {
            Self::Original(location) => Self::Original(location.clone()),
            Self::DataForge(stored) => Self::DataForge(StoredDataForgeSite::new(stored.site)),
            Self::Xml(stored) => Self::Xml(stored.clone()),
        }
    }
}

impl PartialEq for RetainedLocation {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Original(left), Self::Original(right)) => left == right,
            (Self::DataForge(left), Self::DataForge(right)) => left.site == right.site,
            (Self::Xml(left), Self::Xml(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for RetainedLocation {}

impl Serialize for RetainedLocation {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.view() {
            LocationView::Original(location) => location.serialize(serializer),
            LocationView::Subresource(site) => site.serialize(serializer),
        }
    }
}

#[cfg(test)]
#[path = "source_location_storage_tests.rs"]
mod tests;
