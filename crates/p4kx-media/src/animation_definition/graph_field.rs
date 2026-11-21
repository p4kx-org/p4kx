// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{SourceDeclaration, SourceDeclarationKind};
use schemars::JsonSchema;
use serde::Serialize;

use super::graph_source::GraphRole;

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum GraphField {
    Name,
    Guid,
    Tags,
    FragmentTags,
    Flags,
    OptionWeight,
    BlendOutDuration,
    StartTime,
    ExitTime,
    EnterTime,
    SelectTime,
    Duration,
    From,
    To,
    SyncGroup,
    Terminal,
    TransitionFragment,
    TransitionTags,
    Speed,
    Weight,
    WeightList,
    ProcedureType,
    ProcedureString,
    ProcedureString2,
    ProcedureCrcString,
    File,
    TagDefinition,
    FragmentDefinition,
    Version,
    Min,
    Max,
    Cells,
    JointName,
    StartKey,
    EndKey,
    Scale,
    Locked,
    DeltaExtraction,
    AnimationName,
    Alias,
    PlaybackScale,
    ParameterAssignment,
    DirectDeltaMotion,
    FacePoint,
    ExampleIndex,
    ExampleWeight,
    FixedParameter,
    ChooseBlendSpace,
    WeightBlendSpace,
    ParameterScale,
    Unknown,
}

impl GraphField {
    pub(super) fn of(role: GraphRole, attribute: &SourceDeclaration<'_>) -> Self {
        if !matches!(
            attribute.kind(),
            SourceDeclarationKind::Attribute { namespace: None }
        ) {
            return Self::Unknown;
        }
        let name = attribute.qualified_name();
        match role {
            GraphRole::Fragment
            | GraphRole::Transition
            | GraphRole::TransitionOption
            | GraphRole::LayerBlend
            | GraphRole::Animation
            | GraphRole::Procedural
            | GraphRole::Database
            | GraphRole::SubDatabase
            | GraphRole::SubDatabaseFragment => Self::mannequin(role, name),
            _ => Self::space(role, name),
        }
    }

    fn mannequin(role: GraphRole, name: &str) -> Self {
        use super::graph_source::GraphRole::{
            Animation, Database, Fragment, LayerBlend, Procedural, SubDatabase,
            SubDatabaseFragment, Transition, TransitionOption,
        };
        match (role, name) {
            (Database, "TagDef") => Self::TagDefinition,
            (Database, "FragDef") => Self::FragmentDefinition,
            (Fragment, "GUID") => Self::Guid,
            (Fragment | SubDatabase, "Tags") => Self::Tags,
            (Fragment, "FragTags") => Self::FragmentTags,
            (Fragment, "BlendOutDuration") => Self::BlendOutDuration,
            (Fragment, "TransitionFragment") => Self::TransitionFragment,
            (Fragment, "TransitionTags") => Self::TransitionTags,
            (Fragment | TransitionOption, "enterTime") => Self::EnterTime,
            (Fragment | TransitionOption, "selectTime") => Self::SelectTime,
            (Fragment, "startTime") | (LayerBlend | Transition, "StartTime") => Self::StartTime,
            (LayerBlend | Transition, "ExitTime") => Self::ExitTime,
            (LayerBlend | Transition, "Duration") => Self::Duration,
            (Transition, "from") => Self::From,
            (Transition, "to") => Self::To,
            (LayerBlend | Transition, "syncGroup") => Self::SyncGroup,
            (LayerBlend | Transition, "terminal") => Self::Terminal,
            (Fragment | LayerBlend | Transition, "OptionWeight") => Self::OptionWeight,
            (Fragment | LayerBlend | Transition | Animation, "flags") => Self::Flags,
            (Animation, "name") => Self::AnimationName,
            (Animation | LayerBlend | Transition, "Speed" | "speed") => Self::Speed,
            (Animation, "weight") => Self::Weight,
            (Animation, "weightList") => Self::WeightList,
            (Procedural, "type") => Self::ProcedureType,
            (Procedural, "string") => Self::ProcedureString,
            (Procedural, "string2") => Self::ProcedureString2,
            (Procedural, "crcString") => Self::ProcedureCrcString,
            (SubDatabase, "File") => Self::File,
            (SubDatabaseFragment, "Name") => Self::Name,
            _ => Self::Unknown,
        }
    }

    fn space(role: GraphRole, name: &str) -> Self {
        use super::graph_source::GraphRole::{
            BlendSpace, BlendSpaceInput, Example, Face, Parameter, Pseudo,
        };
        match (role, name) {
            (BlendSpace, "version") => Self::Version,
            (Parameter, "Name" | "name") => Self::Name,
            (Parameter, "Min" | "min") => Self::Min,
            (Parameter, "Max" | "max") => Self::Max,
            (Parameter, "Cells" | "cells") => Self::Cells,
            (Parameter, "JointName") => Self::JointName,
            (Parameter, "SKey" | "skey") => Self::StartKey,
            (Parameter, "EKey" | "ekey") => Self::EndKey,
            (Parameter, "Scale" | "scale") => Self::Scale,
            (Parameter, "Locked" | "locked") => Self::Locked,
            (Parameter, "deltaextraction") => Self::DeltaExtraction,
            (Parameter, "ChooseBlendSpace") => Self::ChooseBlendSpace,
            (Parameter, "WeightBlendSpace") => Self::WeightBlendSpace,
            (Parameter, "ParaScale") => Self::ParameterScale,
            (Example | BlendSpaceInput, "AName") => Self::AnimationName,
            (Example | Pseudo, "Alias") => Self::Alias,
            (Example, "PlaybackScale") => Self::PlaybackScale,
            (BlendSpaceInput, "fixedPara") => Self::FixedParameter,
            (Example | Pseudo, _) if indexed(name, "SetPara") => Self::ParameterAssignment,
            (Example, _) if indexed(name, "UseDirectlyForDeltaMotion") => Self::DirectDeltaMotion,
            (Face, _) if indexed(name, "p") => Self::FacePoint,
            (Pseudo, _) if indexed(name, "p") => Self::ExampleIndex,
            (Pseudo, _) if indexed(name, "w") => Self::ExampleWeight,
            _ => Self::Unknown,
        }
    }
}

fn indexed(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}
