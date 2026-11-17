// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordText, RecordTree, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::{Serialize, Serializer};

use crate::catalog_origins::CatalogOrigins;
use crate::guid_text::{CatalogGuid, GuidText};
use crate::record_header_field::RecordHeaderField;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, source_header};

/// Independently sourced original metadata and stable convenience values of one physical Record.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CanonicalRecordHeader<'source> {
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_name: SourcedValue<&'source str>,
    pub(crate) class_name: SourcedValue<&'source str>,
    pub(crate) record_path: SourcedValue<&'source str>,
    pub(crate) record_index: SourcedValue<u32>,
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) file_name: SourcedValue<&'source str>,
    pub(crate) metadata_tag: SourcedValue<&'source str>,
}

impl<'source> CanonicalRecordHeader<'source> {
    pub(crate) fn read(
        tree: &RecordTree<'source>,
        origins: &CatalogOrigins,
    ) -> Result<Self, CatalogError> {
        let context = origins.record(tree.record_index());
        let metadata = tree.metadata();
        Ok(Self {
            guid: context.header_value(
                RecordHeaderField::Guid,
                SourceValue::Present(CatalogGuid(tree.guid())),
            )?,
            record_name: context
                .header_value(RecordHeaderField::RecordName, text_value(metadata.name()))?,
            class_name: source_header::class_name(tree, origins)?,
            record_path: context.header_conversion(
                RecordHeaderField::FileName,
                SourceValue::Present(tree.record_path()),
            )?,
            record_index: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(tree.record_index()),
            )?,
            source_type: context.header_selector(
                RecordHeaderField::SourceType,
                SourceValue::Present(tree.type_name()),
            )?,
            file_name: context.header_value(
                RecordHeaderField::FileName,
                SourceValue::Present(metadata.file_name()),
            )?,
            metadata_tag: context.header_value(
                RecordHeaderField::MetadataTag,
                text_value(metadata.metadata_tag()),
            )?,
        })
    }
}

impl<'source> CanonicalRecordHeader<'source> {
    /// The original Record GUID with its source evidence.
    #[must_use]
    pub fn guid(&self) -> SourcedValue<Guid> {
        self.guid.map_ref(|guid| guid.0)
    }

    /// The original Record name.
    #[must_use]
    pub const fn record_name(&self) -> &SourcedValue<&'source str> {
        &self.record_name
    }

    /// The normalized class name.
    #[must_use]
    pub const fn class_name(&self) -> &SourcedValue<&'source str> {
        &self.class_name
    }

    /// The normalized Record path.
    #[must_use]
    pub const fn record_path(&self) -> &SourcedValue<&'source str> {
        &self.record_path
    }

    /// The physical Record index.
    #[must_use]
    pub const fn record_index(&self) -> &SourcedValue<u32> {
        &self.record_index
    }

    /// The declared Record type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// The original file name.
    #[must_use]
    pub const fn file_name(&self) -> &SourcedValue<&'source str> {
        &self.file_name
    }

    /// The original metadata tag.
    #[must_use]
    pub const fn metadata_tag(&self) -> &SourcedValue<&'source str> {
        &self.metadata_tag
    }
}

fn text_value(value: RecordText<'_>) -> SourceValue<&str> {
    value.fold(
        || SourceValue::Absent,
        || SourceValue::Null,
        SourceValue::Present,
    )
}

/// The physical Record identity captured independently from the body.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct CatalogRecordSource {
    /// The Record GUID.
    #[serde(serialize_with = "serialize_guid")]
    #[schemars(with = "SourcedValue<CatalogGuid>")]
    pub guid: SourcedValue<Guid>,
    /// The original Record name.
    pub record_name: SourcedValue<String>,
    /// The normalized class name.
    pub class_name: SourcedValue<String>,
    /// The normalized Record path.
    pub record_path: SourcedValue<String>,
    /// The physical Record index.
    pub record_index: SourcedValue<u32>,
    /// The declared Record type.
    pub source_type: SourcedValue<String>,
    /// The original file name.
    pub file_name: SourcedValue<String>,
    /// The original metadata tag.
    pub metadata_tag: SourcedValue<String>,
}

pub(crate) fn serialize_guid<S: Serializer>(
    value: &SourcedValue<Guid>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    value
        .map_ref(|guid| CatalogGuid(*guid))
        .serialize(serializer)
}

impl CatalogRecordSource {
    pub(crate) fn order_key(&self) -> (Option<GuidText>, Option<u32>) {
        (
            self.guid.value().map(|guid| CatalogGuid(*guid).text()),
            self.record_index.value().copied(),
        )
    }
}

impl From<CanonicalRecordHeader<'_>> for CatalogRecordSource {
    fn from(source: CanonicalRecordHeader<'_>) -> Self {
        Self {
            guid: source.guid.map(|guid| guid.0),
            record_name: source.record_name.map(str::to_owned),
            class_name: source.class_name.map(str::to_owned),
            record_path: source.record_path.map(str::to_owned),
            record_index: source.record_index,
            source_type: source.source_type.map(str::to_owned),
            file_name: source.file_name.map(str::to_owned),
            metadata_tag: source.metadata_tag.map(str::to_owned),
        }
    }
}
