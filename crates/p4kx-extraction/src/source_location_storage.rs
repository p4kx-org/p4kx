// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::OnceLock;

use serde::{Serialize, Serializer};

use super::location::DataForgeField;
use super::{LocationView, SourceLocation, SourceUseRef, Subresource};

#[path = "source_packed_dataforge_location.rs"]
mod packed_dataforge_location;
pub(crate) use self::packed_dataforge_location::PackedDataForgeLocation;

#[path = "source_reference_subresource_storage.rs"]
mod reference_subresource;
#[path = "source_xml_location_storage.rs"]
mod xml;

use self::reference_subresource::StoredReferenceSubresource;
use self::xml::StoredXmlSite;

const _: () = assert!(
    size_of::<StoredReferenceSubresource>() <= size_of::<StoredXmlSite>(),
    "short subresource storage fits the existing XML location footprint"
);

const SURVEY_FIELD: &str = "$fields";
const DECLARATION_FIELDS: &[&str] = &[SURVEY_FIELD];

const VALUE_FIELDS: &[&str] = &[
    "$value",
    SURVEY_FIELD,
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
    RecordDeclarations([u64; 1]),
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
            ("dataforgeRecordDeclarations", [record]) => (
                Coordinates::RecordDeclarations([*record]),
                DECLARATION_FIELDS,
            ),
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
            Coordinates::RecordDeclarations(indices) => {
                ("dataforgeRecordDeclarations", indices.as_slice())
            }
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
    DataForgeField(StoredDataForgeField),
    Xml(StoredXmlSite),
    ReferenceSubresource(StoredReferenceSubresource),
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

#[derive(Debug)]
pub(crate) struct StoredDataForgeField {
    reference: SourceUseRef,
    field_start: usize,
    record_index: u32,
    original: OnceLock<Box<SourceLocation>>,
}

impl StoredDataForgeField {
    fn from_reference(field: DataForgeField<'_>, reference: &SourceUseRef) -> Option<Self> {
        let prefix = reference.as_str().strip_suffix(field.field)?;
        Some(Self {
            reference: reference.clone(),
            field_start: prefix.len(),
            record_index: field.record_index,
            original: OnceLock::new(),
        })
    }

    fn site(&self) -> DataForgeField<'_> {
        // The offset is a verified UTF-8 boundary in this immutable reference.
        let (_, field) = self.reference.as_str().split_at(self.field_start);
        DataForgeField {
            record_index: self.record_index,
            field,
        }
    }
}

impl Clone for StoredDataForgeField {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            field_start: self.field_start,
            record_index: self.record_index,
            original: OnceLock::new(),
        }
    }
}

impl RetainedLocation {
    pub(crate) fn new(
        location: SourceLocation,
        compact_reference: Option<&SourceUseRef>,
        full_reference: Option<(&SourceUseRef, usize)>,
    ) -> Self {
        if let (
            SourceLocation::DataForge {
                record_index,
                field,
            },
            Some(reference),
        ) = (&location, compact_reference)
            && let Some(stored) = StoredDataForgeField::from_reference(
                DataForgeField {
                    record_index: *record_index,
                    field,
                },
                reference,
            )
        {
            return Self::DataForgeField(stored);
        }
        if let Some(site) = DataForgeSite::from_original(&location) {
            return Self::DataForge(StoredDataForgeSite::new(site));
        }
        let location = match StoredXmlSite::from_original(location) {
            Ok(stored) => return Self::Xml(stored),
            Err(original) => original,
        };
        if let Some((reference, start)) = full_reference
            && let Some(stored) =
                StoredReferenceSubresource::from_original(&location, reference, start)
        {
            return Self::ReferenceSubresource(stored);
        }
        Self::Original(location)
    }

    pub(crate) fn for_reference(
        &self,
        reference: &SourceUseRef,
        full_start: Option<usize>,
    ) -> Self {
        if let Self::DataForgeField(stored) = self
            && let Some(stored) = StoredDataForgeField::from_reference(stored.site(), reference)
        {
            return Self::DataForgeField(stored);
        }
        if let Self::ReferenceSubresource(stored) = self
            && let Some(start) = full_start
        {
            let site = stored.subresource();
            return StoredReferenceSubresource::new(&site, reference, start).map_or_else(
                || Self::Original(site.to_owned_location()),
                Self::ReferenceSubresource,
            );
        }
        self.clone()
    }

    pub(crate) fn view(&self) -> LocationView<'_> {
        match self {
            Self::Original(location) => LocationView::Original(location),
            Self::DataForge(stored) => LocationView::Subresource(stored.site.subresource()),
            Self::DataForgeField(stored) => LocationView::DataForgeField(stored.site()),
            Self::Xml(stored) => LocationView::Subresource(stored.subresource()),
            Self::ReferenceSubresource(stored) => LocationView::Subresource(stored.subresource()),
        }
    }

    pub(crate) fn original(&self) -> &SourceLocation {
        match self {
            Self::Original(location) => location,
            Self::DataForge(stored) => stored
                .original
                .get_or_init(|| Box::new(stored.site.subresource().to_owned_location())),
            Self::DataForgeField(stored) => stored
                .original
                .get_or_init(|| Box::new(stored.site().to_owned_location())),
            Self::Xml(stored) => stored.original(),
            Self::ReferenceSubresource(stored) => stored.original(),
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
            Self::DataForgeField(stored) => Some(stored.original.get().is_some()),
            Self::Xml(stored) => Some(stored.is_materialized()),
            Self::ReferenceSubresource(stored) => Some(stored.is_materialized()),
        }
    }
}

impl Clone for RetainedLocation {
    fn clone(&self) -> Self {
        match self {
            Self::Original(location) => Self::Original(location.clone()),
            Self::DataForge(stored) => Self::DataForge(StoredDataForgeSite::new(stored.site)),
            Self::DataForgeField(stored) => Self::DataForgeField(stored.clone()),
            Self::Xml(stored) => Self::Xml(stored.clone()),
            Self::ReferenceSubresource(stored) => Self::ReferenceSubresource(stored.clone()),
        }
    }
}

impl PartialEq for RetainedLocation {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Original(left), Self::Original(right)) => left == right,
            (Self::DataForge(left), Self::DataForge(right)) => left.site == right.site,
            (Self::DataForgeField(left), Self::DataForgeField(right)) => {
                left.site() == right.site()
            }
            (Self::Xml(left), Self::Xml(right)) => left == right,
            (Self::ReferenceSubresource(left), Self::ReferenceSubresource(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for RetainedLocation {}

impl Serialize for RetainedLocation {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.view() {
            LocationView::Original(location) => location.serialize(serializer),
            LocationView::DataForgeField(site) => site.serialize(serializer),
            LocationView::Subresource(site) => site.serialize(serializer),
        }
    }
}

#[cfg(test)]
#[path = "source_location_storage_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "source_declaration_site_tests.rs"]
mod declaration_tests;
