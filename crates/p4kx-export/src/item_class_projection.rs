// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, Language, SourceValue, SourceValueState, SourcedValue};

use super::inputs::unavailable_value;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::authored_fields::locale_input::AuthoredLocaleInput;
use crate::catalog_v3::catalog_text_forms;
use crate::item_class::{ClassLine, ItemClass};
use crate::{CatalogError, ExportAccountingError};

pub(super) struct ClassifiedItem {
    pub(super) value: SourcedValue<ItemClass>,
    pub(super) line: ClassLine,
}

pub(super) fn read(
    input: &AuthoredLocaleInput<'_>,
    projection: &CatalogProjection<'_, '_>,
) -> Result<ClassifiedItem, CatalogError> {
    let key = input.key();
    let Some(&token) = key.value().filter(|token| !token.is_empty()) else {
        return absent(key.state(), key.origin(), projection);
    };
    if p4kx_extraction::is_placeholder_key(token) {
        return absent(SourceValueState::Absent, key.origin(), projection);
    }
    let texts = projection.legacy_texts();
    if !texts.consulted(Language::English) {
        return Err(ExportAccountingError::MissingFieldInspection.into());
    }
    let uses = &projection.origins().uses;
    let Some(locale) = projection.origins().locale(Language::English) else {
        return missing_english(key, projection);
    };
    let selector = input
        .selector()
        .ok_or(ExportAccountingError::MissingFieldInspection)?;
    let projected = catalog_text_forms::project(
        uses,
        locale,
        Language::English,
        std::slice::from_ref(selector),
        texts.lookup_forms(Language::English, token),
        true,
    )?;
    let display = projected.forms.display_text;
    let line = ClassLine::read(display.value().map(AsRef::as_ref));
    let state = line
        .class()
        .map_or(SourceValue::Absent, SourceValue::Present);
    Ok(ClassifiedItem {
        value: uses.value_with_origins(state, &[key.origin(), display.origin()])?,
        line,
    })
}

pub(super) fn absent(
    state: SourceValueState,
    origin: &FieldOrigin,
    projection: &CatalogProjection<'_, '_>,
) -> Result<ClassifiedItem, CatalogError> {
    let state = unavailable_value(state)?.unwrap_or(SourceValue::Absent);
    Ok(ClassifiedItem {
        value: projection
            .origins()
            .uses
            .value_with_origins(state, &[origin])?,
        line: ClassLine::Missing,
    })
}

fn missing_english(
    key: &SourcedValue<&str>,
    projection: &CatalogProjection<'_, '_>,
) -> Result<ClassifiedItem, CatalogError> {
    let directory = projection
        .origins()
        .locale_directory()
        .map_or_default(std::slice::from_ref);
    let uses = &projection.origins().uses;
    let observed = uses.value(
        SourceValue::Absent,
        directory,
        projection.origins().locale_directory_complete(),
    );
    Ok(ClassifiedItem {
        value: uses.add_value_origins(observed, &[key.origin()])?,
        line: ClassLine::Missing,
    })
}
