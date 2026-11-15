// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use super::graph_source::GraphRole;
use super::origins;
use super::payload::{self as wire, DefinitionPayload};
use super::projection::{DefinitionProjection, ElementSite};
use crate::audio_control_payload::AtlPosition;
use crate::audio_record_origins as records;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, Status, Subresource};

#[derive(Clone, Copy)]
pub(super) enum ClipOwner {
    Repaired,
    Other,
    Group(Group),
    Fragment(Fragment),
    Layer {
        fragment: Fragment,
        layer: ElementSite,
    },
    BlendExample(ElementSite),
}

#[derive(Clone, Copy)]
pub(super) struct Group {
    kind: ClipReferenceParentKind,
    site: ElementSite,
}

#[derive(Clone, Copy)]
pub(super) struct Fragment {
    group: Group,
    site: ElementSite,
}

impl ClipOwner {
    pub(super) const fn root(repaired: bool) -> Self {
        if repaired {
            Self::Repaired
        } else {
            Self::Other
        }
    }

    pub(super) const fn at(self, role: Option<GraphRole>, site: ElementSite) -> Self {
        match (self, role) {
            (Self::Repaired, _) => Self::Repaired,
            (_, Some(GraphRole::FragmentGroup)) => Self::Group(Group {
                kind: ClipReferenceParentKind::FragmentGroup,
                site,
            }),
            (_, Some(GraphRole::TransitionOption)) => Self::Group(Group {
                kind: ClipReferenceParentKind::TransitionOption,
                site,
            }),
            (Self::Group(group), Some(GraphRole::Fragment)) => {
                Self::Fragment(Fragment { group, site })
            }
            (Self::Fragment(fragment), Some(GraphRole::AnimationLayer)) => Self::Layer {
                fragment,
                layer: site,
            },
            (Self::Layer { .. }, Some(GraphRole::Animation)) => self,
            (_, Some(GraphRole::Example)) => Self::BlendExample(site),
            _ => Self::Other,
        }
    }

    pub(super) fn write(
        self,
        projection: &DefinitionProjection<'_, '_>,
        indices: [u64; 2],
        selector_position: AtlPosition,
        authored_name: &str,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let (kind, site, group, layer) = match self {
            Self::Layer { fragment, layer } => (
                ClipReferenceOwnerKind::MannequinFragment,
                fragment.site,
                Some(fragment.group),
                Some(layer),
            ),
            Self::BlendExample(site) => {
                (ClipReferenceOwnerKind::BlendSpaceExample, site, None, None)
            }
            _ => return Ok(()),
        };
        let prepared =
            prepare_origins(projection, selector_position, (site, group, layer), writer)?;
        let payload = ClipReference {
            owner: ClipReferenceOwner {
                kind,
                element_ordinal: DecimalU64::new(site.index),
            },
            owner_parent: group.map(|group| ClipReferenceParent {
                kind: group.kind,
                element_ordinal: DecimalU64::new(group.site.index),
            }),
            layer_ordinal: layer.map(|layer| DecimalU64::new(layer.index)),
            selector_position,
            authored_name,
            resolution: ClipReferenceResolution::Unresolved {
                reason: UnresolvedClipReason::of(kind, authored_name, projection, writer),
            },
        };
        prepared.write(
            writer,
            &Subresource::new(wire::CLIP_REFERENCE, indices.to_vec()),
            Status::Complete,
            &DefinitionPayload::ClipReference(payload),
            &[],
        )
    }
}

fn prepare_origins(
    projection: &DefinitionProjection<'_, '_>,
    selector_position: AtlPosition,
    (owner, group, layer): (ElementSite, Option<Group>, Option<ElementSite>),
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let proof =
        |position| origins::position(projection.input, position, SourceUseRole::Selector, writer);
    let mut prepared = origins::record(projection.input, proof(selector_position)?, writer)?;
    records::subtree(
        &mut prepared,
        "/data/payload/owner",
        proof(owner.position)?,
        writer,
    )?;
    if let Some(group) = group {
        records::subtree(
            &mut prepared,
            "/data/payload/owner_parent",
            proof(group.site.position)?,
            writer,
        )?;
    }
    if let Some(layer) = layer {
        records::field(
            &mut prepared,
            "/data/payload/layer_ordinal",
            proof(layer.position)?,
            writer,
        )?;
    }
    Ok(prepared)
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ClipReference<'source> {
    owner: ClipReferenceOwner,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_parent: Option<ClipReferenceParent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layer_ordinal: Option<DecimalU64>,
    selector_position: AtlPosition,
    authored_name: &'source str,
    resolution: ClipReferenceResolution,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClipReferenceOwner {
    kind: ClipReferenceOwnerKind,
    element_ordinal: DecimalU64,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ClipReferenceOwnerKind {
    MannequinFragment,
    BlendSpaceExample,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClipReferenceParent {
    kind: ClipReferenceParentKind,
    element_ordinal: DecimalU64,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ClipReferenceParentKind {
    FragmentGroup,
    TransitionOption,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
enum ClipReferenceResolution {
    Unresolved { reason: UnresolvedClipReason },
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum UnresolvedClipReason {
    EmptySelector,
    NoOwningScope,
    SourceJoinUnavailable,
}

impl UnresolvedClipReason {
    const fn of(
        kind: ClipReferenceOwnerKind,
        name: &str,
        projection: &DefinitionProjection<'_, '_>,
        writer: &MediaWriter<'_>,
    ) -> Self {
        if name.is_empty() {
            return Self::EmptySelector;
        }
        let directory = projection.context.directory;
        if matches!(kind, ClipReferenceOwnerKind::MannequinFragment)
            && writer.animation_entity.is_some()
            && directory.chain.is_empty()
            && directory.uses_definition_scope()
        {
            Self::SourceJoinUnavailable
        } else {
            Self::NoOwningScope
        }
    }
}
