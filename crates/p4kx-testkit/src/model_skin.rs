// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use half::f16;

use crate::bytes::{put_u16, put_u32};
use crate::model::{TestModel, TestNormals, TestPositions};

const INDICES: u32 = 0xEECD_C168;
const VERTS_UVS: u32 = 0x9132_9AE9;
const VERTS_UVS_2: u32 = 0xB3A7_0D5E;
const NORMALS: u32 = 0x9CF3_F615;
const NORMALS_2: u32 = 0x38A5_81FE;
const TANGENTS: u32 = 0xB95E_9A1B;
const QTANGENTS: u32 = 0xEE05_7252;
const UNKNOWN_STREAM: u32 = 0x9D51_C5EE;
const BONEMAP: u32 = 0x677C_7B23;
const BONEMAP32: u32 = 0x6ECA_3708;
const HEADER_PADDING: usize = 88;
const STREAM_ALIGNMENT: usize = 8;

pub(crate) fn skin_chunk(model: &TestModel) -> Vec<u8> {
    let (min, max) = bounds(model);
    let mut out = Vec::new();
    put_u32(&mut out, 0);
    put_u32(&mut out, 4);
    put_u32(&mut out, count(model.vertices.len()));
    put_u32(&mut out, count(model.indices.len()));
    put_u32(&mut out, count(model.submeshes.len()));
    put_u32(&mut out, 0);
    let (model_min, model_max) = model.model_box.unwrap_or((min, max));
    put_vector(&mut out, model_min);
    put_vector(&mut out, model_max);
    put_vector(&mut out, min);
    put_vector(&mut out, max);
    put_u32(&mut out, 4);
    put_u32(&mut out, 0);
    out.resize(out.len() + HEADER_PADDING, 0);
    for submesh in &model.submeshes {
        put_u16(&mut out, submesh.material_id);
        put_u16(&mut out, submesh.node);
        put_u32(&mut out, submesh.first_index);
        put_u32(&mut out, submesh.index_count);
        put_u32(&mut out, submesh.first_vertex);
        put_u32(&mut out, 0);
        put_u32(&mut out, submesh.vertex_count);
        out.extend_from_slice(&1.0_f32.to_le_bytes());
        put_vector(&mut out, [0.0; 3]);
        put_u32(&mut out, 0);
        put_u32(&mut out, 0);
    }
    put_indices(&mut out, model);
    put_vertices(&mut out, model, min, max);
    put_normals(&mut out, model);
    put_second_uvs(&mut out, model);
    put_bone_map(&mut out, model);
    stream(&mut out, UNKNOWN_STREAM, 2, |data| {
        for _ in &model.vertices {
            put_u16(data, 0);
        }
    });
    for _ in 0..model.repeated_index_streams {
        put_indices(&mut out, model);
    }
    out
}

fn put_second_uvs(out: &mut Vec<u8>, model: &TestModel) {
    let Some(uvs) = &model.second_uvs else {
        return;
    };
    stream(out, NORMALS_2, 4, |data| {
        for uv in uvs {
            for value in uv {
                put_u16(data, f16::from_f32(*value).to_bits());
            }
        }
    });
}

fn put_bone_map(out: &mut Vec<u8>, model: &TestModel) {
    let Some((wide, entries)) = &model.bone_map else {
        return;
    };
    let (tag, influences) = if *wide { (BONEMAP32, 8) } else { (BONEMAP, 4) };
    stream(out, tag, influences * 3, |data| {
        for vertex in 0..model.vertices.len() {
            let entry = entries.get(vertex).map_or_default(Vec::as_slice);
            let at = |slot: usize| entry.get(slot).copied().unwrap_or((0, 0));
            for slot in 0..influences as usize {
                put_u16(data, at(slot).0);
            }
            for slot in 0..influences as usize {
                data.push(at(slot).1);
            }
        }
    });
}

fn put_floats(out: &mut Vec<u8>, tag: u32, model: &TestModel) {
    stream(out, tag, 12, |data| {
        for vertex in &model.vertices {
            put_vector(data, vertex.normal);
        }
    });
}

fn put_tangent_frames(out: &mut Vec<u8>, model: &TestModel) {
    stream(out, QTANGENTS, 8, |data| {
        for vertex in &model.vertices {
            let (tangent, bitangent) = tangent_frame(vertex.normal);
            if model.tangent_sign < 0.0 {
                put_u32(data, pack_unit_vector(tangent) | (1 << 31));
                put_u32(data, pack_unit_vector(bitangent.map(|value| -value)));
            } else {
                put_u32(data, pack_unit_vector(tangent));
                put_u32(data, pack_unit_vector(bitangent));
            }
        }
    });
}

fn put_normals(out: &mut Vec<u8>, model: &TestModel) {
    match model.normals {
        TestNormals::Float => put_floats(out, NORMALS_2, model),
        TestNormals::Compressed => put_tangent_frames(out, model),
        TestNormals::FloatOnNormals => put_floats(out, NORMALS, model),
        TestNormals::PackedOnNormals => put_packed(out, NORMALS, model),
        TestNormals::Tangents16 => stream(out, TANGENTS, 16, |data| {
            for vertex in &model.vertices {
                put_quaternion(data, quaternion_of_normal(vertex.normal));
                put_quaternion(data, [0.0, 0.0, 0.0, 1.0]);
            }
        }),
        TestNormals::None => {}
    }
}

fn put_packed(out: &mut Vec<u8>, tag: u32, model: &TestModel) {
    stream(out, tag, 4, |data| {
        for vertex in &model.vertices {
            put_u32(data, pack_unit_vector(vertex.normal));
        }
    });
}

fn quaternion_of_normal([x, y, z]: [f32; 3]) -> [f32; 4] {
    let scale = 2.0_f32.mul_add(z, 2.0).sqrt();
    if scale < 1e-6 {
        [1.0, 0.0, 0.0, 0.0]
    } else {
        [-y / scale, x / scale, 0.0, (1.0 + z) / scale]
    }
}

fn put_quaternion(out: &mut Vec<u8>, quaternion: [f32; 4]) {
    for value in quaternion {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the value is clamped and rounded to the i16 range"
        )]
        let raw = (value.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&raw.to_le_bytes());
    }
}

fn put_indices(out: &mut Vec<u8>, model: &TestModel) {
    let size = if model.wide_indices { 4 } else { 2 };
    stream(out, INDICES, size, |data| {
        for &index in &model.indices {
            if model.wide_indices {
                put_u32(data, index);
            } else {
                put_u16(data, u16::try_from(index).unwrap_or(u16::MAX));
            }
        }
    });
}

fn put_vertices(out: &mut Vec<u8>, model: &TestModel, min: [f32; 3], max: [f32; 3]) {
    let (tag, size) = match model.positions {
        TestPositions::Quantized => (VERTS_UVS_2, 16),
        TestPositions::Float => (VERTS_UVS, 20),
    };
    stream(out, tag, size, |data| {
        for (at, vertex) in model.vertices.iter().enumerate() {
            match model.positions {
                TestPositions::Quantized => {
                    for axis in 0..3 {
                        let at = |values: [f32; 3]| values.get(axis).copied().unwrap_or(0.0);
                        let half = ((at(max) - at(min)) / 2.0).max(1.0);
                        let centre = f32::midpoint(at(max), at(min));
                        let snorm = ((at(vertex.position) - centre) / half).clamp(-1.0, 1.0);
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "the value is rounded and clamped to the i16 range"
                        )]
                        let raw = (snorm * 32767.0).round() as i16;
                        data.extend_from_slice(&raw.to_le_bytes());
                    }
                    put_u16(data, 0x7FFF);
                }
                TestPositions::Float => put_vector(data, vertex.position),
            }
            let [red, green, blue, alpha] = model
                .colours
                .as_ref()
                .and_then(|colours| colours.get(at))
                .copied()
                .unwrap_or([0xFF; 4]);
            data.extend_from_slice(&[blue, green, red, alpha]);
            for value in vertex.uv {
                put_u16(data, f16::from_f32(value).to_bits());
            }
        }
    });
}

fn stream(out: &mut Vec<u8>, tag: u32, element_size: u32, write: impl FnOnce(&mut Vec<u8>)) {
    put_u32(out, tag);
    put_u32(out, element_size);
    let start = out.len();
    write(out);
    let len = out.len() - start;
    out.resize(start + len.next_multiple_of(STREAM_ALIGNMENT), 0);
}

fn bounds(model: &TestModel) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for vertex in &model.vertices {
        for ((low, high), value) in min.iter_mut().zip(&mut max).zip(vertex.position) {
            *low = low.min(value);
            *high = high.max(value);
        }
    }
    if model.vertices.is_empty() {
        return ([0.0; 3], [0.0; 3]);
    }
    (min, max)
}

pub(crate) fn tangent_frame(normal: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let helper = if normal[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let tangent = normalise(cross(helper, normal));
    (tangent, cross(normal, tangent))
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1].mul_add(b[2], -(a[2] * b[1])),
        a[2].mul_add(b[0], -(a[0] * b[2])),
        a[0].mul_add(b[1], -(a[1] * b[0])),
    ]
}

fn normalise(vector: [f32; 3]) -> [f32; 3] {
    let len = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if len > 0.0 {
        vector.map(|value| value / len)
    } else {
        [1.0, 0.0, 0.0]
    }
}

fn pack_unit_vector(vector: [f32; 3]) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the component is clamped to −1…1, so the biased value lies in 0…32766"
    )]
    let pack = |value: f32| ((value.clamp(-1.0, 1.0) * 16383.0).round() + 16383.0) as u32;
    let z_sign = u32::from(vector[2] < 0.0) << 30;
    pack(vector[0]) | (pack(vector[1]) << 15) | z_sign
}

fn put_vector(out: &mut Vec<u8>, vector: [f32; 3]) {
    for value in vector {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

fn count(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
