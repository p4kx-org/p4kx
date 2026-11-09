// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::{
    Language, SourceLocation, SourceUse, SourceUseRole, SourceUses, SourceValue,
};

use super::catalog_origins::CatalogLocaleInput;
use super::catalog_text_contract::TextForms;
use crate::CatalogError;
use crate::texts::TextProjection;

/// Both language forms and their actual aggregate contributions.
pub(crate) struct TextFormProjection<'source> {
    pub(crate) forms: TextForms<'source>,
    pub(crate) uses: Vec<SourceUse>,
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
            let location = SourceLocation::LocalizationLookup {
                language,
                selector: selector.reference().clone(),
            };
            let payload_location = text.as_ref().map(|_| location.clone());
            source_uses.push(input.scope().at(location, SourceUseRole::Selector)?);
            if let Some(location) = payload_location {
                source_uses.push(input.scope().at(location, SourceUseRole::Payload)?);
            }
        }
    }
    let mut display_uses = source_uses.clone();
    if looked_up && text.is_some() {
        for selector in selectors {
            let conversion = input.scope().at(
                SourceLocation::LocalizationLookup {
                    language,
                    selector: selector.reference().clone(),
                },
                SourceUseRole::Conversion,
            )?;
            if input.decoded() {
                source_uses.push(conversion.clone());
            }
            display_uses.push(conversion);
        }
    }
    let source = text.as_ref().map_or(SourceValue::Absent, |text| {
        SourceValue::Present(Cow::Borrowed(text.source))
    });
    let display = text.map_or(SourceValue::Absent, |text| {
        SourceValue::Present(text.display)
    });
    Ok(TextFormProjection {
        forms: TextForms {
            source_text: registry.value(source, &source_uses, true),
            display_text: registry.value(display, &display_uses, true),
        },
        uses: display_uses,
    })
}
