// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::{self, CatalogFields};
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::blueprint_category_list::{
    BlueprintCategoryList, PreparedBlueprintCategoryList,
};
use crate::guid_text::CatalogGuid;
use crate::source_fields::FieldError;

/// A readable physical list and the exact field coverage diagnoses.
pub(crate) struct BlueprintCategoryListRead<'source> {
    pub(crate) value: PreparedBlueprintCategoryList<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads one database without changing its authored member order or source states.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryListRead<'source>, CatalogError> {
    let mut notes = Vec::new();
    let context = projection
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let fields = CatalogFields::new(tree.root(), "", &context);
    fields.check(&["categories"], &mut notes);
    let categories = fields.array("categories", &mut notes)?.try_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(ordinal, value)| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                catalog_read::reference_value(
                    &context,
                    Some(value),
                    fields.source().type_name(),
                    &format!("categories[{ordinal}]"),
                    &mut notes,
                )
            })
            .collect::<Result<Vec<_>, CatalogError>>()
    })?;
    Ok(BlueprintCategoryListRead {
        value: PreparedBlueprintCategoryList {
            guid: CatalogGuid(tree.guid()),
            declaration: BlueprintCategoryList {
                source: CanonicalRecordHeader::read(tree, projection.origins())?,
                categories,
            },
        },
        notes,
    })
}

/// Materializes the declared database family independently of selected row count.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedBlueprintCategoryList<'source>>,
) -> CatalogTable<BlueprintCategoryList<'source>> {
    CatalogTable::new(
        forge,
        &["BlueprintCategoryDatabaseRecord"],
        rows.into_iter().map(|row| row.declaration).collect(),
    )
}
