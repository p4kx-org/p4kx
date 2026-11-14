// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Value, is_placeholder_key};

use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalValueFact;
use crate::template_context_value::{LocalizationMeaning, TemplateLocalization};
use crate::template_source_members::TemplateMembers;

pub(crate) fn read<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<TemplateLocalization, CatalogError> {
    let interpretation = match fact.value() {
        Value::String("") | Value::Locale("") => LocalizationMeaning::Empty,
        Value::String(key) | Value::Locale(key) if is_placeholder_key(key) => {
            LocalizationMeaning::Placeholder
        }
        Value::String(key) | Value::Locale(key) if key.starts_with('@') => {
            LocalizationMeaning::Lookup
        }
        Value::String(_) | Value::Locale(_) => LocalizationMeaning::NotKey,
        Value::Null => LocalizationMeaning::Null,
        Value::Error(_) => LocalizationMeaning::Unreadable,
        _ => LocalizationMeaning::WrongShape,
    };
    Ok(TemplateLocalization {
        source: members.value_ref(fact)?,
        interpretation,
        text: projection.description_fact(fact)?,
    })
}
