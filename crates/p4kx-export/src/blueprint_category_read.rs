// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::blueprint_category_definition::{BlueprintCategory, BlueprintCategoryDefinition};
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::CatalogFields;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

pub(crate) struct BlueprintCategoryRead<'source> {
    pub(crate) value: BlueprintCategory<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads the original category header and diagnoses every unexpected authored body field.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryRead<'source>, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let original = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&original, input, admission)?;
    let mut notes = Vec::new();
    let context = projection
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    CatalogFields::observed(tree.root(), "", &context, &capture)?
        .check_observed(&[], &mut notes)?;
    Ok(BlueprintCategoryRead {
        value: BlueprintCategory {
            guid: CatalogGuid(tree.guid()),
            definition: BlueprintCategoryDefinition {
                source: CanonicalRecordHeader::read(tree, projection.origins())?,
            },
        },
        notes,
    })
}

/// Materializes physical categories with independently measured source-family availability.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<BlueprintCategory<'source>>,
) -> CatalogTable<BlueprintCategoryDefinition<'source>> {
    CatalogTable::new(
        forge,
        &["BlueprintCategoryRecord"],
        rows.into_iter().map(|row| row.definition).collect(),
    )
}
