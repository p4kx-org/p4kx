// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordText, RecordTree, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_origins::CatalogOrigins;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, source_header};

/// Independently sourced original metadata and stable convenience values of one physical Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CanonicalRecordHeader<'source> {
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

fn text_value(value: RecordText<'_>) -> SourceValue<&str> {
    value.fold(
        || SourceValue::Absent,
        || SourceValue::Null,
        SourceValue::Present,
    )
}
