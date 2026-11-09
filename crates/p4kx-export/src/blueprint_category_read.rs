// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, RecordTree};

use super::blueprint_category_definition::{BlueprintCategory, BlueprintCategoryDefinition};
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::CatalogFields;
use crate::catalog_table::CatalogTable;
use crate::guid_text::CatalogGuid;
use crate::source_fields::FieldError;

pub(crate) struct BlueprintCategoryRead<'source> {
    pub(crate) value: BlueprintCategory<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads the original category header and diagnoses every unexpected authored body field.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
) -> Result<BlueprintCategoryRead<'source>, CatalogError> {
    let mut notes = Vec::new();
    let context = projection.origins().record(tree.record_index());
    CatalogFields::new(tree.root(), "", &context).check(&[], &mut notes);
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
