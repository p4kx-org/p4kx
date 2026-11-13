// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Materials as plain colours in the GLB of the testkit cube.

use std::error::Error;

use p4kx_chunks::ChunkFile;
use p4kx_gltf::{GlbOptions, GlbSummary, MaterialMode, write_glb};
use p4kx_model::{CompanionError, CompanionPath, Model};
use p4kx_testkit::gltf::material::AlphaMode;
use p4kx_testkit::{
    CUBE_CAPS_MATERIAL, CUBE_SIDES_MATERIAL, CheckedGlb, TestModel, TestMtl, TestMtlMaterial,
    check_glb,
};

type TestResult = Result<(), Box<dyn Error>>;

const NO_DRAW: u32 = 0x400;
const TWO_SIDED: u32 = 0x2;

fn model_with(mtl: Option<Vec<u8>>) -> Result<Model, Box<dyn Error>> {
    let cube = TestModel::cube();
    let cgfm = cube.cgfm();
    let mut companions = move |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("cube.cgfm") => Some(cgfm.clone()),
            CompanionPath::Material("objects/test/cube") => mtl.clone(),
            _ => None,
        })
    };
    Ok(Model::read(
        "cube.cgf",
        &ChunkFile::parse(cube.cgf())?,
        &mut companions,
    )?)
}

fn export(mtl: Option<Vec<u8>>) -> Result<(CheckedGlb, GlbSummary), Box<dyn Error>> {
    let mut bytes = Vec::new();
    let options = GlbOptions::default().with_materials(MaterialMode::Colours);
    let summary = write_glb(&model_with(mtl)?, &options, &mut bytes)?;
    Ok((check_glb(&bytes)?, summary))
}

fn matte(name: &str) -> TestMtlMaterial {
    TestMtlMaterial::new(name)
        .diffuse([0.2, 0.4, 0.6])
        .specular([0.04; 3])
        .shininess(204.0)
}

fn material_of(glb: &CheckedGlb, name: &str) -> Result<usize, Box<dyn Error>> {
    glb.document
        .materials()
        .find(|material| material.name() == Some(name))
        .and_then(|material| material.index())
        .ok_or_else(|| format!("no material {name}").into())
}

fn close(actual: &[f32], expected: &[f32]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (a - e).abs() < 1e-5)
}

#[test]
fn submeshes_get_the_material_of_their_id_with_its_colour_and_opacity() -> TestResult {
    let caps = matte("caps").diffuse([0.8, 0.1, 0.1]);
    let sides = matte("sides").opacity(0.5);
    let file = TestMtl::multi(&[matte("unused"), caps, sides]).cryxml();

    let (glb, summary) = export(Some(file))?;

    assert_eq!(summary.materials, 2, "only the materials in use");
    assert_eq!(summary.default_material_ids, 0, "no fallback");
    assert_eq!(summary.hidden_primitives, 0, "nothing hidden");
    assert_eq!(glb.document.textures().len(), 0, "no textures");
    assert_eq!(glb.document.images().len(), 0, "no images");
    let mesh = glb.document.meshes().next().ok_or("a mesh")?;
    let by_id: Vec<(usize, Option<&str>)> = mesh
        .primitives()
        .map(|p| {
            (
                p.material().index().unwrap_or(usize::MAX),
                p.material().name(),
            )
        })
        .collect();
    assert_eq!(
        by_id,
        [
            (material_of(&glb, "caps")?, Some("caps")),
            (material_of(&glb, "sides")?, Some("sides"))
        ],
        "caps are ID {CUBE_CAPS_MATERIAL}, sides ID {CUBE_SIDES_MATERIAL}"
    );
    let caps = glb
        .document
        .materials()
        .nth(material_of(&glb, "caps")?)
        .ok_or("caps")?;
    let pbr = caps.pbr_metallic_roughness();
    assert!(
        close(&pbr.base_color_factor(), &[0.8, 0.1, 0.1, 1.0]),
        "caps colour"
    );
    assert!(pbr.base_color_texture().is_none(), "no base colour texture");
    assert_eq!(caps.alpha_mode(), AlphaMode::Opaque, "opaque");
    let sides = glb
        .document
        .materials()
        .nth(material_of(&glb, "sides")?)
        .ok_or("sides")?;
    assert!(
        close(
            &sides.pbr_metallic_roughness().base_color_factor(),
            &[0.2, 0.4, 0.6, 0.5]
        ),
        "opacity is the alpha"
    );
    assert_eq!(sides.alpha_mode(), AlphaMode::Blend, "translucent blends");
    Ok(())
}

#[test]
fn a_single_material_serves_every_id_as_one_glb_material() -> TestResult {
    let (glb, summary) = export(Some(TestMtl::single(&matte("only")).text_xml()))?;

    assert_eq!(summary.materials, 1, "one material");
    assert_eq!(summary.primitives, 2, "two primitives");
    assert_eq!(summary.default_material_ids, 0);
    let mesh = glb.document.meshes().next().ok_or("a mesh")?;
    let used: Vec<Option<usize>> = mesh.primitives().map(|p| p.material().index()).collect();
    assert_eq!(used, [Some(0), Some(0)], "both use it");
    Ok(())
}

#[test]
fn roughness_follows_shininess_and_is_half_where_textures_carry_gloss() -> TestResult {
    let file = TestMtl::multi(&[
        matte("zero"),
        matte("glossy").shininess(255.0),
        matte("layered").shader("HardSurface").shininess(255.0),
    ])
    .cryxml();

    let (glb, _) = export(Some(file))?;

    let glossy = glb
        .document
        .materials()
        .nth(material_of(&glb, "glossy")?)
        .ok_or("glossy")?;
    assert!(
        glossy.pbr_metallic_roughness().roughness_factor().abs() < 1e-6,
        "shininess 255 is mirror smooth"
    );
    let layered = glb
        .document
        .materials()
        .nth(material_of(&glb, "layered")?)
        .ok_or("layered")?;
    assert!(
        (layered.pbr_metallic_roughness().roughness_factor() - 0.5).abs() < 1e-6,
        "a layered shader takes its gloss from textures"
    );
    Ok(())
}

#[test]
fn bright_specular_on_a_metal_surface_makes_a_metal_with_the_specular_as_colour() -> TestResult {
    let metal = matte("metal")
        .surface_type("metal_dense")
        .diffuse([0.9, 0.9, 0.9])
        .specular([0.9, 0.7, 0.5]);
    let plastic = matte("plastic")
        .surface_type("metal_dense")
        .specular([0.04; 3]);
    let file = TestMtl::multi(&[matte("unused"), metal, plastic]).cryxml();

    let (glb, _) = export(Some(file))?;

    let metal = glb
        .document
        .materials()
        .nth(material_of(&glb, "metal")?)
        .ok_or("metal")?;
    let pbr = metal.pbr_metallic_roughness();
    assert!((pbr.metallic_factor() - 1.0).abs() < 1e-6, "metallic");
    assert!(
        close(&pbr.base_color_factor(), &[0.9, 0.7, 0.5, 1.0]),
        "specular colour"
    );
    let plastic = glb
        .document
        .materials()
        .nth(material_of(&glb, "plastic")?)
        .ok_or("plastic")?;
    let pbr = plastic.pbr_metallic_roughness();
    assert!(
        pbr.metallic_factor().abs() < 1e-6,
        "a dim specular is a dielectric"
    );
    assert!(
        close(&pbr.base_color_factor(), &[0.2, 0.4, 0.6, 1.0]),
        "diffuse colour"
    );
    Ok(())
}

#[test]
fn alpha_test_masks_decals_and_glass_blend_and_two_sided_flags_carry_over() -> TestResult {
    let masked = matte("masked").alpha_test(0.4).opacity(0.5);
    let decal = matte("decal")
        .shader("MeshDecal")
        .texture("TexSlot1", "decal_diff.tif")
        .flags(TWO_SIDED);
    let file = TestMtl::multi(&[matte("unused"), masked, decal]).cryxml();
    let (glb, _) = export(Some(file))?;

    let masked = glb
        .document
        .materials()
        .nth(material_of(&glb, "masked")?)
        .ok_or("masked")?;
    assert_eq!(masked.alpha_mode(), AlphaMode::Mask, "alpha test masks");
    assert!(
        (masked.alpha_cutoff().unwrap_or(0.0) - 0.4).abs() < 1e-6,
        "cutoff"
    );
    assert!(!masked.double_sided(), "one-sided");
    let decal = glb
        .document
        .materials()
        .nth(material_of(&glb, "decal")?)
        .ok_or("decal")?;
    assert_eq!(decal.alpha_mode(), AlphaMode::Blend, "decals blend");
    assert!(decal.double_sided(), "decals and the flag draw both faces");

    let glass = TestMtl::multi(&[
        matte("unused"),
        matte("glass").shader("GlassPBR"),
        matte("x"),
    ])
    .cryxml();
    let (glb, _) = export(Some(glass))?;
    let glass = glb
        .document
        .materials()
        .nth(material_of(&glb, "glass")?)
        .ok_or("glass")?;
    assert_eq!(glass.alpha_mode(), AlphaMode::Blend, "glass blends");
    assert!(
        glass.pbr_metallic_roughness().base_color_factor()[3] < 1.0,
        "and is see-through"
    );
    Ok(())
}

#[test]
fn emissive_is_scaled_by_glow_and_kept_in_range() -> TestResult {
    let lit = matte("lit").emissive([0.5, 0.25, 0.0]).glow(1.0);
    let bright = matte("bright").emissive([1.0, 0.5, 0.0]).glow(4.0);
    let file = TestMtl::multi(&[matte("unused"), lit, bright]).cryxml();

    let (glb, _) = export(Some(file))?;
    let extension = "KHR_materials_emissive_strength";
    let root = glb.document.as_json();
    assert_eq!(
        root.extensions_used,
        [extension],
        "declare the extension once"
    );
    assert!(
        root.extensions_required.is_empty(),
        "core fallback remains usable"
    );
    assert_eq!(emissive_strength(&glb, "lit")?, None);
    assert_eq!(emissive_strength(&glb, "bright")?, Some(4.0));

    let lit = glb
        .document
        .materials()
        .nth(material_of(&glb, "lit")?)
        .ok_or("lit")?;
    assert!(
        close(&lit.emissive_factor(), &[0.5, 0.25, 0.0]),
        "emissive times glow"
    );
    let bright = glb
        .document
        .materials()
        .nth(material_of(&glb, "bright")?)
        .ok_or("bright")?;
    assert!(
        close(&bright.emissive_factor(), &[1.0, 0.5, 0.0]),
        "above 1 it is normalised: {:?}",
        bright.emissive_factor()
    );
    Ok(())
}

fn emissive_strength(glb: &CheckedGlb, name: &str) -> Result<Option<f32>, Box<dyn Error>> {
    Ok(glb
        .document
        .as_json()
        .materials
        .get(material_of(glb, name)?)
        .and_then(|material| material.extensions.as_ref())
        .and_then(|extensions| extensions.emissive_strength.as_ref())
        .map(|strength| strength.emissive_strength.0))
}

#[test]
fn emissive_magnitude_remains_distinct_and_ignores_nonfinite_input() -> TestResult {
    let (glb, _) = export(Some(
        TestMtl::multi(&[
            matte("unused"),
            matte("stronger").emissive([1.0, 0.5, 0.0]).glow(8.0),
            matte("invalid")
                .emissive([f32::NAN, f32::INFINITY, -1.0])
                .glow(4.0),
        ])
        .cryxml(),
    ))?;
    assert_eq!(emissive_strength(&glb, "stronger")?, Some(8.0));
    assert_eq!(emissive_strength(&glb, "invalid")?, None);
    let (glb, _) = export(Some(
        TestMtl::multi(&[
            matte("unused"),
            matte("dark").emissive([1.0; 3]),
            matte("x"),
        ])
        .cryxml(),
    ))?;
    assert_eq!(glb.document.as_json().extensions_used, [] as [String; 0]);
    let dark = glb
        .document
        .materials()
        .nth(material_of(&glb, "dark")?)
        .ok_or("dark")?;
    assert!(
        close(&dark.emissive_factor(), &[0.0; 3]),
        "no glow, no emission"
    );
    Ok(())
}

#[test]
fn submeshes_of_a_material_that_draws_nothing_are_left_out() -> TestResult {
    let caps = matte("caps");
    let proxy = matte("proxy").shader("NoDraw");
    let file = TestMtl::multi(&[matte("unused"), caps, proxy]).cryxml();

    let (glb, summary) = export(Some(file))?;

    assert_eq!(summary.hidden_primitives, 1, "the sides are hidden");
    assert_eq!(summary.primitives, 1, "only the caps remain");
    assert_eq!(
        summary.materials, 1,
        "and the hidden material is not written"
    );
    assert_eq!(summary.triangles, 4, "12 indices are 4 triangles");
    assert_eq!(glb.document.materials().len(), 1);

    let by_flag = TestMtl::multi(&[
        matte("unused"),
        matte("caps"),
        matte("proxy").flags(NO_DRAW),
    ]);
    let (_, summary) = export(Some(by_flag.cryxml()))?;
    assert_eq!(summary.hidden_primitives, 1, "the flag alone hides it");

    let clear = TestMtl::multi(&[matte("unused"), matte("caps"), matte("clear").opacity(0.0)]);
    let (_, summary) = export(Some(clear.cryxml()))?;
    assert_eq!(summary.hidden_primitives, 1, "so does opacity 0");
    Ok(())
}

#[test]
fn a_model_with_only_hidden_geometry_exports_without_meshes() -> TestResult {
    let hidden = matte("proxy").shader("NoDraw");

    let (glb, summary) = export(Some(TestMtl::single(&hidden).cryxml()))?;

    assert_eq!(summary.primitives, 0, "nothing drawn");
    assert_eq!(summary.hidden_primitives, 2);
    assert_eq!(glb.document.meshes().len(), 0, "no mesh without primitives");
    assert_eq!(glb.document.nodes().len(), 2, "the hierarchy stays");
    Ok(())
}

#[test]
fn ids_beyond_the_list_and_a_missing_library_get_one_default_material() -> TestResult {
    let file = TestMtl::multi(&[matte("zero"), matte("caps")]).cryxml();

    let (glb, summary) = export(Some(file))?;

    assert_eq!(
        summary.default_material_ids, 1,
        "the sides' ID 2 is past the list"
    );
    assert_eq!(summary.materials, 2, "caps and the default");
    let default = glb
        .document
        .materials()
        .nth(material_of(&glb, "default")?)
        .ok_or("default")?;
    assert!(
        close(
            &default.pbr_metallic_roughness().base_color_factor(),
            &[0.8, 0.8, 0.8, 1.0]
        ),
        "light grey"
    );

    let (glb, summary) = export(None)?;

    assert_eq!(summary.materials, 1, "one default for every ID");
    assert_eq!(summary.default_material_ids, 2, "both IDs use it");
    assert_eq!(summary.primitives, 2, "the geometry is all there");
    assert_eq!(
        glb.document
            .materials()
            .next()
            .and_then(|m| m.name().map(str::to_owned))
            .as_deref(),
        Some("default")
    );
    Ok(())
}

#[test]
fn unnamed_materials_get_a_name_from_their_position() -> TestResult {
    let nameless = TestMtlMaterial::new("");
    let file = TestMtl::multi(&[matte("zero"), nameless, matte("sides")]).cryxml();

    let (glb, _) = export(Some(file))?;

    material_of(&glb, "material_1")?;
    Ok(())
}
