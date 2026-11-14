// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceUseRole, SourceValue, SourcedValue};

use crate::catalog_origins::CatalogOrigins;
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, record_fields};

/// Binds the actual name/default/prefix operation to its original header inputs.
pub(crate) fn class_name<'source>(
    tree: &RecordTree<'source>,
    origins: &CatalogOrigins,
) -> Result<SourcedValue<&'source str>, CatalogError> {
    let context = origins.record(tree.record_index());
    tree.metadata().name().fold(
        || context.header_selector(RecordHeaderField::RecordName, SourceValue::Present("")),
        || context.header_selector(RecordHeaderField::RecordName, SourceValue::Present("")),
        |_| {
            let inputs = [
                context.observe_header(RecordHeaderField::RecordName, SourceUseRole::Conversion)?,
                context.observe_header(RecordHeaderField::SourceType, SourceUseRole::Selector)?,
            ];
            Ok(origins.uses.value(
                SourceValue::Present(record_fields::class_name(tree)),
                &inputs,
                true,
            ))
        },
    )
}
