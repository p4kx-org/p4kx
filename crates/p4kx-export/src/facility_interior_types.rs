// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::item_component_owner::ComponentOwner;
use super::super::loadout_position::LoadoutPosition;
use super::child::InteriorChild;
use crate::catalog_read::original::PhysicalEntryRef;
use crate::schema_fields::every_field_required;

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(in crate::catalog_v3) struct InteriorRoomRef {
    pub(super) owner: ComponentOwner,
    #[serde(flatten)]
    pub(super) declaration: InteriorRoomDeclaration,
    pub(super) loadout_position: Option<SourcedValue<LoadoutPosition>>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(super) enum InteriorRoomDeclaration {
    Vehicle {
        object_container_ordinal: SourcedValue<u32>,
    },
    Item,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(in crate::catalog_v3) struct InteriorFacilityPlacement {
    pub(super) room: InteriorRoomRef,
    pub(super) position: SourcedValue<InteriorSocPosition>,
    pub(super) stream_name: SourcedValue<String>,
    pub(super) class_record_index: SourcedValue<u32>,
    pub(super) local_transform: SourcedValue<[[f32; 4]; 3]>,
    pub(super) root_room_transform: Option<SourcedValue<[[f32; 4]; 3]>>,
    pub(super) ship_frame_transform: SourcedValue<[[f32; 4]; 3]>,
    pub(super) entity_attributes: Vec<SocAttribute>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteriorSocPosition {
    pub(super) container_occurrence_ordinal: u32,
    pub(super) container: PhysicalEntryRef,
    pub(super) descriptor: PhysicalEntryRef,
    pub(super) descriptor_node_index: u32,
    pub(super) stream: PhysicalEntryRef,
    pub(super) chunk_index: u32,
    pub(super) entity_ordinal: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(super) struct SocAttribute {
    pub(super) name: SourcedValue<String>,
    pub(super) value: SourcedValue<String>,
}

/// Scope and findings for one original ship-room declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(in crate::catalog_v3) struct InteriorRoomCoverage {
    pub(super) room: InteriorRoomRef,
    pub(super) containers: Vec<InteriorRoomContainer>,
    pub(super) findings: Vec<InteriorFinding>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteriorRoomContainer {
    pub(super) parent_ordinal: Option<u32>,
    pub(super) container: SourcedValue<PhysicalEntryRef>,
    pub(super) child: Option<InteriorChild>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteriorFinding {
    pub(super) kind: InteriorFindingKind,
    pub(super) container_ordinal: Option<u32>,
    pub(super) stream: Option<SourcedValue<String>>,
    pub(super) child: Option<InteriorChild>,
}

impl InteriorRoomCoverage {
    pub(super) fn note(
        &mut self,
        kind: InteriorFindingKind,
        container_ordinal: Option<u32>,
        stream: Option<SourcedValue<String>>,
        child: Option<InteriorChild>,
    ) {
        self.findings.push(InteriorFinding {
            kind,
            container_ordinal,
            stream,
            child,
        });
    }
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(super) enum InteriorFindingKind {
    MissingContainer,
    AmbiguousContainer,
    UnreadableContainer,
    MissingDescriptor,
    AmbiguousDescriptor,
    UnreadableDescriptor,
    InvalidChildPath,
    AmbiguousChildName,
    ChildCycle,
    InvalidChildTransform,
    AmbiguousStreamName,
    MissingStream,
    AmbiguousStream,
    UnreadableStream,
    UnreadableEntityChunk,
    LossyEntityChunk,
    AmbiguousClass,
    InvalidClass,
}
