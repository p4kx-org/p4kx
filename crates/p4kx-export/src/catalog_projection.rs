// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::sync::Mutex;

use p4kx_extraction::{
    Language, SourceLocation, SourceUse, SourceUseRole, SourceValue, SourcedValue,
    is_placeholder_key,
};

use super::catalog_text_forms;
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_text_contract::{CanonicalText, CatalogTexts, GameTextRef, PublicNameRef};
use crate::texts::{CATALOG_LANGUAGES, TextProjection, Texts};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Domain {
    PublicName,
    GameText,
}

struct Pending<'source> {
    key: &'source str,
    payloads: Vec<SourceUse>,
    selectors: Vec<SourceUse>,
    translations: BTreeMap<Language, Option<TextProjection<'source>>>,
    looked_up: bool,
}

/// Per-catalog projection inputs and canonical text rows with precise source uses.
pub(crate) struct CatalogProjection<'context, 'source> {
    texts: &'context Texts<'source>,
    origins: &'context CatalogOrigins,
    pending: Mutex<BTreeMap<(Domain, &'source str), Pending<'source>>>,
}

impl<'context, 'source> CatalogProjection<'context, 'source> {
    pub(crate) const fn new(
        texts: &'context Texts<'source>,
        origins: &'context CatalogOrigins,
    ) -> Self {
        Self {
            texts,
            origins,
            pending: Mutex::new(BTreeMap::new()),
        }
    }

    pub(crate) const fn legacy_texts(&self) -> &'context Texts<'source> {
        self.texts
    }

    pub(crate) const fn origins(&self) -> &'context CatalogOrigins {
        self.origins
    }

    pub(crate) fn name(
        &self,
        key: SourcedValue<&'source str>,
        record_index: u32,
        field: &str,
    ) -> Result<SourcedValue<PublicNameRef>, CatalogError> {
        key.try_map(|key| {
            self.register(Domain::PublicName, key, record_index, field)?;
            Ok(PublicNameRef(identifier(Domain::PublicName, key)))
        })
    }

    pub(crate) fn description(
        &self,
        key: SourcedValue<&'source str>,
        record_index: u32,
        field: &str,
    ) -> Result<SourcedValue<GameTextRef>, CatalogError> {
        key.try_map(|key| {
            self.register(Domain::GameText, key, record_index, field)?;
            Ok(GameTextRef(identifier(Domain::GameText, key)))
        })
    }

    fn register(
        &self,
        domain: Domain,
        key: &'source str,
        record_index: u32,
        field: &str,
    ) -> Result<(), CatalogError> {
        let payload = self.origins.dataforge.at(
            SourceLocation::DataForge {
                record_index,
                field: field.to_owned(),
            },
            SourceUseRole::Payload,
        )?;
        let selector = self.origins.dataforge.at(
            SourceLocation::DataForge {
                record_index,
                field: field.to_owned(),
            },
            SourceUseRole::Selector,
        )?;
        let placeholder = is_placeholder_key(key);
        if placeholder {
            self.texts.note_placeholder();
        }
        let translations = CATALOG_LANGUAGES
            .into_iter()
            .map(|language| {
                let text = if placeholder {
                    None
                } else {
                    self.texts.project(language, key)
                };
                (language, text)
            })
            .filter(|(language, _)| self.origins.locale(*language).is_some())
            .collect();
        let mut pending = crate::export_evidence_lock::lock(&self.pending)?;
        let row = pending.entry((domain, key)).or_insert_with(|| Pending {
            key,
            payloads: Vec::new(),
            selectors: Vec::new(),
            translations,
            looked_up: !placeholder,
        });
        row.payloads.push(payload);
        row.selectors.push(selector);
        drop(pending);
        Ok(())
    }

    pub(crate) fn finish(&self) -> Result<CatalogTexts<'source>, CatalogError> {
        let rows = std::mem::take(&mut *crate::export_evidence_lock::lock(&self.pending)?);
        let mut public_names = Vec::new();
        let mut game_texts = Vec::new();
        for ((domain, key), row) in rows {
            match domain {
                Domain::PublicName => {
                    public_names.push(self.row(row, PublicNameRef(identifier(domain, key)))?);
                }
                Domain::GameText => {
                    game_texts.push(self.row(row, GameTextRef(identifier(domain, key)))?);
                }
            }
        }
        Ok(CatalogTexts {
            public_names,
            game_texts,
        })
    }

    fn row<R>(
        &self,
        row: Pending<'source>,
        id: R,
    ) -> Result<CanonicalText<'source, R>, CatalogError> {
        let mut all_uses = row.selectors.clone();
        let mut translations = BTreeMap::new();
        for (language, text) in row.translations {
            let Some(input) = self.origins.locale(language) else {
                continue;
            };
            let projection = catalog_text_forms::project(
                &self.origins.uses,
                input,
                language,
                &row.selectors,
                text,
                row.looked_up,
            )?;
            all_uses.extend(projection.uses);
            translations.insert(language.code().to_owned(), projection.forms);
        }
        Ok(CanonicalText {
            id: self
                .origins
                .uses
                .value(SourceValue::Present(id), &row.payloads, true),
            key: self
                .origins
                .uses
                .value(SourceValue::Present(row.key), &row.payloads, true),
            translations: self.origins.uses.value(
                SourceValue::Present(translations),
                &all_uses,
                true,
            ),
        })
    }
}

fn identifier(domain: Domain, key: &str) -> String {
    let prefix = match domain {
        Domain::PublicName => "name",
        Domain::GameText => "text",
    };
    format!("{prefix}:{}:{key}", key.len())
}
