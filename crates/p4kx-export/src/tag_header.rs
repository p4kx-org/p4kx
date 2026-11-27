// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordTree};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_origins::CatalogOrigins;
use crate::schema_fields::every_field_required;

/// Exact original Record identity shared by the tag datasets.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TagHeader<'source> {
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) physical: TagPhysicalSource<'source>,
    #[serde(flatten)]
    pub(crate) header: CanonicalRecordHeader<'source>,
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
    Ok(TagHeader {
        physical: TagPhysicalSource {
            guid: tree.guid(),
            record_index: tree.record_index(),
            record_path: tree.record_path(),
        },
        header: CanonicalRecordHeader::read(tree, origins)?,
    })
}
