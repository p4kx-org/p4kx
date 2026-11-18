// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, RecordTree, SourceValueState};

use super::item_definition::ItemDefinition;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<ItemDefinition<'source>>, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let mut notes = Vec::new();
    let row = ItemDefinition::read(tree, &originals, &capture, projection, &numbers, &mut notes)?;
    let components = row.components();
    let selection_complete = matches!(
        components.state(),
        SourceValueState::Present | SourceValueState::Absent
    ) && components.origin().source_set_complete();
    Ok(ScopedSourceRead {
        value: row,
        notes,
        selection_complete,
    })
}
