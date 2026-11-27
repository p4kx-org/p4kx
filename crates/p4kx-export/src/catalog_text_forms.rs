// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::{Language, SourceUse, SourceUseRole, SourceUses, SourceValue};

use super::catalog_origins::CatalogLocaleInput;
use super::catalog_text_contract::TextForms;
use crate::CatalogError;
use crate::texts::TextProjection;

/// Both language forms and their actual aggregate contributions.
pub(crate) struct TextFormProjection<'source> {
    pub(crate) forms: TextForms<'source>,
    pub(crate) uses: Vec<SourceUse>,
}

/// Records the language Entry consulted for this actual authored key selector.
pub(crate) fn lookup_use(
    input: &CatalogLocaleInput,
    language: Language,
    selector: &SourceUse,
    role: SourceUseRole,
) -> Result<SourceUse, CatalogError> {
    Ok(input
        .scope()
        .at_localization_lookup(language, selector, role)?)
}

/// Binds one prepared lookup to its exact key, locale and conversion inputs.
pub(crate) fn project<'source>(
    registry: &SourceUses,
    input: &CatalogLocaleInput,
    language: Language,
    selectors: &[SourceUse],
    text: Option<TextProjection<'source>>,
    looked_up: bool,
) -> Result<TextFormProjection<'source>, CatalogError> {
    let mut source_uses = selectors.to_vec();
    if looked_up {
        for selector in selectors {
            source_uses.push(lookup_use(
                input,
                language,
                selector,
                SourceUseRole::Selector,
            )?);
            if text.is_some() {
                source_uses.push(lookup_use(
                    input,
                    language,
                    selector,
                    SourceUseRole::Payload,
                )?);
            }
        }
    }
    let converts = looked_up && text.is_some();
    let display_only_conversion = converts && !input.decoded() && !selectors.is_empty();
    let mut display_uses = display_only_conversion.then(|| source_uses.clone());
    if converts {
        let conversions = display_uses.as_mut().unwrap_or(&mut source_uses);
        for selector in selectors {
            conversions.push(lookup_use(
                input,
                language,
                selector,
                SourceUseRole::Conversion,
            )?);
        }
    }
    Ok(TextFormProjection {
        forms: bind_forms(registry, text, &source_uses, display_uses.as_deref()),
        uses: display_uses.unwrap_or(source_uses),
    })
}

fn bind_forms<'source>(
    registry: &SourceUses,
    text: Option<TextProjection<'source>>,
    source_uses: &[SourceUse],
    display_uses: Option<&[SourceUse]>,
) -> TextForms<'source> {
    let text = text.map(|text| (Cow::Borrowed(text.source), text.display));
    let (source_text, display_text) = if let Some(display_uses) = display_uses {
        let (source, display) = text.unzip();
        (
            registry.value(
                source.map_or(SourceValue::Absent, SourceValue::Present),
                source_uses,
                true,
            ),
            registry.value(
                display.map_or(SourceValue::Absent, SourceValue::Present),
                display_uses,
                true,
            ),
        )
    } else {
        registry
            .value(
                text.map_or(SourceValue::Absent, SourceValue::Present),
                source_uses,
                true,
            )
            .unzip()
    };
    TextForms {
        source_text,
        display_text,
    }
}

#[cfg(test)]
#[path = "catalog_text_forms_tests.rs"]
mod tests;
