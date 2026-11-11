// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::broker_values::{Month, RepeatType};
use crate::catalog_text_contract::GameTextRef;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_extras::EndReason;
use crate::template_modifier::{LicenseType, Reaction};
use crate::template_objective::{CommsTiming, ObjectiveCategory, ObjectiveState};

/// Interpretation of a selected original key independently of source presence and text lookup.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum LocalizationMeaning {
    Lookup,
    Empty,
    Placeholder,
    NotKey,
    Null,
    WrongShape,
    Unreadable,
}

/// A real original key's lookup into the existing canonical `GameText` table.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateLocalization {
    pub(crate) source: SourceValueRef,
    pub(crate) interpretation: LocalizationMeaning,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) text: SourcedValue<GameTextRef>,
}

/// Recognized control meanings; the original label remains in its one D cell.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum TemplateVocabulary {
    ObjectiveState { value: Option<ObjectiveState> },
    ObjectiveCategory { value: Option<ObjectiveCategory> },
    CommsTiming { value: Option<CommsTiming> },
    EndReason { value: Option<EndReason> },
    Reaction { value: Option<Reaction> },
    LicenseType { value: Option<LicenseType> },
    Month { value: Option<Month> },
    RepeatType { value: Option<RepeatType> },
}

/// One explicit interpretation of an actual label or source Struct type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateVocabularyProjection {
    pub(crate) source: SourceValueRef,
    pub(crate) meaning: SourcedValue<TemplateVocabulary>,
}
