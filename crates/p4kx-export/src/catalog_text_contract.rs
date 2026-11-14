// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::BTreeMap;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;

/// A reference into the public short-name domain.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(transparent)]
pub(crate) struct PublicNameRef(pub(crate) String);

/// A reference into the protected description domain.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(transparent)]
#[schemars(extend("x-gameText" = true))]
pub(crate) struct GameTextRef(pub(crate) String);

/// One canonical text key and its actually available language projections.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "CanonicalTextOf{R}", transform = every_field_required)]
pub(crate) struct CanonicalText<'source, R> {
    pub(crate) id: SourcedValue<R>,
    pub(crate) key: SourcedValue<&'source str>,
    pub(crate) translations: SourcedValue<BTreeMap<String, TextForms<'source>>>,
}

/// The exact decoded source text and its derived display form for one language.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TextForms<'source> {
    pub(crate) source_text: SourcedValue<Cow<'source, str>>,
    pub(crate) display_text: SourcedValue<Cow<'source, str>>,
}

/// Separate public-name and protected game-text definition tables.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogTexts<'source> {
    pub(crate) public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) game_texts: Vec<CanonicalText<'source, GameTextRef>>,
}

#[cfg(test)]
#[path = "catalog_text_schema_tests.rs"]
mod tests;
