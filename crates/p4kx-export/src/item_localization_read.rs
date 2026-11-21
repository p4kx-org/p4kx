// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourcedValue};

use super::ItemLocalization;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::authored_fields::Observed;
use crate::source_fields::FieldError;

pub(super) fn read<'source>(
    source: Observed<'_, '_, '_, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<ItemLocalization<'source>>, CatalogError> {
    let retain = |source: Observed<'_, '_, '_, 'source>, notes: &mut Vec<FieldError>| {
        let (name, name_input) = source.public_name_input("Name", projection, notes)?;
        let (description, description_input) =
            source.protected_locale_input("Description", projection, notes)?;
        Ok(ItemLocalization {
            name,
            description,
            name_input,
            description_input,
        })
    };
    if source.storage_type("Localization")? == Some(DataType::StrongPointer) {
        source.declared_pointer("Localization", notes, retain)
    } else {
        source.declared_class("Localization", notes, retain)
    }
}
