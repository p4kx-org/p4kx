// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree};

use super::contract::EntityTags;
use super::{fields, header};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::CatalogFields;
use crate::catalog_read::original::OriginalFlowFields;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(EntityTags<'source>, Vec<FieldError>), CatalogError> {
    let context = input.origins().record(tree.record_index());
    let scope = input.record_scope(admission.record(), tree)?;
    let original = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&original, input, admission)?;
    let fields = CatalogFields::observed(tree.root(), "", &context, &capture)?;
    let mut notes = Vec::new();
    let tags = fields::references(&fields, "tags", &mut notes, cancel)?;
    Ok((
        EntityTags {
            source: header::read(tree, input.origins())?,
            tags,
        },
        notes,
    ))
}
