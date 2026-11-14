// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use gltf::accessor::{DataType, Dimensions};
use gltf::{Accessor, Document, Gltf, Semantic};

use crate::glb_animation::check_animations;
use crate::glb_container::split_glb;
use crate::glb_morph::{check_morph_targets, check_sparse};
use crate::glb_skin::check_skins;

/// A GLB file that passed [`check_glb`], with its document and binary chunk.
#[derive(Debug)]
pub struct CheckedGlb {
    /// The glTF document.
    pub document: Document,
    pub(crate) blob: Vec<u8>,
    pub(crate) json: gltf::json::Value,
}

impl CheckedGlb {
    /// Returns the values of a float accessor, component after component.
    #[must_use]
    pub fn floats(&self, accessor: &Accessor<'_>) -> Vec<f32> {
        if accessor.sparse().is_some() {
            return self.sparse_floats(accessor);
        }
        self.plain_floats(accessor)
    }

    pub(crate) fn plain_floats(&self, accessor: &Accessor<'_>) -> Vec<f32> {
        self.elements(accessor)
            .flat_map(|bytes| {
                bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .copied()
                    .map(f32::from_le_bytes)
            })
            .collect()
    }

    /// Returns the bytes of an accessor, element after element.
    #[must_use]
    pub fn bytes(&self, accessor: &Accessor<'_>) -> Vec<u8> {
        self.elements(accessor).flatten().copied().collect()
    }

    /// Returns the values of an unsigned integer accessor.
    #[must_use]
    pub fn indices(&self, accessor: &Accessor<'_>) -> Vec<u32> {
        let size = accessor.data_type().size();
        self.elements(accessor)
            .map(|bytes| match (size, bytes) {
                (1, [value, ..]) => u32::from(*value),
                (2, [a, b, ..]) => u32::from(u16::from_le_bytes([*a, *b])),
                (4, [a, b, c, d, ..]) => u32::from_le_bytes([*a, *b, *c, *d]),
                _ => u32::MAX,
            })
            .collect()
    }

    fn elements<'a>(&'a self, accessor: &Accessor<'_>) -> impl Iterator<Item = &'a [u8]> + 'a {
        let element = accessor.size();
        let (start, stride) = accessor.view().map_or((0, element), |view| {
            (
                view.offset() + accessor.offset(),
                view.stride().unwrap_or(element),
            )
        });
        let blob = &self.blob;
        (0..accessor.count()).filter_map(move |index| {
            let at = start.checked_add(index.checked_mul(stride)?)?;
            blob.get(at..at.checked_add(element)?)
        })
    }
}

/// Checks a GLB file against the glTF 2.0 schema and the Khronos validator's data rules.
///
/// The rules are those a writer can break: container lengths and padding, buffer, view and accessor
/// bounds and alignment, finite floats, exact `POSITION` bounds, unit normals, unit tangents with
/// a handedness of ±1, colour types, index range and primitive restart values, triangle counts,
/// unit rotations and a node forest, and for skins unique joints, affine inverse bind matrices,
/// the skeleton root, skinned meshes at the root of the scene and per vertex weights that add up
/// to 1 over bones the skin has, and for animations distinct channel targets, strictly increasing
/// key times with exact `min` and `max`, one output per key of the property's type and unit
/// rotations, and for sparse accessors strictly ascending indices below the count with unshared,
/// in-bounds views, and for morph targets one `POSITION` delta accessor with `min` and `max` of
/// the primitive's vertex count per target, the same number of targets on every primitive of a
/// mesh and default weights and target names that match them.
///
/// # Errors
///
/// Returns the first violation found as text.
pub fn check_glb(bytes: &[u8]) -> Result<CheckedGlb, String> {
    let (json, blob) = split_glb(bytes)?;
    let gltf = Gltf::from_slice(bytes).map_err(|error| format!("invalid glTF: {error}"))?;
    std::str::from_utf8(json).map_err(|_| "the JSON chunk is no UTF-8".to_owned())?;
    let checked = CheckedGlb {
        document: gltf.document,
        blob: blob.to_vec(),
        json: gltf::json::deserialize::from_slice(json)
            .map_err(|error| format!("invalid JSON: {error}"))?,
    };
    check_buffers(&checked)?;
    check_accessors(&checked)?;
    check_meshes(&checked)?;
    check_morph_targets(&checked)?;
    check_nodes(&checked)?;
    crate::glb_images::check_textures(&checked)?;
    check_skins(&checked)?;
    check_animations(&checked)?;
    crate::glb_lights::check_lights(&checked)?;
    Ok(checked)
}

fn check_buffers(glb: &CheckedGlb) -> Result<(), String> {
    for buffer in glb.document.buffers() {
        if !matches!(buffer.source(), gltf::buffer::Source::Bin) || buffer.index() != 0 {
            return Err("a GLB writer keeps every buffer in the binary chunk".to_owned());
        }
        let len = glb.blob.len();
        if buffer.length() > len || len - buffer.length() >= 4 {
            return Err(format!(
                "buffer of {} bytes in a binary chunk of {len}",
                buffer.length()
            ));
        }
    }
    for view in glb.document.views() {
        let end = view.offset().checked_add(view.length());
        if end.is_none_or(|end| end > view.buffer().length()) {
            return Err(format!("bufferView {} exceeds its buffer", view.index()));
        }
    }
    Ok(())
}

fn check_accessors(glb: &CheckedGlb) -> Result<(), String> {
    for accessor in glb.document.accessors() {
        let index = accessor.index();
        check_sparse(glb, &accessor)?;
        let Some(view) = accessor.view() else {
            if accessor.sparse().is_some() && accessor.count() > 0 {
                if accessor.data_type() == DataType::F32 {
                    check_floats(glb, &accessor)?;
                }
                continue;
            }
            return Err(format!("accessor {index} has no bufferView"));
        };
        let component = accessor.data_type().size();
        if accessor.count() == 0 {
            return Err(format!("accessor {index} is empty"));
        }
        if (view.offset() + accessor.offset()) % component != 0 {
            return Err(format!(
                "accessor {index} is not aligned to its component size"
            ));
        }
        let stride = view.stride().unwrap_or_else(|| accessor.size());
        let end = accessor.offset() + stride * (accessor.count() - 1) + accessor.size();
        if end > view.length() {
            return Err(format!("accessor {index} exceeds its bufferView"));
        }
        if accessor.data_type() == DataType::F32 {
            check_floats(glb, &accessor)?;
        }
    }
    Ok(())
}

fn check_floats(glb: &CheckedGlb, accessor: &Accessor<'_>) -> Result<(), String> {
    let index = accessor.index();
    let values = glb.floats(accessor);
    if values.iter().any(|value| !value.is_finite()) {
        return Err(format!("accessor {index} holds a value that is not finite"));
    }
    let width = accessor.dimensions().multiplicity();
    for (bound, declared, pick) in [
        ("min", accessor.min(), f32::min as fn(f32, f32) -> f32),
        ("max", accessor.max(), f32::max),
    ] {
        let Some(declared) = declared else { continue };
        let actual: Vec<f32> = (0..width)
            .map(|component| {
                values
                    .iter()
                    .skip(component)
                    .step_by(width)
                    .copied()
                    .reduce(pick)
                    .unwrap_or(f32::NAN)
            })
            .collect();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the bound is compared as the f32 the data holds"
        )]
        let declared: Vec<f32> = declared.as_array().map_or_default(|values| {
            values
                .iter()
                .filter_map(gltf::json::Value::as_f64)
                .map(|value| value as f32)
                .collect()
        });
        if declared != actual {
            return Err(format!(
                "accessor {index} declares {bound} {declared:?}, its data has {actual:?}"
            ));
        }
    }
    Ok(())
}

fn check_meshes(glb: &CheckedGlb) -> Result<(), String> {
    for mesh in glb.document.meshes() {
        for primitive in mesh.primitives() {
            let name = format!("mesh {} primitive {}", mesh.index(), primitive.index());
            let position = primitive
                .get(&Semantic::Positions)
                .ok_or_else(|| format!("{name} has no POSITION"))?;
            if position.min().is_none() || position.max().is_none() {
                return Err(format!("{name}: POSITION lacks min or max"));
            }
            let vertices = position.count();
            for (semantic, attribute) in primitive.attributes() {
                if attribute.count() != vertices {
                    return Err(format!("{name}: {semantic:?} count differs from POSITION"));
                }
                match semantic {
                    Semantic::Normals => check_unit(glb, &attribute, &name)?,
                    Semantic::Tangents => check_tangents(glb, &attribute, &name)?,
                    Semantic::Colors(_) => check_colours(&attribute, &name)?,
                    _ => {}
                }
            }
            let indices = primitive
                .indices()
                .ok_or_else(|| format!("{name} has no indices"))?;
            check_indices(glb, &indices, vertices, &name)?;
        }
    }
    Ok(())
}

fn check_unit(glb: &CheckedGlb, normals: &Accessor<'_>, name: &str) -> Result<(), String> {
    if normals.dimensions() != Dimensions::Vec3 {
        return Err(format!("{name}: NORMAL is no VEC3"));
    }
    for normal in glb.floats(normals).as_chunks::<3>().0 {
        let len = normal.iter().map(|value| value * value).sum::<f32>().sqrt();
        if (len - 1.0).abs() > 0.0005 {
            return Err(format!("{name}: normal {normal:?} is not of unit length"));
        }
    }
    Ok(())
}

fn check_tangents(glb: &CheckedGlb, tangents: &Accessor<'_>, name: &str) -> Result<(), String> {
    if tangents.dimensions() != Dimensions::Vec4 || tangents.data_type() != DataType::F32 {
        return Err(format!("{name}: TANGENT is no VEC4 of floats"));
    }
    for tangent in glb.floats(tangents).as_chunks::<4>().0 {
        let [x, y, z, w] = *tangent;
        let len = z.mul_add(z, y.mul_add(y, x * x)).sqrt();
        if (len - 1.0).abs() > 0.0005 {
            return Err(format!("{name}: tangent {tangent:?} is not of unit length"));
        }
        if (w.abs() - 1.0).abs() > f32::EPSILON {
            return Err(format!(
                "{name}: tangent {tangent:?} has no handedness of ±1"
            ));
        }
    }
    Ok(())
}

fn check_colours(colours: &Accessor<'_>, name: &str) -> Result<(), String> {
    let wide = matches!(colours.dimensions(), Dimensions::Vec3 | Dimensions::Vec4);
    let normalised =
        matches!(colours.data_type(), DataType::U8 | DataType::U16) && colours.normalized();
    if !wide || !(normalised || colours.data_type() == DataType::F32) {
        return Err(format!(
            "{name}: COLOR is no VEC3 or VEC4 of normalised integers or floats"
        ));
    }
    Ok(())
}

fn check_indices(
    glb: &CheckedGlb,
    indices: &Accessor<'_>,
    vertices: usize,
    name: &str,
) -> Result<(), String> {
    let restart = match indices.data_type() {
        DataType::U8 => u32::from(u8::MAX),
        DataType::U16 => u32::from(u16::MAX),
        DataType::U32 => u32::MAX,
        other => return Err(format!("{name}: indices of type {other:?}")),
    };
    if !indices.count().is_multiple_of(3) {
        return Err(format!(
            "{name}: {} indices are no whole triangles",
            indices.count()
        ));
    }
    let values = glb.indices(indices);
    if values.len() != indices.count() {
        return Err(format!("{name}: indices exceed their buffer"));
    }
    if let Some(bad) = values
        .iter()
        .find(|&&index| index == restart || index as usize >= vertices)
    {
        return Err(format!("{name}: index {bad} with {vertices} vertices"));
    }
    Ok(())
}

fn check_nodes(glb: &CheckedGlb) -> Result<(), String> {
    let count = glb.document.nodes().len();
    let mut parents = vec![None; count];
    for node in glb.document.nodes() {
        let (_, rotation, _) = node.transform().decomposed();
        let len = rotation
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        if (len - 1.0).abs() > 0.000_005 {
            return Err(format!("node {} has no unit rotation", node.index()));
        }
        for child in node.children() {
            let slot = parents
                .get_mut(child.index())
                .ok_or_else(|| format!("node {} has an unknown child", node.index()))?;
            if slot.replace(node.index()).is_some() {
                return Err(format!("node {} has two parents", child.index()));
            }
        }
    }
    for start in 0..count {
        let mut at = start;
        for _ in 0..=count {
            match parents.get(at).copied().flatten() {
                Some(parent) => at = parent,
                None => break,
            }
            if at == start {
                return Err(format!("node {start} is its own ancestor"));
            }
        }
    }
    for scene in glb.document.scenes() {
        if let Some(node) = scene
            .nodes()
            .find(|node| parents.get(node.index()) != Some(&None))
        {
            return Err(format!("scene root {} has a parent", node.index()));
        }
    }
    Ok(())
}
