// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Record, RecordTree};

use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::NumberCapture;
use crate::catalog_projection::CatalogProjection;
use crate::guid_text::CatalogGuid;

use super::definition::LootFilterDefinition;
use crate::source_value_catalog;

/// Reads every original field and value once without following Reference target bodies.
pub(crate) fn read<'source>(
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<LootFilterDefinition<'source>, CatalogError> {
    let scope = projection.original().record_scope(record, tree)?;
    let source = CanonicalRecordHeader::read(tree, projection.origins())?;
    let numbers = NumberCapture::default();
    let output =
        source_value_catalog::read_arena(&scope, cancel, |site| numbers.retain_loot(site))?;
    Ok(LootFilterDefinition {
        physical_guid: CatalogGuid(record.guid()),
        physical_path: tree.record_path(),
        numbers: numbers.finish(),
        source,
        fields: output.fields,
        values: output.values,
    })
}
