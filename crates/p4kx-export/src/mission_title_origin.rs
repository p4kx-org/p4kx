// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

use p4kx_extraction::{CancelToken, Language, SourceUse, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;

#[path = "mission_title_item_audit.rs"]
mod item_audit;

pub(crate) use self::item_audit::SelectedItemKey;
use self::item_audit::{ItemToken, ItemTokenOutcome};
use crate::catalog_origins::CatalogLocaleInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::GameTextRef;
use crate::catalog_v3::catalog_text_forms;
use crate::mission_text_inputs::{Consulted, MissionItemGap, MissionKey};
use crate::mission_title::{self, AsciiKey, Token};
use crate::schema_fields::every_field_required;
use crate::texts::TextProjection;

/// One authored key and the derived text of each actually available language.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(crate) struct ProjectedMissionText<'source> {
    #[schemars(extend("x-gameText" = true))]
    pub(crate) key: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) translations: BTreeMap<&'static str, SourcedValue<Cow<'source, str>>>,
    #[serde(rename = "itemTokens")]
    #[schemars(extend("x-gameText" = true))]
    item_tokens: BTreeMap<&'static str, Vec<ItemToken>>,
}

pub(crate) enum ResolvedToken<'read, 'source> {
    Key(&'read MissionKey<'source>),
    Runtime(&'static str),
    ItemKey(SelectedItemKey<'read, 'source>),
    ItemGap(MissionItemGap),
}

pub(crate) struct SelectedMissionText<'read, 'source> {
    pub(crate) key: Option<&'read MissionKey<'source>>,
    pub(crate) decisions: Vec<SourceUse>,
    pub(crate) unreadable: bool,
}

type Resolve<'query, 'read, 'source> = dyn FnMut(
        &str,
        Option<&str>,
        &mut Consulted<'read>,
    ) -> Result<Option<ResolvedToken<'read, 'source>>, CatalogError>
    + 'query;

pub(crate) fn project<'read, 'source>(
    projection: &CatalogProjection<'_, 'source>,
    selected: &SelectedMissionText<'read, 'source>,
    resolve: &mut Resolve<'_, 'read, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ProjectedMissionText<'source>, CatalogError> {
    mission_title::check_cancel(cancel)?;
    let origins = projection.origins();
    let Some(key) = selected.key else {
        let value = if selected.unreadable {
            SourceValue::Unreadable
        } else {
            SourceValue::Absent
        };
        return Ok(ProjectedMissionText {
            key: origins
                .uses
                .value(value, &selected.decisions, !selected.unreadable),
            translations: BTreeMap::new(),
            item_tokens: BTreeMap::new(),
        });
    };
    let (reference, (translations, item_tokens)) =
        projection.mission_key(key, |prepared, selector| {
            let mut translations = BTreeMap::new();
            let mut item_tokens = BTreeMap::new();
            for (&language, text) in prepared {
                mission_title::check_cancel(cancel)?;
                let locale = origins
                    .locale(language)
                    .ok_or(CatalogError::MissionTextPlan)?;
                let input = LanguageInputs {
                    projection,
                    locale,
                    language,
                    key: key.text,
                    selector,
                };
                let rendered = input.render(text.as_ref(), &selected.decisions, resolve, cancel)?;
                translations.insert(language.code(), rendered.value);
                if !rendered.item_tokens.is_empty() {
                    item_tokens.insert(language.code(), rendered.item_tokens);
                }
            }
            Ok((translations, item_tokens))
        })?;
    Ok(ProjectedMissionText {
        key: origins
            .uses
            .value(SourceValue::Present(reference), &selected.decisions, true),
        translations,
        item_tokens,
    })
}

#[derive(Clone, Copy)]
enum LookupState {
    Missing,
    Empty,
    Display,
}

impl LookupState {
    fn read(text: Option<&TextProjection<'_>>) -> Self {
        match text {
            None => Self::Missing,
            Some(text) if text.display.is_empty() => Self::Empty,
            Some(_) => Self::Display,
        }
    }

    fn retain(
        self,
        locale: &CatalogLocaleInput,
        language: Language,
        selector: &SourceUse,
        into: &mut Vec<SourceUse>,
    ) -> Result<(), CatalogError> {
        into.try_reserve(3)
            .map_err(CatalogError::SourceUseAllocation)?;
        into.push(catalog_text_forms::lookup_use(
            locale,
            language,
            selector,
            SourceUseRole::Selector,
        )?);
        if matches!(self, Self::Display) {
            into.push(catalog_text_forms::lookup_use(
                locale,
                language,
                selector,
                SourceUseRole::Payload,
            )?);
        }
        if !matches!(self, Self::Missing) {
            into.push(catalog_text_forms::lookup_use(
                locale,
                language,
                selector,
                SourceUseRole::Conversion,
            )?);
        }
        Ok(())
    }
}

struct RenderedLanguage<'source> {
    value: SourcedValue<Cow<'source, str>>,
    item_tokens: Vec<ItemToken>,
}

struct LanguageInputs<'read, 'catalog, 'source> {
    projection: &'read CatalogProjection<'catalog, 'source>,
    locale: &'read CatalogLocaleInput,
    language: Language,
    key: &'source str,
    selector: &'read SourceUse,
}

impl<'source> LanguageInputs<'_, '_, 'source> {
    fn render<'read>(
        &self,
        text: Option<&TextProjection<'source>>,
        decisions: &[SourceUse],
        resolve: &mut Resolve<'_, 'read, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<RenderedLanguage<'source>, CatalogError> {
        let mut used = Consulted::new(decisions)?;
        let state = LookupState::read(text);
        state.retain(self.locale, self.language, self.selector, used.uses_mut())?;
        let mut item_tokens = Vec::new();
        let value = if let Some(text) = text.filter(|text| !text.display.is_empty()) {
            SourceValue::Present(self.expand(
                text.display.clone(),
                state,
                &mut used,
                &mut item_tokens,
                resolve,
                cancel,
            )?)
        } else {
            SourceValue::Absent
        };
        let value = self
            .projection
            .origins()
            .uses
            .value(value, used.uses(), true);
        Ok(RenderedLanguage { value, item_tokens })
    }

    fn expand<'read>(
        &self,
        text: Cow<'source, str>,
        root: LookupState,
        used: &mut Consulted<'read>,
        item_tokens: &mut Vec<ItemToken>,
        resolve: &mut Resolve<'_, 'read, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Cow<'source, str>, CatalogError> {
        let mut lookups = HashMap::new();

        let mut keys = Vec::new();
        let mut key_selectors = HashMap::new();
        let mut localise = |key: &'source str| {
            let text = self.projection.legacy_texts().project(self.language, key);
            let state = LookupState::read(text.as_ref());
            lookups
                .try_reserve(1)
                .map_err(CatalogError::MissionTextAllocation)?;
            lookups.insert(AsciiKey::locale(key), state);
            Ok(text
                .map(|text| text.display)
                .filter(|text| !text.is_empty()))
        };
        let mut token = |source_key: &'source str, name: &str, variant: Option<&str>| {
            let selected = resolve(name, variant, used)?;
            if matches!(
                selected,
                Some(ResolvedToken::ItemKey(_) | ResolvedToken::ItemGap(_))
            ) {
                self.record_item(
                    key_selectors.get(&AsciiKey::locale(source_key)),
                    (source_key, name, variant),
                    resolve,
                    item_tokens,
                )?;
            }
            Ok(match selected {
                Some(
                    ResolvedToken::Key(key) | ResolvedToken::ItemKey(SelectedItemKey { key, .. }),
                ) => {
                    let selector = key.selector(self.projection.origins())?;
                    keys.try_reserve(1)
                        .map_err(CatalogError::SourceUseAllocation)?;
                    key_selectors
                        .try_reserve(1)
                        .map_err(CatalogError::SourceUseAllocation)?;
                    key_selectors
                        .entry(AsciiKey::locale(key.text))
                        .or_insert_with(|| selector.clone());
                    keys.push((key.text, selector));
                    Some(Token::Key(key.text))
                }
                Some(ResolvedToken::Runtime(label)) => Some(Token::Placeholder(label)),
                Some(ResolvedToken::ItemGap(_)) | None => None,
            })
        };
        let display =
            mission_title::render_language(self.key, text, &mut localise, &mut token, cancel)?;
        self.retain_lookups(keys, &lookups, root, used, cancel)?;
        Ok(display)
    }

    fn retain_lookups(
        &self,
        keys: Vec<(&'source str, SourceUse)>,
        lookups: &HashMap<AsciiKey<'source>, LookupState>,
        root: LookupState,
        used: &mut Consulted<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        for (key, selector) in keys {
            mission_title::check_cancel(cancel)?;
            let state = if AsciiKey::locale(key) == AsciiKey::locale(self.key) {
                root
            } else {
                *lookups
                    .get(&AsciiKey::locale(key))
                    .ok_or(CatalogError::MissionTextPlan)?
            };
            state.retain(self.locale, self.language, &selector, used.uses_mut())?;
        }
        Ok(())
    }

    fn record_item<'read>(
        &self,
        nested_selector: Option<&SourceUse>,
        (source_key, name, variant): (&str, &str, Option<&str>),
        resolve: &mut Resolve<'_, 'read, 'source>,
        item_tokens: &mut Vec<ItemToken>,
    ) -> Result<(), CatalogError> {
        let token_selector = if AsciiKey::locale(source_key) == AsciiKey::locale(self.key) {
            self.selector
        } else {
            nested_selector.ok_or(CatalogError::MissionTextPlan)?
        };
        let origins = self.projection.origins();
        let mut original = Vec::new();
        LookupState::Display.retain(self.locale, self.language, token_selector, &mut original)?;
        let mut selected = Consulted::new(&original)?;
        let outcome = match resolve(name, variant, &mut selected)? {
            Some(ResolvedToken::ItemKey(SelectedItemKey {
                key,
                guid,
                record_index,
            })) => {
                let selector = key.selector(origins)?;
                let projected = self
                    .projection
                    .legacy_texts()
                    .project(self.language, key.text);
                let state = LookupState::read(projected.as_ref());
                state.retain(self.locale, self.language, &selector, selected.uses_mut())?;
                match state {
                    LookupState::Display => ItemTokenOutcome::preview(guid, record_index),
                    _ => ItemTokenOutcome::Unresolved {
                        reason: MissionItemGap::MissingTranslation,
                    },
                }
            }
            Some(ResolvedToken::ItemGap(reason)) => ItemTokenOutcome::Unresolved { reason },
            _ => return Err(CatalogError::MissionTextPlan),
        };
        let row = ItemToken::new(origins, name, variant, &original, selected.uses(), outcome)?;
        item_tokens
            .try_reserve(1)
            .map_err(CatalogError::MissionTextAllocation)?;
        item_tokens.push(row);
        Ok(())
    }
}
