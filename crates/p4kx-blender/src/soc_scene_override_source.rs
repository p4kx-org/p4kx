// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_assembly::SocContainerCapture;
use p4kx_extraction::{
    CancelToken, InputScope, PhysicalSourceRef, SourceInput, SourceLocation, SourceUseRole,
    SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::{soc_scene_classes, soc_scene_raw};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntityReferenceOverrideSource {
    source: PhysicalSourceRef,
    #[schemars(
        description = "Original verified member payload encoded with standard padded Base64."
    )]
    raw: SourcedValue<String>,
}

pub(crate) fn read(
    capture: &SocContainerCapture,
    sources: &SourceUses,
    cancel: &CancelToken,
) -> Result<Vec<EntityReferenceOverrideSource>, SocSceneError> {
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for member in capture
        .occurrences()
        .iter()
        .flat_map(|occurrence| &occurrence.entity_reference_overrides)
    {
        soc_scene_classes::check(cancel)?;
        let Ok(bytes) = member.payload() else {
            continue;
        };
        let scope = sources.observe(SourceInput::Archive(member.origin()), InputScope::Payload)?;
        let source = scope.source_ref().clone();
        if !seen.insert(source.clone()) {
            continue;
        }
        rows.try_reserve(1)?;
        let end = u64::try_from(bytes.len()).map_err(|_| SocSceneError::Position)?;
        let used = scope.at(
            SourceLocation::Subresource {
                namespace: "socEntityReferenceOverride".to_owned(),
                indices: vec![0, end],
                field: "raw".to_owned(),
            },
            SourceUseRole::Payload,
        )?;
        rows.push(EntityReferenceOverrideSource {
            source,
            raw: sources.value(
                SourceValue::Present(soc_scene_raw::encode(bytes, cancel)?),
                &[used],
                true,
            ),
        });
    }
    Ok(rows)
}
