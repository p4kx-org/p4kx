// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::Reference;
use crate::audio_control_payload::AtlPosition;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct EventTimeline<'source> {
    pub(super) position: AtlPosition,
    pub(super) target: TimelineTarget<'source>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum TimelineTarget<'source> {
    MissingName,
    AmbiguousName,
    Path {
        authored_path: &'source str,
        archive_path: Option<String>,
        position: AtlPosition,
        #[schemars(schema_with = "crate::schema::reference_definition")]
        reference: Box<Reference>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Event {
    pub(super) animation_element: DecimalU64,
    pub(super) position: AtlPosition,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct EventAttribute<'source> {
    pub(super) field: EventField,
    pub(super) qualified_name: &'source str,
    pub(super) namespace: Option<&'source str>,
    pub(super) value: &'source str,
    pub(super) lexeme: Option<&'source str>,
    pub(super) position: AtlPosition,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum EventField {
    Name,
    Time,
    EndTime,
    SeekTime,
    Bone,
    Offset,
    Direction,
    Parameter,
    Model,
    DropInZone,
    StopOnInterrupt,
    Unknown,
}

impl EventField {
    pub(super) fn of(name: &str, namespace: Option<&str>) -> Self {
        if namespace.is_some() {
            return Self::Unknown;
        }
        match name {
            "name" => Self::Name,
            "time" => Self::Time,
            "endTime" => Self::EndTime,
            "seekTime" => Self::SeekTime,
            "bone" => Self::Bone,
            "offset" => Self::Offset,
            "dir" => Self::Direction,
            "parameter" => Self::Parameter,
            "model" => Self::Model,
            "dropInZone" => Self::DropInZone,
            "stopOnInterrupt" => Self::StopOnInterrupt,
            _ => Self::Unknown,
        }
    }
}
