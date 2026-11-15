// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::sync::Mutex;

use p4kx_extraction::{
    Language, SourceLocation, SourceUse, SourceUseRole, SourceValue, SourcedValue,
    is_placeholder_key,
};

#[path = "catalog_protected_key.rs"]
mod protected_key;
pub(crate) use protected_key::ProtectedKey;

use super::catalog_text_forms;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::catalog_text_contract::{
    CanonicalText, CatalogLanguagePolicy, CatalogTexts, GameTextRef, PublicNameRef,
    language_sources,
};
use crate::mission_text_inputs::{MissionKey, SelectedKey};
use crate::texts::{CATALOG_LANGUAGES, TextProjection, Texts};

type Translations<'source> = BTreeMap<Language, Option<TextProjection<'source>>>;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Domain {
    PublicName,
    GameText,
}

struct Pending<'source> {
    key: &'source str,
    payloads: Vec<SourceUse>,
    selectors: Vec<SourceUse>,
    translations: Translations<'source>,
    looked_up: bool,
}

/// Per-catalog projection inputs and canonical text rows with precise source uses.
pub(crate) struct CatalogProjection<'context, 'source> {
    texts: &'context Texts<'source>,
    original: &'context OriginalCatalogInput<'context, 'source>,
    pending: Mutex<BTreeMap<(Domain, &'source str), Pending<'source>>>,
}

impl<'context, 'source> CatalogProjection<'context, 'source> {
    pub(crate) const fn new(
        texts: &'context Texts<'source>,
        original: &'context OriginalCatalogInput<'context, 'source>,
    ) -> Self {
        Self {
            texts,
            original,
            pending: Mutex::new(BTreeMap::new()),
        }
    }

    pub(crate) const fn original(&self) -> &'context OriginalCatalogInput<'context, 'source> {
        self.original
    }

    pub(crate) const fn legacy_texts(&self) -> &'context Texts<'source> {
        self.texts
    }

    pub(crate) const fn origins(&self) -> &'context CatalogOrigins {
        self.original.origins()
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

    /// Retains the exact authored Locale occurrence in the public short-name domain.
    pub(crate) fn public_name_fact(
        &self,
        fact: &OriginalValueFact<'_, '_, 'source>,
    ) -> Result<PublicNameRef, CatalogError> {
        self.locale_fact(Domain::PublicName, fact)
            .map(PublicNameRef)
    }

    /// Retains an authored Locale occurrence in the protected text domain.
    pub(crate) fn protected_locale_fact(
        &self,
        fact: &OriginalValueFact<'_, '_, 'source>,
    ) -> Result<GameTextRef, CatalogError> {
        self.locale_fact(Domain::GameText, fact).map(GameTextRef)
    }

    fn locale_fact(
        &self,
        domain: Domain,
        fact: &OriginalValueFact<'_, '_, 'source>,
    ) -> Result<String, CatalogError> {
        use crate::catalog_read::original::OriginalValuePart;
        use p4kx_extraction::Value;
        fact.check_input(self.original)?;
        let &Value::Locale(key) = fact.value() else {
            return Err(crate::ExportAccountingError::ConflictingSourceDatum.into());
        };
        let payload = fact.observe(OriginalValuePart::Value, SourceUseRole::Payload)?;
        let selector = fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?;
        self.register_uses(domain, key, payload, selector)?;
        Ok(identifier(domain, key))
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

    /// Reads a protected key at an exact original Struct field.
    pub(crate) fn protected_key_field<'scope, 'read>(
        &self,
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        owner: Option<&OriginalValueFact<'scope, 'read, 'source>>,
        name: &str,
    ) -> Result<SourcedValue<GameTextRef>, CatalogError> {
        match fields.field(owner, name)? {
            Some((_, fact)) => self.description_fact(fact),
            None => Ok(fields
                .sources()
                .value(SourceValue::Absent, &[fields.survey(owner)?], true)),
        }
    }

    /// Projects one actual original key through the existing canonical `GameText` registry.
    pub(crate) fn description_fact(
        &self,
        fact: &OriginalValueFact<'_, '_, 'source>,
    ) -> Result<SourcedValue<GameTextRef>, CatalogError> {
        fact.check_input(self.original)?;
        self.finish_protected_key(ProtectedKey::from_fact(fact)?)
    }

    /// Registers a retained protected lookup and preserves the original field evidence.
    pub(crate) fn finish_protected_key(
        &self,
        key: ProtectedKey<'source>,
    ) -> Result<SourcedValue<GameTextRef>, CatalogError> {
        key.map(&self.origins().uses, |key, payload, selector| {
            self.register_uses(Domain::GameText, key, payload, selector)?;
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
        let payload = self.origins().dataforge.at(
            SourceLocation::DataForge {
                record_index,
                field: field.to_owned(),
            },
            SourceUseRole::Payload,
        )?;
        let selector = self.origins().dataforge.at(
            SourceLocation::DataForge {
                record_index,
                field: field.to_owned(),
            },
            SourceUseRole::Selector,
        )?;
        self.register_uses(domain, key, payload, selector)
    }

    /// Uses one original mission-key lookup for the canonical text row and the rendered value.
    pub(crate) fn mission_key<T>(
        &self,
        key: &MissionKey<'source>,
        render: impl FnOnce(&Translations<'source>, &SelectedKey) -> Result<T, CatalogError>,
    ) -> Result<(GameTextRef, T), CatalogError> {
        let observed = key.retain(self.origins())?;
        let translations = self.translations(key.text, false);
        let value = render(&translations, &observed)?;
        self.insert_pending(
            Domain::GameText,
            key.text,
            observed.payload,
            observed.selector,
            translations,
            true,
        )?;
        Ok((GameTextRef(identifier(Domain::GameText, key.text)), value))
    }

    fn register_uses(
        &self,
        domain: Domain,
        key: &'source str,
        payload: SourceUse,
        selector: SourceUse,
    ) -> Result<(), CatalogError> {
        let placeholder = is_placeholder_key(key);
        if placeholder {
            self.texts.note_placeholder();
        }
        let translations = self.translations(key, placeholder);
        self.insert_pending(domain, key, payload, selector, translations, !placeholder)
    }

    fn translations(&self, key: &str, placeholder: bool) -> Translations<'source> {
        CATALOG_LANGUAGES
            .into_iter()
            .map(|language| {
                let text = if placeholder {
                    None
                } else {
                    self.texts.project(language, key)
                };
                (language, text)
            })
            .filter(|(language, _)| self.origins().locale(*language).is_some())
            .collect()
    }

    fn insert_pending(
        &self,
        domain: Domain,
        key: &'source str,
        payload: SourceUse,
        selector: SourceUse,
        translations: Translations<'source>,
        looked_up: bool,
    ) -> Result<(), CatalogError> {
        let mut pending = crate::export_evidence_lock::lock(&self.pending)?;
        let row = pending.entry((domain, key)).or_insert_with(|| Pending {
            key,
            payloads: Vec::new(),
            selectors: Vec::new(),
            translations,
            looked_up,
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
            language_policy: CatalogLanguagePolicy::AVAILABLE_SUPPORTED,
            language_sources: language_sources(self.texts, self.origins())?,
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
            let Some(input) = self.origins().locale(language) else {
                continue;
            };
            let projection = catalog_text_forms::project(
                &self.origins().uses,
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
                .origins()
                .uses
                .value(SourceValue::Present(id), &row.payloads, true),
            key: self
                .origins()
                .uses
                .value(SourceValue::Present(row.key), &row.payloads, true),
            translations: self.origins().uses.value(
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
