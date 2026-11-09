// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourceValue, SourcedValue};

use super::contract::{ResourceGroupKey, ResourceGroupMembers};
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::{self, CatalogFields};
use crate::catalog_record_reference::CatalogRecordReference;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::source_fields::FieldError;

/// Freezes both original membership arrays before the selected group map is constructed.
pub(crate) fn read(
    tree: &RecordTree<'_>,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<(ResourceGroupMembers, Vec<FieldError>), CatalogError> {
    let context = origins.record(tree.record_index());
    let fields = CatalogFields::new(tree.root(), "", &context);
    let mut notes = Vec::new();
    let groups = references(&fields, "groups", &mut notes, cancel)?;
    let resources = references(&fields, "resources", &mut notes, cancel)?;
    Ok((
        ResourceGroupMembers {
            physical: ResourceGroupKey {
                guid: CatalogGuid(tree.guid()),
                record_index: tree.record_index(),
            },
            guid: context.header_value(
                RecordHeaderField::Guid,
                SourceValue::Present(CatalogGuid(tree.guid())),
            )?,
            record_index: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(tree.record_index()),
            )?,
            groups,
            resources,
        },
        notes,
    ))
}

fn references(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
    cancel: Option<&CancelToken>,
) -> Result<SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>, CatalogError> {
    fields.array(name, notes)?.try_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(ordinal, value)| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                catalog_read::reference_value(
                    fields.context(),
                    Some(value),
                    fields.source().type_name(),
                    &format!("{name}[{ordinal}]"),
                    notes,
                )
            })
            .collect::<Result<_, CatalogError>>()
    })
}
