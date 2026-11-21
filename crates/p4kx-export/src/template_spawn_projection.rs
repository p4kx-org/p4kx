// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_v3::tag_catalog::dependent_origins::ContextualSourcedValue;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;

/// A registered content result derived from one actual property-value occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateSpawnProjection {
    pub(crate) source: SourceValueRef,
    pub(crate) result: TemplateSpawnResult,
}

/// Content registration preserves the established identity recipes and empty-result decisions.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum TemplateSpawnResult {
    Ships {
        pool: ContextualSourcedValue<CatalogGuid>,
        empty: bool,
    },
    Npcs {
        pool: ContextualSourcedValue<CatalogGuid>,
        empty: bool,
    },
    Entities {
        pool: ContextualSourcedValue<CatalogGuid>,
        empty: bool,
    },
    Location {
        query: ContextualSourcedValue<CatalogGuid>,
    },
    Unavailable {
        identifier: ContextualSourcedValue<CatalogGuid>,
    },
}

/// A winning variable's existing level/pool selection using only canonical source navigation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplatePoolBinding {
    pub(crate) property: SourceValueRef,
    pub(crate) variable: crate::template_source_refs::SourceFieldRef,
    pub(crate) token: crate::template_source_refs::DeclarationBinding,
    pub(crate) registration: SourceValueRef,
    pub(crate) level: crate::spawn_model::SpawnLevel,
}
