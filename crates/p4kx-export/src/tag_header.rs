// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordTree, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// Exact original Record identity shared by the tag datasets.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TagHeader<'source> {
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) physical: TagPhysicalSource<'source>,
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_name: SourcedValue<&'source str>,
    pub(crate) class_name: SourcedValue<&'source str>,
    pub(crate) record_path: SourcedValue<&'source str>,
    pub(crate) record_index: SourcedValue<u32>,
    pub(crate) source_type: SourcedValue<&'source str>,
}

/// The original canonical source key used for sorting and emitted-row accounting.
#[derive(Debug)]
pub(crate) struct TagPhysicalSource<'source> {
    pub(crate) guid: Guid,
    pub(crate) record_index: u32,
    pub(crate) record_path: &'source str,
}

pub(super) fn read<'source>(
    tree: &RecordTree<'source>,
    origins: &CatalogOrigins,
) -> Result<TagHeader<'source>, CatalogError> {
    let context = origins.record(tree.record_index());
    Ok(TagHeader {
        physical: TagPhysicalSource {
            guid: tree.guid(),
            record_index: tree.record_index(),
            record_path: tree.record_path(),
        },
        guid: context.value("$guid", SourceValue::Present(CatalogGuid(tree.guid())))?,
        record_name: context.value("$name", SourceValue::Present(tree.name()))?,
        class_name: crate::source_header::class_name(tree, origins)?,
        record_path: context.value("$recordPath", SourceValue::Present(tree.record_path()))?,
        record_index: context
            .selector("$recordIndex", SourceValue::Present(tree.record_index()))?,
        source_type: context.selector("$type", SourceValue::Present(tree.type_name()))?,
    })
}
