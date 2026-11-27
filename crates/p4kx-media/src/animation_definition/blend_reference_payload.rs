// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_control_payload::AtlPosition;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct BlendReference<'source> {
    pub(super) kind: PointKind,
    pub(super) attribute: &'source str,
    pub(super) value: &'source str,
    pub(super) lexeme: Option<&'source str>,
    pub(super) position: AtlPosition,
    pub(super) ordinal: OrdinalDisposition,
    pub(super) alias: AliasDisposition,
    pub(super) interpretations: Interpretation,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PointKind {
    Face,
    Pseudo,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum OrdinalDisposition {
    NonInteger,
    UnavailableLayout,
    OutOfRange,
    Candidate,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum AliasDisposition {
    Missing,
    Exact,
    RepeatedExact,
    CaseVariant,
    RepeatedCaseVariant,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Interpretation {
    Unresolved,
    OrdinalOnly,
    ExactAliasOnly,
    Agreement,
    Conflict,
    TentativeCaseVariant,
    OrdinalWithCaseVariant,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct BlendCandidate {
    pub(super) basis: CandidateBasis,
    pub(super) target_element_index: DecimalU64,
    pub(super) target_kind: ExampleKind,
    pub(super) target_position: AtlPosition,
    pub(super) alias_declaration_ordinal: Option<DecimalU64>,
    pub(super) alias_position: Option<AtlPosition>,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CandidateBasis {
    LocalOrdinal,
    ExactAlias,
    CaseVariantAlias,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ExampleKind {
    Real,
    Pseudo,
}
