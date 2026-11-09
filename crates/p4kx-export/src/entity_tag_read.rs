// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree};

use super::contract::EntityTags;
use super::{fields, header};
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::CatalogFields;
use crate::source_fields::FieldError;

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<(EntityTags<'source>, Vec<FieldError>), CatalogError> {
    let context = origins.record(tree.record_index());
    let fields = CatalogFields::new(tree.root(), "", &context);
    let mut notes = Vec::new();
    let tags = fields::references(&fields, "tags", &mut notes, cancel)?;
    Ok((
        EntityTags {
            source: header::read(tree, origins)?,
            tags,
        },
        notes,
    ))
}
