// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceUses, SourceValue};

use crate::CatalogError;
use crate::catalog_read::original::{OriginalValueFact, OriginalValuePart};

use super::data_type::SourceDataType;
use super::field::{SourceDeclaration, SourceField};
use super::value::SourceValueRef;

pub(crate) fn read<'source>(
    site: &OriginalValueFact<'_, '_, 'source>,
    value: SourceValueRef,
    sources: &SourceUses,
) -> Result<SourceField<'source>, CatalogError> {
    let storage = site.field().storage();
    let data_type = SourceDataType::read(storage.data_type());
    let name = site
        .bind(
            OriginalValuePart::FieldName,
            SourceValue::Present(site.field().name()),
            SourceUseRole::Payload,
            true,
        )?
        .into_cell(sources);
    Ok(SourceField {
        name,
        declaration: SourceDeclaration {
            data_type: site
                .bind(
                    OriginalValuePart::DataType,
                    SourceValue::Present(data_type),
                    SourceUseRole::Selector,
                    true,
                )?
                .into_cell(sources),
            is_array: site
                .bind(
                    OriginalValuePart::IsArray,
                    SourceValue::Present(storage.is_array()),
                    SourceUseRole::Selector,
                    true,
                )?
                .into_cell(sources),
            target: site
                .bind(
                    OriginalValuePart::Target,
                    SourceValue::Present(storage.target()),
                    SourceUseRole::Selector,
                    true,
                )?
                .into_cell(sources),
        },
        value,
    })
}
