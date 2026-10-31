// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `COLOR_0` only on the primitives whose material reads the vertex colours.

use std::error::Error;

use p4kx_chunks::ChunkFile;
use p4kx_gltf::{GlbOptions, write_glb};
use p4kx_model::{CompanionError, CompanionPath, Model};
use p4kx_testkit::gltf::{Accessor, Semantic};
use p4kx_testkit::{
    CUBE_MATERIAL_LIBRARY, CheckedGlb, TestModel, TestMtl, TestMtlMaterial, check_glb,
};

type TestResult = Result<(), Box<dyn Error>>;

pub(crate) fn glass(name: &str, mask: &str) -> TestMtlMaterial {
    TestMtlMaterial::new(name)
        .shader("GlassPBR")
        .string_gen_mask(mask)
}

/// Returns the GLB of the testkit cube with vertex colours and the given materials.
pub(crate) fn glb_bytes(materials: &[TestMtlMaterial]) -> Result<Vec<u8>, Box<dyn Error>> {
    glb_bytes_with(materials, &GlbOptions::default())
}

fn glb_bytes_with(
    materials: &[TestMtlMaterial],
    options: &GlbOptions,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let cube = TestModel::cube().with_colours(vec![[10, 20, 30, 40]; 24]);
    let cgfm = cube.cgfm();
    let mtl = TestMtl::multi(materials).cryxml();
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("cube.cgfm") => Some(cgfm.clone()),
            CompanionPath::Material(CUBE_MATERIAL_LIBRARY) => Some(mtl.clone()),
            _ => None,
        })
    };
    let model = Model::read("cube.cgf", &ChunkFile::parse(cube.cgf())?, &mut companions)?;
    let mut bytes = Vec::new();
    write_glb(&model, options, &mut bytes)?;
    Ok(bytes)
}

fn export(materials: &[TestMtlMaterial]) -> Result<CheckedGlb, Box<dyn Error>> {
    Ok(check_glb(&glb_bytes(materials)?)?)
}

fn export_with(
    materials: &[TestMtlMaterial],
    options: &GlbOptions,
) -> Result<CheckedGlb, Box<dyn Error>> {
    Ok(check_glb(&glb_bytes_with(materials, options)?)?)
}

fn has_colours(glb: &CheckedGlb) -> Result<Vec<bool>, Box<dyn Error>> {
    let mesh = glb.document.meshes().next().ok_or("a mesh")?;
    Ok(mesh
        .primitives()
        .map(|primitive| primitive.get(&Semantic::Colors(0)).is_some())
        .collect())
}

#[test]
fn a_tinted_and_an_untinted_glass_primitive_of_one_mesh_differ_in_color_0() -> TestResult {
    let glb = export(&[
        glass("unused", ""),
        glass("glass_ext", "%NORMAL_MAP"),
        glass("lens", "%PER_VERTEX_TINT"),
    ])?;
    assert_eq!(
        has_colours(&glb)?,
        [false, true],
        "the caps are untinted glass, the sides tinted"
    );
    Ok(())
}

#[test]
fn a_mesh_whose_materials_all_ignore_the_colours_has_no_color_0() -> TestResult {
    let glb = export(&[
        glass("unused", ""),
        glass("a", "%NORMAL_MAP"),
        TestMtlMaterial::new("b"),
    ])?;
    assert_eq!(has_colours(&glb)?, [false, false]);
    assert_eq!(colour_accessors(&glb), 0, "no colour data is written");
    Ok(())
}

#[test]
fn the_primitives_that_read_the_colours_share_one_accessor() -> TestResult {
    let glb = export(&[
        glass("unused", ""),
        glass("a", "%PER_VERTEX_TINT"),
        glass("b", "%PER_VERTEX_TINT"),
    ])?;
    assert_eq!(has_colours(&glb)?, [true, true]);
    assert_eq!(colour_accessors(&glb), 1, "one accessor for the mesh");
    Ok(())
}

/// Counts the normalised accessors, which in this unskinned cube are the colours.
fn colour_accessors(glb: &CheckedGlb) -> usize {
    glb.document
        .accessors()
        .filter(Accessor::normalized)
        .count()
}

fn layered(name: &str) -> TestMtlMaterial {
    TestMtlMaterial::new(name)
        .shader("HardSurface")
        .layer_with(&[("Name", "a"), ("Path", "a/a.mtl")])
        .layer_with(&[("Name", "b"), ("Path", "a/b.mtl")])
}

#[test]
fn layered_surfaces_without_a_blend_map_carry_their_shares_only_when_asked() -> TestResult {
    let blended = layered("blended").texture("TexSlot8", "a/m_blend.tif");
    let materials = [glass("unused", ""), layered("free"), blended];
    let shares = GlbOptions::default().with_layer_vertex_colours();
    assert_eq!(
        has_colours(&export_with(&materials, &shares)?)?,
        [true, false],
        "the blend map takes the layers' shares instead"
    );
    assert_eq!(
        has_colours(&export(&materials)?)?,
        [false, false],
        "a viewer would multiply the shares into the colour as a tint"
    );
    Ok(())
}

#[test]
fn tinted_glass_keeps_its_colours_without_the_layer_shares() -> TestResult {
    let glb = export(&[
        glass("unused", ""),
        glass("plain", ""),
        glass("lens", "%PER_VERTEX_TINT"),
    ])?;
    assert_eq!(has_colours(&glb)?, [false, true]);
    Ok(())
}
