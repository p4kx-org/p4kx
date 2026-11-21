// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::SourceValue;

use super::{
    CatalogProjection, Domain, FinishedTexts, KindTextSummary, Pending, RegistryState, identifier,
};
use crate::CatalogError;
use crate::catalog_text_contract::{
    CanonicalText, CatalogLanguagePolicy, CatalogTexts, GameTextRef, PublicNameRef,
    language_sources,
};
use crate::catalog_v3::catalog_text_forms;
use crate::texts::CATALOG_LANGUAGES;

pub(crate) struct CanonicalRows<'source> {
    pub(crate) public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    pub(crate) game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    kind_coverage: Vec<KindTextSummary>,
}

impl<'source> CatalogProjection<'_, 'source> {
    pub(crate) fn finish(&self) -> Result<FinishedTexts<'source>, CatalogError> {
        let rows = self.finish_rows()?;
        Ok(FinishedTexts {
            texts: CatalogTexts {
                language_policy: CatalogLanguagePolicy::AVAILABLE_SUPPORTED,
                language_sources: language_sources(
                    self.texts,
                    &self.origins().uses,
                    self.origins().locale_origins(),
                    &CATALOG_LANGUAGES,
                )?,
                public_names: rows.public_names,
                game_texts: rows.game_texts,
            },
            kind_coverage: rows.kind_coverage,
        })
    }

    pub(crate) fn finish_public_names(
        &self,
    ) -> Result<Vec<CanonicalText<'source, PublicNameRef>>, CatalogError> {
        Ok(self.finish_rows()?.public_names)
    }

    pub(crate) fn finish_rows(&self) -> Result<CanonicalRows<'source>, CatalogError> {
        let RegistryState {
            pending: rows,
            coverage,
        } = std::mem::take(&mut *crate::export_evidence_lock::lock(
            &self.registry.state,
        )?);
        let mut public_names = Vec::new();
        let mut game_texts = Vec::new();
        for ((domain, key), row) in rows {
            match domain {
                Domain::PublicName => {
                    public_names.push(self.row(&row, PublicNameRef(identifier(domain, key)))?);
                }
                Domain::GameText => {
                    game_texts.push(self.row(&row, GameTextRef(identifier(domain, key)))?);
                }
            }
        }
        Ok(CanonicalRows {
            public_names,
            game_texts,
            kind_coverage: coverage.finish(),
        })
    }

    fn row<R>(
        &self,
        row: &Pending<'source>,
        id: R,
    ) -> Result<CanonicalText<'source, R>, CatalogError> {
        let uses = &self.origins().uses;
        let mut all_uses = row.selectors.clone();
        let mut translations = BTreeMap::new();
        let looked_up = row.lookup.was_projected();
        for language in CATALOG_LANGUAGES {
            let Some(input) = self.origins().locale(language) else {
                continue;
            };
            let text = if looked_up {
                self.texts.lookup_forms(language, row.key)
            } else {
                None
            };
            let projection = catalog_text_forms::project(
                uses,
                input,
                language,
                &row.selectors,
                text,
                looked_up,
            )?;
            all_uses.extend(projection.uses);
            translations.insert(language.code().to_owned(), projection.forms);
        }
        let (id, key) = uses
            .value(SourceValue::Present((id, row.key)), &row.payloads, true)
            .unzip();
        Ok(CanonicalText {
            id,
            key,
            translations: uses.value(SourceValue::Present(translations), &all_uses, true),
        })
    }
}
