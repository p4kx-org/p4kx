// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::PhysicsFloat;
use schemars::JsonSchema;
use serde::Serialize;

use super::frame_hierarchy::PhysicsFrame;
use crate::SourceRange;

pub(in crate::provider::model_source) const TABLE: &str = "model-physics-table";
pub(in crate::provider::model_source) const SLOT: &str = "model-physics-slot";
pub(super) const VERTEX: &str = "model-physics-vertex";
pub(super) const TRIANGLE: &str = "model-physics-triangle";
pub(super) const NODE: &str = "model-physics-node";
pub(in crate::provider::model_source) const FRAME: &str = "model-physics-frame";

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PhysicsTableStatus {
    Read,
    UnsupportedVersion,
    Malformed,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PhysicsTable {
    pub(super) disposition: PhysicsTableStatus,
    pub(super) slot_count: Option<usize>,
    pub(super) slot_table: Option<SourceRange>,
    pub(super) trailing: Option<SourceRange>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PhysicsScalar {
    pub(super) value: f32,
    pub(super) bits: u32,
}

impl From<PhysicsFloat> for PhysicsScalar {
    fn from(value: PhysicsFloat) -> Self {
        Self {
            value: value.value(),
            bits: value.bits(),
        }
    }
}

pub(super) type Vector = [PhysicsScalar; 3];

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum GeometryFrame {
    SourceNodeZUpMetres,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "shape", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum PhysicsShape {
    Box {
        axes: [[PhysicsScalar; 3]; 3],
        orientation_word: u32,
        centre: Vector,
        half_size: Vector,
    },
    Sphere {
        centre: Vector,
        radius: PhysicsScalar,
    },
    Cylinder {
        centre: Vector,
        axis: Vector,
        radius: PhysicsScalar,
        half_height: PhysicsScalar,
    },
    Capsule {
        centre: Vector,
        axis: Vector,
        radius: PhysicsScalar,
        half_height: PhysicsScalar,
    },
    Trimesh {
        vertex_count: usize,
        triangle_count: usize,
        vertex_range: SourceRange,
        index_range: SourceRange,
        material_range: SourceRange,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "disposition", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum PhysicsSlotBlob {
    MissingPayload,
    DamageMesh,
    SignedDistanceField,
    UnknownMarker {
        marker: u32,
    },
    UnknownKind {
        selector: u32,
    },
    Malformed,
    Geometry {
        frame: GeometryFrame,
        geometry: Box<PhysicsShape>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PhysicsSlot {
    pub(super) packed_key: u32,
    pub(super) part: u16,
    pub(super) node: Option<u16>,
    pub(super) declared_length: u32,
    pub(super) declared_range: SourceRange,
    pub(super) retained_range: Option<SourceRange>,
    pub(super) blob: PhysicsSlotBlob,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PhysicsVertex {
    pub(super) position: Vector,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PhysicsTriangle {
    pub(super) indices: [u16; 3],
    pub(super) material_id: u8,
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum PhysicsPayload {
    Table(PhysicsTable),
    Node(PhysicsNode),
    Frame(PhysicsFrame),
    Slot(PhysicsSlot),
    Vertex(PhysicsVertex),
    Triangle(PhysicsTriangle),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PhysicsNode {
    pub(super) stored_id: u32,
    pub(super) parent_word: u16,
    pub(super) geometry_type: u16,
    pub(super) local_matrix_bits: [[u32; 4]; 3],
    pub(super) companion_matrix_bits: [[u32; 4]; 3],
    pub(super) scale_bits: [u32; 3],
}
