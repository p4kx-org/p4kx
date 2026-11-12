// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::item_component_owner::ComponentOwner;
use crate::catalog_read::original::PhysicalEntryRef;
use crate::schema_fields::every_field_required;

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(in crate::catalog_v3) struct InteriorRoomRef {
    pub(super) owner: ComponentOwner,
    pub(super) object_container_ordinal: SourcedValue<u32>,
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
    pub(super) entity_attributes: Vec<SocAttribute>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteriorSocPosition {
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

/// Scope and findings for one inspected direct room declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(in crate::catalog_v3) struct InteriorRoomCoverage {
    pub(super) room: InteriorRoomRef,
    pub(super) findings: Vec<InteriorFinding>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteriorFinding {
    pub(super) kind: InteriorFindingKind,
    pub(super) stream: Option<SourcedValue<String>>,
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
    UnexaminedChild,
    AmbiguousStreamName,
    MissingStream,
    AmbiguousStream,
    UnreadableStream,
    UnreadableEntityChunk,
    LossyEntityChunk,
    AmbiguousClass,
    InvalidClass,
}
