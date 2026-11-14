// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use super::payload::{self as wire, DefinitionPayload};
use super::projection::DefinitionProjection;
use crate::audio_control_payload::AtlPosition;
use crate::{MediaError, MediaWriter};

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum FrontierReason {
    MissingFile,
    DuplicateFile,
    UnsupportedTargetRoot,
    RepairedTarget,
    InvalidTarget,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Frontier {
    source_element_index: DecimalU64,
    source_declaration_ordinal: Option<DecimalU64>,
    reason: FrontierReason,
}

impl Frontier {
    pub(super) fn new(element: u64, reason: FrontierReason, declaration: Option<u64>) -> Self {
        Self {
            source_element_index: DecimalU64::new(element),
            source_declaration_ordinal: declaration.map(DecimalU64::new),
            reason,
        }
    }
}

pub(super) fn write_attribute_issue(
    projection: &DefinitionProjection<'_, '_>,
    site: (u64, AtlPosition),
    reason: FrontierReason,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let (index, position) = site;
    projection.write(
        position,
        wire::SUB_DATABASE_ATTRIBUTE_FRONTIER,
        vec![index],
        &DefinitionPayload::SubDatabaseFrontier(Frontier::new(index, reason, None)),
        writer,
    )
}
