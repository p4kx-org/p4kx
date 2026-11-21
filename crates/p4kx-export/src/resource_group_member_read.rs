// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourceValue, SourcedValue};

use super::contract::{ResourceGroupKey, ResourceGroupMembers};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::CatalogFields;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

/// Freezes both original membership arrays before the selected group map is constructed.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, '_>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(ResourceGroupMembers, Vec<FieldError>), CatalogError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let original = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&original, input, admission)?;
    let context = input
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let fields = CatalogFields::observed(tree.root(), "", &context, &capture)?;
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
                fields.reference_value(&format!("{name}[{ordinal}]"), Some(value), notes)
            })
            .collect::<Result<_, CatalogError>>()
    })
}
