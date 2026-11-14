// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    SocGeometryCandidateKind, SocGeometryReference, SocGeometryTarget, SocGeometryTier,
};
use p4kx_extraction::{
    EntryOrigin, InputScope, PhysicalSourceRef, SourceInput, SourceLocation, SourceUse,
    SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_controller_candidates::{Candidates, ControllerIndex};
use crate::soc_scene_origin::XmlOrigin;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct GeometryReference {
    chunk_ordinal: usize,
    model_ordinal: usize,
    target: SourcedValue<Target>,
    #[serde(skip_serializing_if = "Option::is_none")]
    controller_candidates: Option<SourcedValue<Candidates>>,
}

impl GeometryReference {
    pub(super) fn read(
        original: &SocGeometryReference,
        origin: &XmlOrigin<'_>,
        controllers: &ControllerIndex,
    ) -> Result<Self, SocSceneError> {
        let range = &original.source_range;
        let selector = origin.scope.at(
            SourceLocation::Subresource {
                namespace: "socIncludedObjects".into(),
                indices: vec![original.chunk_ordinal as u64, range.start, range.end],
                field: "modelPath".into(),
            },
            SourceUseRole::Selector,
        )?;
        let controller_candidates =
            controllers.candidates(&original.target, origin.sources, &selector)?;
        let mut uses = vec![selector];
        let (target, complete) = Target::read(&original.target, origin.sources, &mut uses)?;
        Ok(Self {
            chunk_ordinal: original.chunk_ordinal,
            model_ordinal: original.model_ordinal,
            controller_candidates: Some(controller_candidates),
            target: origin
                .sources
                .value(SourceValue::Present(target), &uses, complete),
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Target {
    InvalidSelector,
    Missing,
    Matched {
        tier: Tier,
        source: PhysicalSourceRef,
    },
    Ambiguous {
        tier: Tier,
        candidates: Vec<PhysicalSourceRef>,
    },
}

impl Target {
    fn read(
        original: &SocGeometryTarget,
        sources: &SourceUses,
        uses: &mut Vec<SourceUse>,
    ) -> Result<(Self, bool), SocSceneError> {
        let (target, complete) = match original {
            SocGeometryTarget::InvalidSelector => (Self::InvalidSelector, true),
            SocGeometryTarget::Missing => (Self::Missing, false),
            SocGeometryTarget::Matched { tier, entry } => (
                Self::Matched {
                    tier: Tier::read(*tier)?,
                    source: directory_entry(entry, sources, uses)?,
                },
                false,
            ),
            SocGeometryTarget::Ambiguous { tier, entries } => (
                Self::Ambiguous {
                    tier: Tier::read(*tier)?,
                    candidates: entries
                        .iter()
                        .map(|entry| directory_entry(entry, sources, uses))
                        .collect::<Result<_, _>>()?,
                },
                false,
            ),
            _ => return Err(SocSceneError::Position),
        };
        Ok((target, complete))
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Tier {
    depth: usize,
    kind: CandidateKind,
}

impl Tier {
    const fn read(original: SocGeometryTier) -> Result<Self, SocSceneError> {
        let kind = match original.kind {
            SocGeometryCandidateKind::DataPrefixed => CandidateKind::DataPrefixed,
            SocGeometryCandidateKind::Relative => CandidateKind::Relative,
            _ => return Err(SocSceneError::Position),
        };
        Ok(Self {
            depth: original.depth,
            kind,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
enum CandidateKind {
    DataPrefixed,
    Relative,
}

pub(super) fn directory_entry(
    original: &EntryOrigin,
    sources: &SourceUses,
    uses: &mut Vec<SourceUse>,
) -> Result<PhysicalSourceRef, SocSceneError> {
    let scope = sources.observe(
        SourceInput::Archive(original),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    uses.push(scope.at(
        SourceLocation::Metadata {
            field: "archiveEntryPath".into(),
        },
        SourceUseRole::Payload,
    )?);
    Ok(scope.source_ref().clone())
}
