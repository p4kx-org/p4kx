// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::BTreeMap;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;

#[path = "catalog_language_sources.rs"]
mod language_sources;

pub(crate) use language_sources::language_sources;
pub use language_sources::{
    CatalogLanguagePolicy, CatalogLanguageSource, CatalogLanguageSources, SourceIniEncoding,
};

/// A reference into the public short-name domain.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct PublicNameRef(pub(crate) String);

impl PublicNameRef {
    /// Borrows the canonical public-name identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A reference into the protected description domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
#[schemars(extend("x-gameText" = true))]
pub struct GameTextRef(pub(crate) String);

impl GameTextRef {
    /// Borrows the canonical protected-text identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One canonical text key and its actually available language projections.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "CanonicalTextOf{R}", transform = every_field_required)]
pub struct CanonicalText<'source, R> {
    pub(crate) id: SourcedValue<R>,
    pub(crate) key: SourcedValue<&'source str>,
    pub(crate) translations: SourcedValue<BTreeMap<String, TextForms<'source>>>,
}

impl<'source, R> CanonicalText<'source, R> {
    /// The canonical identifier in the reference's text domain.
    #[must_use]
    pub const fn id(&self) -> &SourcedValue<R> {
        &self.id
    }

    /// The original localization key.
    #[must_use]
    pub const fn key(&self) -> &SourcedValue<&'source str> {
        &self.key
    }

    /// Available original and display forms by language code.
    #[must_use]
    pub const fn translations(&self) -> &SourcedValue<BTreeMap<String, TextForms<'source>>> {
        &self.translations
    }
}

/// The exact decoded source text and its derived display form for one language.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct TextForms<'source> {
    /// The decoded text, with the original markup and escapes.
    pub source_text: SourcedValue<Cow<'source, str>>,
    /// The text formatted for display.
    pub display_text: SourcedValue<Cow<'source, str>>,
}

/// Separate public-name and protected game-text definition tables.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogTexts<'source> {
    pub(crate) language_policy: CatalogLanguagePolicy,
    pub(crate) language_sources: CatalogLanguageSources<'source>,
    pub(crate) public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) game_texts: Vec<CanonicalText<'source, GameTextRef>>,
}

#[cfg(test)]
#[path = "catalog_text_schema_tests.rs"]
mod tests;
