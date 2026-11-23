// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The cube model and material libraries of a scene, and the helpers that write and read it back.

use std::error::Error;

use p4kx_chunks::ChunkFile;
use p4kx_gltf::{GlbOptions, GlbSummary, Scene, TextureLoader, write_scene_glb_with};
use p4kx_model::{
    CompanionError, CompanionPath, LodSelection, MaterialReading, MaterialStatus, Model,
    PositionBox, Transform,
};
use p4kx_testkit::{CheckedGlb, TestModel, TestMtl, TestMtlMaterial, check_glb};

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

const LIBRARY: &str = "objects/test/cube";

pub(crate) fn read_cube() -> Result<Model, Box<dyn Error>> {
    let cube = TestModel::cube();
    let cgfm = cube.cgfm();
    let mut companions = move |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("cube.cgfm") => Some(cgfm.clone()),
            _ => None,
        })
    };
    Ok(Model::read_with_materials(
        "cube.cgf",
        &ChunkFile::parse(cube.cgf())?,
        &mut companions,
        LodSelection::All,
        PositionBox::default(),
        MaterialReading::Skipped,
    )?)
}

pub(crate) fn add_cube_tangents(model: &mut Model) -> TestResult {
    let mesh = model
        .lods
        .first_mut()
        .and_then(|lod| lod.meshes.first_mut())
        .ok_or("cube mesh")?;
    let cube = TestModel::cube();
    let (faces, _) = cube.vertices().as_chunks::<4>();
    let tangents: Vec<_> = faces
        .iter()
        .flat_map(|[origin, right, ..]| {
            let [x, y, z] = origin.position;
            let [right_x, right_y, right_z] = right.position;
            let edge = [right_x - x, right_y - y, right_z - z];
            let length = edge.iter().map(|value| value * value).sum::<f32>().sqrt();
            let [x, y, z] = edge.map(|value| value / length);
            // The glTF writer reverses the authored handedness.
            [[x, y, z, -1.0]; 4]
        })
        .collect();
    mesh.tangents = Some(
        mesh.source
            .iter()
            .map(|&source| {
                let index = usize::try_from(source)?;
                tangents
                    .get(index)
                    .copied()
                    .ok_or_else(|| "cube vertex".into())
            })
            .collect::<Result<_, Box<dyn Error>>>()?,
    );
    Ok(())
}

pub(crate) fn library(sub_materials: &[TestMtlMaterial]) -> MaterialStatus {
    let file = TestMtl::multi(sub_materials).cryxml();
    let mut companions = move |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(LIBRARY) => Some(file.clone()),
            _ => None,
        })
    };
    MaterialStatus::load(Some(LIBRARY), &mut companions)
}

pub(crate) const fn translation(x: f32, y: f32, z: f32) -> Transform {
    Transform {
        rows: [[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]],
    }
}

pub(crate) fn write(
    scene: &Scene<'_>,
    options: &GlbOptions,
    textures: &dyn TextureLoader,
) -> Result<(CheckedGlb, GlbSummary), Box<dyn Error>> {
    let mut bytes = Vec::new();
    let summary = write_scene_glb_with(scene, options, textures, &mut bytes)?;
    assert_eq!(summary.bytes, bytes.len() as u64, "summary length");
    Ok((check_glb(&bytes)?, summary))
}

pub(crate) fn base_colours(glb: &CheckedGlb) -> Vec<[f32; 3]> {
    let mut colours: Vec<[f32; 3]> = glb
        .document
        .materials()
        .map(|material| {
            let [r, g, b, _] = material.pbr_metallic_roughness().base_color_factor();
            [r, g, b]
        })
        .collect();
    colours.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    colours
}

pub(crate) fn close(actual: [f32; 3], expected: [f32; 3]) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(a, e)| (a - e).abs() < 1e-5)
}
