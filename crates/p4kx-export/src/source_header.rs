// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceLocation, SourceUseRole, SourceValue, SourcedValue};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::record_fields;

/// Binds the type-prefix removal to the exact original Record header inputs.
pub(crate) fn class_name<'source>(
    tree: &RecordTree<'source>,
    origins: &CatalogOrigins,
) -> Result<SourcedValue<&'source str>, CatalogError> {
    let location = |field: &str| SourceLocation::DataForge {
        record_index: tree.record_index(),
        field: field.to_owned(),
    };
    let inputs = [
        origins
            .dataforge
            .at(location("$name"), SourceUseRole::Conversion)?,
        origins
            .dataforge
            .at(location("$type"), SourceUseRole::Selector)?,
    ];
    Ok(origins.uses.value(
        SourceValue::Present(record_fields::class_name(tree)),
        &inputs,
        true,
    ))
}
