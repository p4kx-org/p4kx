// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::sync::Mutex;

use p4kx_extraction::{
    SourceLocation, SourceUse, SourceUseRole, SourceValue, SourcedValue, is_placeholder_key,
};

#[path = "catalog_protected_key.rs"]
mod protected_key;
pub(crate) use protected_key::ProtectedKey;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::catalog_text_contract::{CatalogTexts, GameTextRef, PublicNameRef};
use crate::mission_text_inputs::{MissionKey, SelectedKey};
use crate::texts::{CATALOG_LANGUAGES, Texts};
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "kind_text_coverage.rs"]
mod kind_text_coverage;
#[path = "catalog_text_registrations.rs"]
mod registrations;
#[path = "catalog_text_rows.rs"]
mod text_rows;

use self::kind_text_coverage::{KindTextCoverage, Lookup, Translations};
pub use self::kind_text_coverage::{KindTextSummary, TextOwner};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Domain {
    PublicName,
    GameText,
}

struct Pending<'source> {
    key: &'source str,
    payloads: Vec<SourceUse>,
    selectors: Vec<SourceUse>,
    lookup: Lookup,
}

#[derive(Default)]
struct RegistryState<'source> {
    pending: BTreeMap<(Domain, &'source str), Pending<'source>>,
    coverage: KindTextCoverage,
}

/// Canonical text registrations shared by the scopes of one catalog run.
#[derive(Default)]
pub(crate) struct CatalogTextRegistry<'source> {
    state: Mutex<RegistryState<'source>>,
}

/// Canonical texts and coverage from the same registrations.
pub(crate) struct FinishedTexts<'source> {
    pub(crate) texts: CatalogTexts<'source>,
    pub(crate) kind_coverage: Vec<KindTextSummary>,
}

/// Per-catalog projection inputs and canonical text rows with precise source uses.
pub(crate) struct CatalogProjection<'context, 'source> {
    texts: &'context Texts<'source>,
    original: &'context OriginalCatalogInput<'context, 'source>,
    registry: &'context CatalogTextRegistry<'source>,
    owner: Option<TextOwner>,
}

impl<'context, 'source> CatalogProjection<'context, 'source> {
    pub(crate) const fn new(
        texts: &'context Texts<'source>,
        original: &'context OriginalCatalogInput<'context, 'source>,
        registry: &'context CatalogTextRegistry<'source>,
    ) -> Self {
        Self {
            texts,
            original,
            registry,
            owner: None,
        }
    }

    pub(crate) const fn for_kind(&self, kind: Kind) -> Self {
        self.for_text_owner(TextOwner::Catalog(kind))
    }

    pub(crate) const fn for_text_owner(&self, owner: TextOwner) -> Self {
        Self {
            owner: Some(owner),
            ..*self
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
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
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
        self.mission_key_fallible(key, render)
    }

    pub(crate) fn mission_key_fallible<T, E: From<CatalogError>>(
        &self,
        key: &MissionKey<'source>,
        render: impl FnOnce(&Translations<'source>, &SelectedKey) -> Result<T, E>,
    ) -> Result<(GameTextRef, T), E> {
        let observed = key.retain(self.origins())?;
        let translations = self.translations(key.text);
        let value = render(&translations, &observed)?;
        self.insert_pending(
            Domain::GameText,
            key.text,
            observed.payload,
            observed.selector,
            Lookup::from_projection(&translations),
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
        let mut state = crate::export_evidence_lock::lock(&self.registry.state)?;
        let RegistryState { pending, coverage } = &mut *state;
        if let Some(row) = pending.get_mut(&(domain, key)) {
            let owner = self
                .owner
                .ok_or(ExportAccountingError::UnscopedCatalogText)?;
            let lookup = if placeholder {
                Lookup::Placeholder
            } else {
                row.lookup
            };
            lookup.recount(self.texts);
            coverage.record(owner, lookup);
            row.payloads.push(payload);
            row.selectors.push(selector);
            return Ok(());
        }
        drop(state);
        let lookup = if placeholder {
            self.texts.note_placeholder();
            Lookup::Placeholder
        } else {
            Lookup::from_projection(&self.translations(key))
        };
        self.insert_pending(domain, key, payload, selector, lookup)
    }

    fn translations(&self, key: &str) -> Translations<'source> {
        CATALOG_LANGUAGES
            .into_iter()
            .map(|language| (language, self.texts.project(language, key)))
            .filter(|(language, _)| self.origins().locale(*language).is_some())
            .collect()
    }

    fn insert_pending(
        &self,
        domain: Domain,
        key: &'source str,
        payload: SourceUse,
        selector: SourceUse,
        lookup: Lookup,
    ) -> Result<(), CatalogError> {
        let mut state = crate::export_evidence_lock::lock(&self.registry.state)?;
        let owner = self
            .owner
            .ok_or(ExportAccountingError::UnscopedCatalogText)?;

        state.coverage.record(owner, lookup);
        let row = state
            .pending
            .entry((domain, key))
            .or_insert_with(|| Pending {
                key,
                payloads: Vec::new(),
                selectors: Vec::new(),
                lookup,
            });
        row.payloads.push(payload);
        row.selectors.push(selector);
        drop(state);
        Ok(())
    }
}

fn identifier(domain: Domain, key: &str) -> String {
    let prefix = match domain {
        Domain::PublicName => "name",
        Domain::GameText => "text",
    };
    format!("{prefix}:{}:{key}", key.len())
}

#[cfg(test)]
#[path = "catalog_projection_tests.rs"]
mod tests;
