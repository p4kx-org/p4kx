// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Pieces worn on a shared skeleton, written with the textures of their materials.

use std::error::Error;

use p4kx_chunks::ChunkFile;
use p4kx_gltf::{
    GlbOptions, GlbSummary, MaterialMode, RigPiece, write_rigged_glb,
    write_rigged_glb_animated_observed, write_rigged_glb_with,
};
use p4kx_model::{
    CompanionError, CompanionPath, MaterialLibrary, MaterialStatus, Model, PaletteSlot, TintEntry,
    TintPalette,
};
use p4kx_testkit::gltf::material::AlphaMode;
use p4kx_testkit::{
    CheckedGlb, TestBone, TestModel, TestMtl, TestMtlMaterial, TestSkeleton, check_glb,
};

use crate::textures::support::{Stub, TestResult, base_image, material, near, texture};

fn bone(name: &str, parent: Option<u16>, z: f32) -> TestBone {
    TestBone {
        name: name.to_owned(),
        parent,
        rotation: [0.0, 0.0, 0.0, 1.0],
        translation: [0.0, 0.0, z],
    }
}

fn body() -> Result<Model, Box<dyn Error>> {
    let skeleton = TestSkeleton::new(vec![
        bone("pelvis", None, 0.0),
        bone("TIP", Some(0), 1.0),
        bone("root", Some(1), 1.0),
    ]);
    let source = TestModel::cube().helpers_only().with_skeleton(&skeleton);
    let mut none = |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> { Ok(None) };
    Ok(Model::read(
        "body.chr",
        &ChunkFile::parse(source.cgf())?,
        &mut none,
    )?)
}

fn piece() -> Result<Model, Box<dyn Error>> {
    let source = TestModel::two_bone_skin();
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("piece.skinm") => Some(source.cgfm()),
            _ => None,
        })
    };
    Ok(Model::read(
        "piece.skin",
        &ChunkFile::parse(source.cgf())?,
        &mut companions,
    )?)
}

fn library(material: &TestMtlMaterial) -> Result<MaterialStatus, Box<dyn Error>> {
    library_at("objects/test/piece", material)
}

fn library_at(path: &str, material: &TestMtlMaterial) -> Result<MaterialStatus, Box<dyn Error>> {
    let materials = [material.clone(), material.clone(), material.clone()];
    let bytes = TestMtl::multi(&materials).cryxml();
    Ok(MaterialStatus::Loaded(MaterialLibrary::parse(
        path, &bytes,
    )?))
}

fn hair_cap(file: &str) -> TestMtlMaterial {
    TestMtlMaterial::new("hair_scalp_m")
        .shader("HairPBR")
        .string_gen_mask("%HAIR_CAP")
        .diffuse([0.0, 0.0, 0.0])
        .texture("TexSlot1", file)
}

const MALE_LIBRARY: &str = "objects/test/hair_v2/sc/male/m_hair_31";
const MALE_SCALP: &str = "Objects/Test/hair_v2/sc/male/textures/m_hair_31_scalp.tif";
const FEMALE_SCALP: &str = "Objects/Test/hair_v2/sc/female/textures/f_hair_31_scalp.tif";

fn write(
    status: &MaterialStatus,
    stub: Option<&Stub>,
) -> Result<(CheckedGlb, GlbSummary), Box<dyn Error>> {
    let (body, piece) = (body()?, piece()?);
    let pieces = [RigPiece::new("head", &piece, status)];
    let options = GlbOptions::default().with_materials(MaterialMode::Textures);
    let mut bytes = Vec::new();
    let summary = match stub {
        Some(stub) => write_rigged_glb_with(&body.bones, &pieces, &options, stub, &mut bytes)?,
        None => write_rigged_glb(&body.bones, &pieces, &options, &mut bytes)?,
    };
    Ok((check_glb(&bytes)?, summary))
}

fn skin_material() -> TestMtlMaterial {
    TestMtlMaterial::new("shader_head")
        .shader("HumanSkin_V2")
        .diffuse([0.5, 1.0, 0.25])
        .texture("TexSlot1", "Objects/Test/head_diff.tif")
        .texture("TexSlot2", "Objects/Test/head_ddna.tif")
}

#[test]
fn the_textures_of_a_rigged_piece_are_embedded_and_shared() -> TestResult {
    let stub = Stub::default()
        .with(
            "objects/test/head_diff.dds",
            &texture([200, 100, 50, 255], false)?,
        )
        .with(
            "objects/test/head_ddna.dds",
            &texture([128, 128, 255, 200], true)?,
        );

    let (glb, summary) = write(&library(&skin_material())?, Some(&stub))?;

    assert_eq!(glb.document.skins().len(), 1, "still one skin");
    assert!(summary.textured_materials >= 1, "{summary:?}");
    assert_eq!(
        summary.textures, 3,
        "the diffuse, normal and smoothness image, each made once however many materials use them"
    );
    let head = material(&glb, "shader_head")?;
    assert!(head.normal_texture().is_some(), "a normal map");
    let base = base_image(&glb, &head)?;
    assert!(
        near(base.pixel(0, 0), [200, 100, 50, 255]),
        "{:?}",
        base.pixel(0, 0)
    );
    assert_eq!(
        head.pbr_metallic_roughness().base_color_factor(),
        [0.5, 1.0, 0.25, 1.0],
        "the material's diffuse colour multiplies the image"
    );
    Ok(())
}

#[test]
fn hair_cards_take_their_opacity_from_the_opacity_map_and_their_colour_from_the_material()
-> TestResult {
    let cards = TestMtlMaterial::new("hair_m")
        .shader("HairPBR")
        .diffuse([0.2, 0.1, 0.05])
        .texture("TexSlot1", "Objects/Test/hair_opac.tif")
        .texture("TexSlot4", "Objects/Test/hair_id.tif");
    let stub = Stub::default().with(
        "objects/test/hair_opac.dds",
        &texture([64, 0, 0, 255], false)?,
    );

    let (glb, summary) = write(&library(&cards)?, Some(&stub))?;

    let hair = material(&glb, "hair_m")?;
    assert_eq!(hair.alpha_mode(), AlphaMode::Mask, "cards are cut out");
    assert_eq!(hair.alpha_cutoff(), Some(0.5));
    let base = base_image(&glb, &hair)?;
    assert!(
        near(base.pixel(0, 0), [255, 255, 255, 64]),
        "white with the opacity as alpha: {:?}",
        base.pixel(0, 0)
    );
    assert_eq!(
        hair.pbr_metallic_roughness().base_color_factor(),
        [0.2, 0.1, 0.05, 1.0]
    );
    assert!(summary.texture_fallbacks.is_empty(), "{summary:?}");
    Ok(())
}

#[test]
fn a_hair_cap_is_blended_by_its_density_map() -> TestResult {
    let stub = Stub::default().with(
        "objects/test/hair_v2/sc/male/textures/m_hair_31_scalp.dds",
        &texture([100, 100, 100, 255], false)?,
    );

    let (glb, summary) = write(
        &library_at(MALE_LIBRARY, &hair_cap(MALE_SCALP))?,
        Some(&stub),
    )?;

    let cap = material(&glb, "hair_scalp_m")?;
    assert_eq!(cap.alpha_mode(), AlphaMode::Blend, "never an opaque shell");
    let base = base_image(&glb, &cap)?;
    assert!(
        near(base.pixel(0, 0), [255, 255, 255, 100]),
        "the density is the alpha: {:?}",
        base.pixel(0, 0)
    );
    assert_eq!(cap.pbr_metallic_roughness().base_color_factor()[3], 1.0);
    assert!(summary.texture_fallbacks.is_empty(), "{summary:?}");
    Ok(())
}

#[test]
fn a_hair_caps_map_of_the_other_body_is_swapped_for_the_bodys_own() -> TestResult {
    let stub = Stub::default().with(
        "objects/test/hair_v2/sc/male/textures/m_hair_31_scalp.dds",
        &texture([80, 80, 80, 255], false)?,
    );

    let (glb, summary) = write(
        &library_at(MALE_LIBRARY, &hair_cap(FEMALE_SCALP))?,
        Some(&stub),
    )?;

    let base = base_image(&glb, &material(&glb, "hair_scalp_m")?)?;
    assert!(
        near(base.pixel(0, 0), [255, 255, 255, 80]),
        "{:?}",
        base.pixel(0, 0)
    );
    assert!(summary.texture_fallbacks.is_empty(), "{summary:?}");
    Ok(())
}

#[test]
fn a_hair_cap_falls_back_to_the_map_its_file_names_when_the_bodys_is_missing() -> TestResult {
    let stub = Stub::default().with(
        "objects/test/hair_v2/sc/female/textures/f_hair_31_scalp.dds",
        &texture([60, 60, 60, 255], false)?,
    );

    let (glb, summary) = write(
        &library_at(MALE_LIBRARY, &hair_cap(FEMALE_SCALP))?,
        Some(&stub),
    )?;

    let base = base_image(&glb, &material(&glb, "hair_scalp_m")?)?;
    assert!(
        near(base.pixel(0, 0), [255, 255, 255, 60]),
        "{:?}",
        base.pixel(0, 0)
    );
    assert!(summary.texture_fallbacks.is_empty(), "{summary:?}");
    Ok(())
}

#[test]
fn a_hair_cap_without_its_map_is_transparent_and_listed() -> TestResult {
    let stub = Stub::default();

    let (glb, summary) = write(
        &library_at(MALE_LIBRARY, &hair_cap(FEMALE_SCALP))?,
        Some(&stub),
    )?;

    let cap = material(&glb, "hair_scalp_m")?;
    assert_eq!(cap.alpha_mode(), AlphaMode::Blend);
    assert_eq!(cap.pbr_metallic_roughness().base_color_factor()[3], 0.0);
    assert!(
        summary
            .texture_fallbacks
            .iter()
            .any(|fallback| fallback.file.contains("m_hair_31_scalp")),
        "{summary:?}"
    );
    Ok(())
}

#[test]
fn a_texture_that_cannot_be_loaded_leaves_the_plain_colour_and_is_listed() -> TestResult {
    let stub = Stub::default();

    let (glb, summary) = write(&library(&skin_material())?, Some(&stub))?;

    let head = material(&glb, "shader_head")?;
    assert!(head.pbr_metallic_roughness().base_color_texture().is_none());
    assert_eq!(
        head.pbr_metallic_roughness().base_color_factor(),
        [0.5, 1.0, 0.25, 1.0]
    );
    assert!(
        summary
            .texture_fallbacks
            .iter()
            .any(|fallback| fallback.file.contains("head_diff")),
        "{summary:?}"
    );
    Ok(())
}

#[test]
fn the_plain_writer_ignores_the_material_mode_and_embeds_no_texture() -> TestResult {
    let (glb, summary) = write(&library(&skin_material())?, None)?;

    assert_eq!(summary.textures, 0);
    assert_eq!(glb.document.textures().len(), 0);
    Ok(())
}

fn painted_library() -> Result<MaterialStatus, Box<dyn Error>> {
    let painted = skin_material().texture("TexSlot4", "Objects/Test/head_spec.tif");
    let MaterialStatus::Loaded(mut library) = library(&painted)? else {
        return Err("expected a material library".into());
    };
    let red = TintEntry {
        tint: [0.8, 0.1, 0.2],
        specular: [0.6; 3],
        ..TintEntry::default()
    };
    let palette = TintPalette::new("paint", [red; 3], [1.0; 3]);
    for material in &mut library.materials {
        material.palette = Some(PaletteSlot::Primary);
        *material = palette.apply(material).ok_or("applied palette")?;
    }
    Ok(MaterialStatus::Loaded(library))
}

#[test]
fn a_shared_image_tracks_the_original_job_and_only_the_palette_values_it_uses() -> TestResult {
    let status = painted_library()?;
    let stub = Stub::default()
        .with(
            "objects/test/head_diff.dds",
            &texture([200, 100, 50, 255], false)?,
        )
        .with(
            "objects/test/head_ddna.dds",
            &texture([128, 128, 255, 200], true)?,
        )
        .with(
            "objects/test/head_spec.dds",
            &texture([230, 230, 230, 255], false)?,
        );
    let (body, piece) = (body()?, piece()?);
    let pieces = [
        RigPiece::new("first", &piece, &status),
        RigPiece::new("shared", &piece, &status),
    ];
    let options = GlbOptions::default().with_materials(MaterialMode::Textures);
    let mut bytes = Vec::new();
    let (summary, emission) =
        write_rigged_glb_animated_observed(&body.bones, &pieces, &[], &options, &stub, &mut bytes)?;
    let glb = check_glb(&bytes)?;
    let head = material(&glb, "shader_head")?;
    let pbr = head.pbr_metallic_roughness();
    let base = pbr
        .base_color_texture()
        .ok_or("base colour texture")?
        .texture();
    let normal = head.normal_texture().ok_or("normal texture")?.texture();
    let packed = pbr
        .metallic_roughness_texture()
        .ok_or("packed texture")?
        .texture();
    for (texture, uses_palette) in [(base, true), (normal, false), (packed, true)] {
        let source = emission
            .image_material(texture.source().index())
            .ok_or("the original material job")?;
        assert_eq!(
            source.part, 0,
            "the first registered part owns the deduplicated job"
        );
        assert!(
            source.slot.is_some(),
            "the source is an authored library slot"
        );
        assert_eq!(
            source.uses_palette, uses_palette,
            "only colour conversion uses the palette"
        );
    }
    assert_eq!(
        summary.textures, 3,
        "the second part reuses all three images"
    );
    Ok(())
}
