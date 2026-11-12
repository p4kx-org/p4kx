// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use gltf::accessor::{DataType, Dimensions};
use gltf::{Accessor, Node, Primitive, Semantic, Skin};

use crate::glb::CheckedGlb;

const WEIGHT_TOLERANCE: f32 = 2.0 / 255.0;
const BIND_TOLERANCE: f32 = 1e-5;

/// Checks the skins of `glb` and the skinned meshes against them: joints and inverse bind
/// matrices, the skeleton root, and the joints and weights of every skinned primitive.
pub(crate) fn check_skins(glb: &CheckedGlb) -> Result<(), String> {
    let parents = parents(glb);
    for skin in glb.document.skins() {
        check_skin(glb, &skin, &parents)?;
    }
    for node in glb.document.nodes() {
        check_node(glb, &node, &parents)?;
    }
    Ok(())
}

fn parents(glb: &CheckedGlb) -> Vec<Option<usize>> {
    let mut parents = vec![None; glb.document.nodes().len()];
    for node in glb.document.nodes() {
        for child in node.children() {
            if let Some(slot) = parents.get_mut(child.index()) {
                *slot = Some(node.index());
            }
        }
    }
    parents
}

fn check_skin(glb: &CheckedGlb, skin: &Skin<'_>, parents: &[Option<usize>]) -> Result<(), String> {
    let name = format!("skin {}", skin.index());
    let joints: Vec<usize> = skin.joints().map(|joint| joint.index()).collect();
    if joints.is_empty() {
        return Err(format!("{name} has no joints"));
    }
    let mut sorted = joints.clone();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.len() != joints.len() {
        return Err(format!("{name} lists a joint twice"));
    }
    if let Some(binds) = skin.inverse_bind_matrices() {
        check_binds(glb, &binds, joints.len(), &name)?;
    }
    if let Some(skeleton) = skin.skeleton() {
        for &joint in &joints {
            if !descends_from(parents, joint, skeleton.index()) {
                return Err(format!(
                    "{name}: joint node {joint} is no descendant of its skeleton node"
                ));
            }
        }
    }
    Ok(())
}

fn descends_from(parents: &[Option<usize>], node: usize, ancestor: usize) -> bool {
    let mut at = Some(node);
    for _ in 0..=parents.len() {
        match at {
            Some(current) if current == ancestor => return true,
            Some(current) => at = parents.get(current).copied().flatten(),
            None => return false,
        }
    }
    false
}

fn check_binds(
    glb: &CheckedGlb,
    binds: &Accessor<'_>,
    joints: usize,
    name: &str,
) -> Result<(), String> {
    if binds.dimensions() != Dimensions::Mat4 || binds.data_type() != DataType::F32 {
        return Err(format!(
            "{name}: the inverse bind matrices are no MAT4 of floats"
        ));
    }
    if binds.count() != joints {
        return Err(format!(
            "{name}: {} inverse bind matrices for {joints} joints",
            binds.count()
        ));
    }
    for matrix in glb.floats(binds).as_chunks::<16>().0 {
        let last_row = [matrix.get(3), matrix.get(7), matrix.get(11), matrix.get(15)];
        let affine = last_row
            .iter()
            .zip([0.0, 0.0, 0.0, 1.0])
            .all(|(value, expected)| {
                value.is_some_and(|value| (value - expected).abs() <= BIND_TOLERANCE)
            });
        if !affine {
            return Err(format!(
                "{name}: an inverse bind matrix is no affine matrix"
            ));
        }
    }
    Ok(())
}

fn check_node(glb: &CheckedGlb, node: &Node<'_>, parents: &[Option<usize>]) -> Result<(), String> {
    let name = format!("node {}", node.index());
    let skinned_primitives = node.mesh().map_or_default(|mesh| {
        mesh.primitives()
            .filter(|primitive| primitive.get(&Semantic::Joints(0)).is_some())
            .count()
    });
    match (node.skin(), node.mesh()) {
        (Some(_), None) => return Err(format!("{name} has a skin but no mesh")),
        (None, Some(_)) if skinned_primitives > 0 => {
            return Err(format!("{name} has a skinned mesh but no skin"));
        }
        (Some(skin), Some(mesh)) => {
            if parents.get(node.index()).copied().flatten().is_some() {
                return Err(format!("{name} has a skinned mesh but is no root"));
            }
            for primitive in mesh.primitives() {
                check_primitive(glb, &primitive, skin.joints().count(), &name)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_primitive(
    glb: &CheckedGlb,
    primitive: &Primitive<'_>,
    joint_count: usize,
    name: &str,
) -> Result<(), String> {
    let vertices = primitive
        .get(&Semantic::Positions)
        .map_or(0, |positions| positions.count());
    let mut sets = Vec::new();
    for set in 0.. {
        let (joints, weights) = (
            primitive.get(&Semantic::Joints(set)),
            primitive.get(&Semantic::Weights(set)),
        );
        match (joints, weights) {
            (Some(joints), Some(weights)) => {
                sets.push((
                    decode_joints(glb, &joints, name)?,
                    decode_weights(glb, &weights, name)?,
                ));
            }
            (None, None) => break,
            _ => {
                return Err(format!(
                    "{name}: JOINTS_{set} and WEIGHTS_{set} come in pairs"
                ));
            }
        }
    }
    if sets.is_empty() {
        return Err(format!(
            "{name}: a primitive of a skinned mesh has no JOINTS_0"
        ));
    }
    for vertex in 0..vertices {
        check_vertex(&sets, vertex, joint_count, name)?;
    }
    Ok(())
}

type InfluenceSet = (Vec<u16>, Vec<f32>);

fn check_vertex(
    sets: &[InfluenceSet],
    vertex: usize,
    joint_count: usize,
    name: &str,
) -> Result<(), String> {
    let mut used = Vec::new();
    let mut total = 0.0;
    for (joints, weights) in sets {
        let at = vertex * 4..vertex * 4 + 4;
        let (Some(joints), Some(weights)) = (joints.get(at.clone()), weights.get(at)) else {
            return Err(format!("{name}: the joints of vertex {vertex} are missing"));
        };
        for (&joint, &weight) in joints.iter().zip(weights) {
            if usize::from(joint) >= joint_count {
                return Err(format!("{name}: joint {joint} of {joint_count}"));
            }
            if weight > 0.0 {
                if used.contains(&joint) {
                    return Err(format!(
                        "{name}: vertex {vertex} is weighted twice to joint {joint}"
                    ));
                }
                used.push(joint);
            } else if joint != 0 {
                return Err(format!(
                    "{name}: vertex {vertex} has joint {joint} with a weight of 0"
                ));
            }
            total += weight;
        }
    }
    if (total - 1.0).abs() > WEIGHT_TOLERANCE {
        return Err(format!(
            "{name}: the weights of vertex {vertex} add up to {total}"
        ));
    }
    Ok(())
}

fn decode_joints(glb: &CheckedGlb, joints: &Accessor<'_>, name: &str) -> Result<Vec<u16>, String> {
    if joints.dimensions() != Dimensions::Vec4 {
        return Err(format!("{name}: JOINTS is no VEC4"));
    }
    let bytes = glb.bytes(joints);
    match joints.data_type() {
        DataType::U8 => Ok(bytes.iter().map(|&value| u16::from(value)).collect()),
        DataType::U16 => Ok(bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_le_bytes(pair))
            .collect()),
        other => Err(format!("{name}: JOINTS of type {other:?}")),
    }
}

fn decode_weights(
    glb: &CheckedGlb,
    weights: &Accessor<'_>,
    name: &str,
) -> Result<Vec<f32>, String> {
    if weights.dimensions() != Dimensions::Vec4 {
        return Err(format!("{name}: WEIGHTS is no VEC4"));
    }
    let bytes = glb.bytes(weights);
    match (weights.data_type(), weights.normalized()) {
        (DataType::F32, _) => Ok(glb.floats(weights)),
        (DataType::U8, true) => Ok(bytes
            .iter()
            .map(|&value| f32::from(value) / f32::from(u8::MAX))
            .collect()),
        (DataType::U16, true) => Ok(bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| f32::from(u16::from_le_bytes(pair)) / f32::from(u16::MAX))
            .collect()),
        (other, _) => Err(format!(
            "{name}: WEIGHTS of type {other:?} that are not normalised"
        )),
    }
}
