// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigins, SourceInventory, SourceInventorySelection, SourceUses};
use schemars::JsonSchema;
use serde::Serialize;

use crate::manifest::SCENE_PROVENANCE_FORMAT_VERSION;
use crate::{Manifest, PackageError, PackageResult};

pub(crate) struct SceneFields {
    pub(crate) fields: FieldOrigins,
    pub(crate) sources: SourceUses,
}

impl SceneFields {
    pub(crate) fn new() -> Self {
        Self {
            fields: FieldOrigins::new(),
            sources: SourceUses::with_compact_use_refs(),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SceneProvenance<'evidence> {
    field_origins: &'evidence FieldOrigins,
    #[schemars(with = "SourceInventory")]
    source_inventory: SourceInventorySelection<'evidence>,
    summary: FieldSummary,
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct FieldSummary {
    fields: usize,
    encrypted: usize,
    unencrypted: usize,
    unknown: usize,
    complete: usize,
}

pub(crate) fn attach(
    manifest: &mut Manifest,
    fields: &FieldOrigins,
    sources: &SourceUses,
) -> PackageResult<()> {
    let summary = FieldSummary::from_fields(fields);
    if summary.fields == 0 {
        return Ok(());
    }
    let inventory = sources.freeze().map_err(evidence_error)?;
    let source_inventory = inventory
        .select_uses(
            fields
                .iter()
                .flat_map(|(_, origin)| origin.source_uses_refs()),
        )
        .map_err(evidence_error)?;
    let provenance = SceneProvenance {
        field_origins: fields,
        source_inventory,
        summary,
    };
    manifest.scene_provenance = Some(serde_json::to_value(provenance).map_err(evidence_error)?);
    SCENE_PROVENANCE_FORMAT_VERSION.clone_into(&mut manifest.version);
    Ok(())
}

impl FieldSummary {
    fn from_fields(fields: &FieldOrigins) -> Self {
        let mut summary = Self::default();
        for (_, origin) in fields.iter() {
            summary.fields += 1;
            summary.complete += usize::from(origin.source_set_complete());
            match origin.contains_encrypted_source() {
                Some(true) => summary.encrypted += 1,
                Some(false) => summary.unencrypted += 1,
                None => summary.unknown += 1,
            }
        }
        summary
    }
}

pub(crate) fn evidence_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> PackageError {
    PackageError::SceneEvidence(Box::new(error))
}
