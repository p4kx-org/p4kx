// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{SourceDeclaration, SourceDeclarationKind};
use schemars::JsonSchema;
use serde::Serialize;

use super::cig_audio_role::CigAudioRole;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CigAudioFieldRecord<'source> {
    field: CigAudioField,
    typed_value: CigAudioValue<'source>,
}

impl<'source> CigAudioFieldRecord<'source> {
    pub(crate) fn read(
        role: CigAudioRole,
        declaration: &SourceDeclaration<'source>,
    ) -> Option<Self> {
        if !matches!(
            declaration.kind(),
            SourceDeclarationKind::Attribute { namespace: None }
        ) {
            return None;
        }
        let field = CigAudioField::from_attribute(role, declaration.qualified_name())?;
        Some(Self {
            field,
            typed_value: field.value(declaration.value()),
        })
    }
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum CigAudioField {
    Type,
    Class,
    Guid,
    Id,
    Name,
    Category,
    Parent,
    StartTrigger,
    Aggregate,
    ContextString,
    PositionMode,
    Priority,
    VoiceLimit,
    DurationMin,
    DurationMax,
    Radius,
    Minimum,
    Maximum,
    GameDefined,
    OneShot,
    Dirty,
    FillSourceRoom,
    Reference,
    OverriddenGuid,
    OwnerContext,
    Parameter,
    Event,
    Source,
    Rtpc,
    SetGlobal,
    Node,
    SourceLinkId,
    TargetLinkId,
    X,
    Y,
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum CigAudioValue<'source> {
    Text(&'source str),
    GuidToken(&'source str),
    Boolean(bool),
    Number(f64),
    UnsignedInteger(u32),
    InvalidBoolean,
    InvalidNumber,
    InvalidUnsignedInteger,
}

impl CigAudioField {
    fn from_attribute(role: CigAudioRole, name: &str) -> Option<Self> {
        use CigAudioRole::{
            EditorPosition, EventReference, Link, Logic, LogicNode, Object, ParameterReference,
        };
        Some(match (role, name) {
            (Object | Logic | LogicNode, "type") => Self::Type,
            (LogicNode, "class") => Self::Class,
            (Object | Logic | LogicNode, "guid") => Self::Guid,
            (Object | LogicNode, "id") => Self::Id,
            (Object, "name") => Self::Name,
            (Object, "category") => Self::Category,
            (Object, "parent") => Self::Parent,
            (Object, "startTrigger") => Self::StartTrigger,
            (Object, "aggregate") => Self::Aggregate,
            (Object, "contextString") => Self::ContextString,
            (Object, "positionMode") => Self::PositionMode,
            (Object, "priority") => Self::Priority,
            (Object, "voiceLimit") => Self::VoiceLimit,
            (Object, "durationMin") => Self::DurationMin,
            (Object, "durationMax") => Self::DurationMax,
            (Object, "radius") => Self::Radius,
            (Object, "minVal") => Self::Minimum,
            (Object, "maxVal") => Self::Maximum,
            (Object, "gameDefined") => Self::GameDefined,
            (Object, "oneShot") => Self::OneShot,
            (Object | Logic | LogicNode, "isDirty") => Self::Dirty,
            (Object, "fillSourceRoom") => Self::FillSourceRoom,
            (EventReference | ParameterReference, "ref") => Self::Reference,
            (LogicNode, "overriddenGUID") => Self::OverriddenGuid,
            (LogicNode, "ownerContext") => Self::OwnerContext,
            (LogicNode, "Parameter") => Self::Parameter,
            (LogicNode, "Event") => Self::Event,
            (LogicNode, "Source") => Self::Source,
            (LogicNode, "Rtpc") => Self::Rtpc,
            (LogicNode, "SetGlobal") => Self::SetGlobal,
            (Link, "node") => Self::Node,
            (Link, "sourceLinkId") => Self::SourceLinkId,
            (Link, "targetLinkId") => Self::TargetLinkId,
            (EditorPosition, "X") => Self::X,
            (EditorPosition, "Y") => Self::Y,
            _ => return None,
        })
    }

    fn value(self, raw: &str) -> CigAudioValue<'_> {
        match self {
            Self::Guid
            | Self::Category
            | Self::Parent
            | Self::StartTrigger
            | Self::Aggregate
            | Self::Reference
            | Self::OverriddenGuid
            | Self::OwnerContext
            | Self::Parameter
            | Self::Event
            | Self::Source
            | Self::Rtpc
            | Self::Node => CigAudioValue::GuidToken(raw),
            Self::GameDefined
            | Self::OneShot
            | Self::Dirty
            | Self::FillSourceRoom
            | Self::SetGlobal => match raw {
                "0" => CigAudioValue::Boolean(false),
                "1" => CigAudioValue::Boolean(true),
                _ => CigAudioValue::InvalidBoolean,
            },
            Self::PositionMode | Self::VoiceLimit | Self::SourceLinkId | Self::TargetLinkId => {
                raw.parse::<u32>().map_or(
                    CigAudioValue::InvalidUnsignedInteger,
                    CigAudioValue::UnsignedInteger,
                )
            }
            Self::Priority
            | Self::DurationMin
            | Self::DurationMax
            | Self::Radius
            | Self::Minimum
            | Self::Maximum
            | Self::X
            | Self::Y => raw
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
                .map_or(CigAudioValue::InvalidNumber, CigAudioValue::Number),
            Self::Type | Self::Class | Self::Id | Self::Name | Self::ContextString => {
                CigAudioValue::Text(raw)
            }
        }
    }
}
