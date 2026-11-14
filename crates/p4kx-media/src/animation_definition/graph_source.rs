// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{SourceDeclaration, SourceDeclarationKind, SourceElement};
use schemars::JsonSchema;
use serde::Serialize;

use super::graph_field::GraphField;
use super::payload::{self as wire, DefinitionFamily, DefinitionPayload};
use super::projection::DefinitionProjection;
use crate::audio_control_payload::{AtlDeclaration, AtlLocation, AtlPosition};
use crate::{MediaError, MediaWriter};

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum GraphRole {
    Database,
    FragmentList,
    FragmentGroup,
    TransitionList,
    Transition,
    TransitionOption,
    Fragment,
    AnimationLayer,
    ProceduralLayer,
    Animation,
    Procedural,
    LayerBlend,
    SubDatabases,
    SubDatabase,
    SubDatabaseFragment,
    BlendSpace,
    CombinedSpace,
    Dimensions,
    Extraction,
    Parameter,
    ExampleList,
    ExamplePseudo,
    Example,
    Pseudo,
    Faces,
    Face,
    BlendSpaces,
    BlendSpaceInput,
}

impl GraphRole {
    pub(super) fn of(
        family: DefinitionFamily,
        parent: Option<Self>,
        root: bool,
        element: SourceElement<'_>,
    ) -> Option<Self> {
        if element.namespace().is_some() {
            return None;
        }
        let tag = element.name();
        if root {
            return match (family, tag) {
                (DefinitionFamily::Adb, "AnimDB") => Some(Self::Database),
                (DefinitionFamily::Bspace, "ParaGroup") => Some(Self::BlendSpace),
                (DefinitionFamily::Comb, "CombinedBlendSpace") => Some(Self::CombinedSpace),
                _ => None,
            };
        }
        let parent = parent?;
        match family {
            DefinitionFamily::Adb => parent.mannequin_child(tag),
            DefinitionFamily::Bspace | DefinitionFamily::Comb => parent.space_child(tag),
            DefinitionFamily::Chrparams | DefinitionFamily::Animevents => None,
        }
    }

    fn mannequin_child(self, tag: &str) -> Option<Self> {
        Some(match (self, tag) {
            (Self::Database, "FragmentList") => Self::FragmentList,
            (Self::Database, "FragmentBlendList") => Self::TransitionList,
            (Self::Database, "SubADBs") => Self::SubDatabases,
            (Self::FragmentList, _) => Self::FragmentGroup,
            (Self::TransitionList, "Blend") => Self::Transition,
            (Self::Transition, _) => Self::TransitionOption,
            (Self::FragmentGroup | Self::TransitionOption, "Fragment") => Self::Fragment,
            (Self::Fragment, "AnimLayer") => Self::AnimationLayer,
            (Self::Fragment, "ProcLayer") => Self::ProceduralLayer,
            (Self::AnimationLayer, "Animation") => Self::Animation,
            (Self::ProceduralLayer, "Procedural") => Self::Procedural,
            (Self::AnimationLayer | Self::ProceduralLayer, "Blend") => Self::LayerBlend,
            (Self::SubDatabases, "SubADB") => Self::SubDatabase,
            (Self::SubDatabase, "FragmentID") => Self::SubDatabaseFragment,
            _ => return None,
        })
    }

    fn space_child(self, tag: &str) -> Option<Self> {
        Some(match (self, tag) {
            (Self::BlendSpace | Self::CombinedSpace, "Dimensions") => Self::Dimensions,
            (Self::BlendSpace | Self::CombinedSpace, "AdditionalExtraction") => Self::Extraction,
            (Self::Dimensions | Self::Extraction, "Param") => Self::Parameter,
            (Self::BlendSpace, "ExampleList") => Self::ExampleList,
            (Self::ExampleList, "Example") => Self::Example,
            (Self::BlendSpace, "ExamplePseudo") => Self::ExamplePseudo,
            (Self::ExamplePseudo, "Pseudo") => Self::Pseudo,
            (Self::BlendSpace, "Blendable") => Self::Faces,
            (Self::Faces, "Face") => Self::Face,
            (Self::CombinedSpace, "BlendSpaces") => Self::BlendSpaces,
            (Self::BlendSpaces, "BlendSpace") => Self::BlendSpaceInput,
            _ => return None,
        })
    }

    pub(super) fn write(
        self,
        projection: &DefinitionProjection<'_, '_>,
        name: &str,
        index: u64,
        location: AtlLocation,
        position: AtlPosition,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        projection.write(
            position,
            wire::GRAPH_NODE,
            vec![index],
            &DefinitionPayload::GraphNode(GraphNode {
                role: self,
                name,
                location,
                position,
            }),
            writer,
        )
    }

    pub(super) fn write_attribute(
        projection: &DefinitionProjection<'_, '_>,
        field: GraphField,
        attribute: &SourceDeclaration<'_>,
        indices: [u64; 2],
        position: AtlPosition,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), super::projection::Failure> {
        if !matches!(attribute.kind(), SourceDeclarationKind::Attribute { .. }) {
            return Ok(());
        }

        projection.write(
            position,
            wire::GRAPH_ATTRIBUTE,
            indices.to_vec(),
            &DefinitionPayload::GraphAttribute(GraphAttribute {
                field,
                declaration: AtlDeclaration {
                    qualified_name: attribute.qualified_name(),
                    value: attribute.value(),
                    lexeme: attribute.lexeme(),
                    kind: attribute.kind().try_into()?,
                    position,
                },
            }),
            writer,
        )?;
        Ok(())
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct GraphNode<'source> {
    role: GraphRole,
    name: &'source str,
    location: AtlLocation,
    position: AtlPosition,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct GraphAttribute<'source> {
    field: GraphField,
    declaration: AtlDeclaration<'source>,
}
