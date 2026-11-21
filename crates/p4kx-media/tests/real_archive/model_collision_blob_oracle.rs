// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use serde_json::{Value, json};

const WORD: usize = size_of::<u32>();
const SHORT: usize = size_of::<u16>();
const TRIPLE: usize = 3;
const SHAPE_AT: usize = 68;
const SHAPE_PAYLOAD: usize = SHAPE_AT + WORD;
const MESH_MAP_FLAGS: usize = 92;

pub(in crate::model_collision_replay) struct Blob<'a> {
    shape: Shape<'a>,
    base: u64,
}

enum Shape<'a> {
    Missing,
    Damage,
    DistanceField,
    UnknownMarker(u32),
    UnknownKind(u32),
    Malformed,
    Box {
        axes: [[u32; 3]; 3],
        orientation: u32,
        centre: [u32; 3],
        half_size: [u32; 3],
    },
    Sphere {
        centre: [u32; 3],
        radius: u32,
    },
    Axial {
        name: &'static str,
        centre: [u32; 3],
        axis: [u32; 3],
        radius: u32,
        half_height: u32,
    },
    Mesh(Mesh<'a>),
}

impl<'a> Blob<'a> {
    pub(super) fn read(bytes: Option<&'a [u8]>, base: u64) -> Self {
        let shape = bytes.map_or(Shape::Missing, |data| {
            shape(data).unwrap_or(Shape::Malformed)
        });
        Self { shape, base }
    }

    pub(super) fn value(&self) -> Value {
        let geometry = match &self.shape {
            Shape::Missing => return json!({"disposition": "missing-payload"}),
            Shape::Damage => return json!({"disposition": "damage-mesh"}),
            Shape::DistanceField => return json!({"disposition": "signed-distance-field"}),
            Shape::UnknownMarker(marker) => {
                return json!({"disposition": "unknown-marker", "marker": marker});
            }
            Shape::UnknownKind(selector) => {
                return json!({"disposition": "unknown-kind", "selector": selector});
            }
            Shape::Malformed => return json!({"disposition": "malformed"}),
            Shape::Box {
                axes,
                orientation,
                centre,
                half_size,
            } => json!({
                "shape": "box", "axes": axes.map(vector_value), "orientation_word": orientation,
                "centre": vector_value(*centre), "half_size": vector_value(*half_size),
            }),
            Shape::Sphere { centre, radius } => json!({
                "shape": "sphere", "centre": vector_value(*centre), "radius": scalar(*radius),
            }),
            Shape::Axial {
                name,
                centre,
                axis,
                radius,
                half_height,
            } => json!({
                "shape": name, "centre": vector_value(*centre), "axis": vector_value(*axis),
                "radius": scalar(*radius), "half_height": scalar(*half_height),
            }),
            Shape::Mesh(mesh) => mesh.shape_value(self.base),
        };
        json!({"disposition": "geometry", "frame": "source-node-z-up-metres", "geometry": geometry})
    }

    pub(super) const fn name(&self) -> &'static str {
        match &self.shape {
            Shape::Missing => "missing-payload",
            Shape::Damage => "damage-mesh",
            Shape::DistanceField => "signed-distance-field",
            Shape::UnknownMarker(_) => "unknown-marker",
            Shape::UnknownKind(_) => "unknown-kind",
            Shape::Malformed => "malformed",
            Shape::Box { .. } => "box",
            Shape::Sphere { .. } => "sphere",
            Shape::Axial { name, .. } => name,
            Shape::Mesh(_) => "trimesh",
        }
    }

    pub(super) const fn is_partial(&self) -> bool {
        matches!(self.shape, Shape::Missing | Shape::Malformed)
    }

    pub(in crate::model_collision_replay) const fn mesh(&self) -> Option<&Mesh<'a>> {
        match &self.shape {
            Shape::Mesh(mesh) => Some(mesh),
            _ => None,
        }
    }
}

fn shape(data: &[u8]) -> Option<Shape<'_>> {
    match word(data, 0)? {
        0xCAFE_BABE => return Some(Shape::Damage),
        0xDEAD_BEB5 => return Some(Shape::DistanceField),
        2 => (),
        marker => return Some(Shape::UnknownMarker(marker)),
    }
    let start = SHAPE_PAYLOAD;
    match word(data, SHAPE_AT)? {
        0 => Some(Shape::Box {
            axes: [
                vector(data, start)?,
                vector(data, start + 3 * WORD)?,
                vector(data, start + 6 * WORD)?,
            ],
            orientation: word(data, start + 9 * WORD)?,
            centre: vector(data, start + 10 * WORD)?,
            half_size: vector(data, start + 13 * WORD)?,
        }),
        1 => Mesh::read(data).map(Shape::Mesh),
        4 => Some(Shape::Sphere {
            centre: vector(data, start)?,
            radius: finite_word(data, start + 3 * WORD)?,
        }),
        kind @ (5 | 6) => Some(Shape::Axial {
            name: if kind == 5 { "cylinder" } else { "capsule" },
            centre: vector(data, start)?,
            axis: vector(data, start + 3 * WORD)?,
            radius: finite_word(data, start + 6 * WORD)?,
            half_height: finite_word(data, start + 7 * WORD)?,
        }),
        kind => Some(Shape::UnknownKind(kind)),
    }
}

pub(in crate::model_collision_replay) struct Mesh<'a> {
    material_start: usize,
    materials: &'a [u8],
    positions: &'a [[u8; TRIPLE * WORD]],
    triangles: &'a [[u8; TRIPLE * SHORT]],
}

impl<'a> Mesh<'a> {
    fn read(data: &'a [u8]) -> Option<Self> {
        let vertices = count(data, SHAPE_PAYLOAD)?;
        let triangles = count(data, SHAPE_PAYLOAD + WORD)?;
        let leading = count(data, MESH_MAP_FLAGS - WORD)?;
        let mapped = *data.get(MESH_MAP_FLAGS)? != 0;
        let map_words = leading.checked_add(if mapped { triangles } else { 0 })?;
        let material_start = (MESH_MAP_FLAGS + SHORT).checked_add(map_words.checked_mul(SHORT)?)?;
        let vertex_start = material_start.checked_add(triangles)?;
        let triangle_start = vertex_start.checked_add(vertices.checked_mul(TRIPLE * WORD)?)?;
        let end = triangle_start.checked_add(triangles.checked_mul(TRIPLE * SHORT)?)?;
        let materials = data.get(material_start..vertex_start)?;
        let positions = data
            .get(vertex_start..triangle_start)?
            .as_chunks::<{ TRIPLE * WORD }>()
            .0;
        let triangles = data
            .get(triangle_start..end)?
            .as_chunks::<{ TRIPLE * SHORT }>()
            .0;
        if positions.iter().any(|row| vector(row, 0).is_none())
            || triangles.iter().any(|row| {
                index_words(*row)
                    .into_iter()
                    .any(|index| usize::from(index) >= vertices)
            })
        {
            return None;
        }
        Some(Self {
            material_start,
            materials,
            positions,
            triangles,
        })
    }

    pub(super) const fn vertices(&self) -> usize {
        self.positions.len()
    }

    pub(super) const fn triangles(&self) -> usize {
        self.triangles.len()
    }

    pub(in crate::model_collision_replay) fn vertex(&self, ordinal: usize) -> Option<Value> {
        Some(json!({"position": vector_value(vector(self.positions.get(ordinal)?, 0)?)}))
    }

    pub(in crate::model_collision_replay) fn triangle(&self, ordinal: usize) -> Option<Value> {
        let indices = index_words(*self.triangles.get(ordinal)?);
        Some(json!({"indices": indices, "material_id": self.materials.get(ordinal)?}))
    }

    fn shape_value(&self, base: u64) -> Value {
        let material = base + self.material_start as u64;
        let vertex = material + self.materials.len() as u64;
        let index = vertex + (self.positions.len() * TRIPLE * WORD) as u64;
        let end = index + (self.triangles.len() * TRIPLE * SHORT) as u64;
        json!({
            "shape": "trimesh",
            "vertex_count": self.vertices(), "triangle_count": self.triangles(),
            "material_range": range_value(material..vertex),
            "vertex_range": range_value(vertex..index),
            "index_range": range_value(index..end),
        })
    }
}

pub(super) fn range_value(range: Range<u64>) -> Value {
    json!({"basis": {"type": "entry"}, "start": range.start, "end_exclusive": range.end})
}

pub(super) fn word(bytes: &[u8], offset: usize) -> Option<u32> {
    let field = bytes.get(offset..offset.checked_add(WORD)?)?;
    Some(u32::from_le_bytes(field.try_into().ok()?))
}

fn count(bytes: &[u8], offset: usize) -> Option<usize> {
    let value = i32::from_le_bytes(word(bytes, offset)?.to_le_bytes());
    usize::try_from(value).ok()
}

const fn index_words(
    [first_lo, first_hi, second_lo, second_hi, third_lo, third_hi]: [u8; 6],
) -> [u16; 3] {
    [
        u16::from_le_bytes([first_lo, first_hi]),
        u16::from_le_bytes([second_lo, second_hi]),
        u16::from_le_bytes([third_lo, third_hi]),
    ]
}

fn finite_word(bytes: &[u8], offset: usize) -> Option<u32> {
    let bits = word(bytes, offset)?;
    f32::from_bits(bits).is_finite().then_some(bits)
}

fn vector(bytes: &[u8], offset: usize) -> Option<[u32; 3]> {
    Some([
        finite_word(bytes, offset)?,
        finite_word(bytes, offset + WORD)?,
        finite_word(bytes, offset + 2 * WORD)?,
    ])
}

fn vector_value(words: [u32; 3]) -> [Value; 3] {
    words.map(scalar)
}

fn scalar(bits: u32) -> Value {
    json!({"bits": bits, "value": f32::from_bits(bits)})
}
