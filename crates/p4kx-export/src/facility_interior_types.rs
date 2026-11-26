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

/// One original SOC Entity located in a Ship room.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct InteriorFacilityPlacement {
    pub(super) room: InteriorRoomRef,
    pub(super) position: SourcedValue<InteriorSocPosition>,
    pub(super) stream_name: SourcedValue<String>,
    pub(super) class_record_index: SourcedValue<u32>,
    pub(super) local_transform: SourcedValue<[[f32; 4]; 3]>,
    pub(super) root_room_transform: Option<SourcedValue<[[f32; 4]; 3]>>,
    pub(super) ship_frame_transform: SourcedValue<[[f32; 4]; 3]>,
    pub(super) entity_attributes: Vec<SocAttribute>,
}

impl InteriorFacilityPlacement {
    /// Borrows the owner of the original room declaration.
    #[must_use]
    pub const fn room_owner(&self) -> &ComponentOwner {
        &self.room.owner
    }

    /// Borrows the ordered Loadout path of an attached room.
    #[must_use]
    pub const fn loadout_position(&self) -> Option<&SourcedValue<LoadoutPosition>> {
        self.room.loadout_position.as_ref()
    }

    /// Borrows the physical position of the original SOC Entity.
    #[must_use]
    pub const fn position(&self) -> &SourcedValue<InteriorSocPosition> {
        &self.position
    }

    /// Borrows the name of the stream declared by the room descriptor.
    #[must_use]
    pub const fn stream_name(&self) -> &SourcedValue<String> {
        &self.stream_name
    }

    /// Borrows the uniquely matched physical DataForge class Record index.
    #[must_use]
    pub const fn class_record_index(&self) -> &SourcedValue<u32> {
        &self.class_record_index
    }

    /// Borrows the row-major affine transform in the Entity's local room frame.
    #[must_use]
    pub const fn local_transform(&self) -> &SourcedValue<[[f32; 4]; 3]> {
        &self.local_transform
    }

    /// Borrows the row-major affine transform in the root room frame, when available.
    #[must_use]
    pub const fn root_room_transform(&self) -> Option<&SourcedValue<[[f32; 4]; 3]>> {
        self.root_room_transform.as_ref()
    }

    /// Borrows the row-major affine transform in the owning Ship's frame.
    #[must_use]
    pub const fn ship_frame_transform(&self) -> &SourcedValue<[[f32; 4]; 3]> {
        &self.ship_frame_transform
    }

    /// Borrows the original Entity attributes in their source order.
    #[must_use]
    pub fn entity_attributes(&self) -> &[SocAttribute] {
        &self.entity_attributes
    }
}

/// Original archive and container coordinates of one SOC Entity.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct InteriorSocPosition {
    /// Occurrence of the container under the root room.
    pub container_occurrence_ordinal: u32,
    /// Physical room container entry.
    pub container: PhysicalEntryRef,
    /// Physical XML descriptor entry.
    pub descriptor: PhysicalEntryRef,
    /// Node index of the stream declaration within the XML descriptor.
    pub descriptor_node_index: u32,
    /// Physical SOC stream entry.
    pub stream: PhysicalEntryRef,
    /// Chunk index within the stream.
    pub chunk_index: u32,
    /// Entity ordinal within the chunk.
    pub entity_ordinal: u32,
}

/// One original SOC Entity attribute.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct SocAttribute {
    /// Original attribute name.
    pub name: SourcedValue<String>,
    /// Original attribute value.
    pub value: SourcedValue<String>,
}

/// Scope and findings for one original ship-room declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct InteriorRoomCoverage {
    pub(super) room: InteriorRoomRef,
    /// The filename from the original vehicle slot or item declaration.
    file_name: SourcedValue<String>,
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
    /// Borrows the original filename and its source state.
    #[must_use]
    pub const fn file_name(&self) -> &SourcedValue<String> {
        &self.file_name
    }

    /// Borrows the physical declaration owner.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.room.owner
    }

    /// Borrows the ordered Loadout path of an attached room.
    #[must_use]
    pub const fn loadout_position(&self) -> Option<&SourcedValue<LoadoutPosition>> {
        self.room.loadout_position.as_ref()
    }

    pub(crate) fn is_complete(&self) -> bool {
        use p4kx_extraction::SourceValueState;
        let file = self.file_name.state();
        matches!(
            file,
            SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null
        ) && self.file_name.origin().source_set_complete()
            && self
                .room
                .loadout_position
                .as_ref()
                .is_none_or(|path| path.origin().source_set_complete())
            && self
                .containers
                .iter()
                .all(|container| container.container.origin().source_set_complete())
            && self.findings.is_empty()
    }

    pub(super) fn declared(room: InteriorRoomRef, file_name: &SourcedValue<&str>) -> Self {
        Self {
            room,
            file_name: file_name.map_ref(|name| (*name).to_owned()),
            containers: Vec::new(),
            findings: Vec::new(),
        }
    }

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
