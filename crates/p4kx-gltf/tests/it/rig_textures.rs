// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Pieces worn on a shared skeleton, written with the textures of their materials.

use std::collections::BTreeMap;
use std::error::Error;

use p4kx_gltf::{
    GlbOptions, MaterialMode, RigPiece, RiggedGlbEmission, RiggedJsonAddress, TextureReadId,
    write_rigged_glb_animated_observed,
};
use p4kx_model::{MaterialStatus, PaletteSlot, TintEntry, TintPalette};
use p4kx_testkit::gltf::Semantic;
use p4kx_testkit::gltf::material::AlphaMode;
use p4kx_testkit::{TestMtlMaterial, check_glb};

use crate::textures::support::{
    Stub, TestResult, add_uvless_mesh, base_image, material, near, texture,
};

mod support;

use support::{
    FEMALE_SCALP, MALE_LIBRARY, MALE_SCALP, body, hair_cap, library, library_at, piece,
    skin_material, write,
};

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

#[test]
fn a_rigged_slot_attributes_texture_reads_only_to_the_uv_material_index() -> TestResult {
    let diffuse = TextureReadId::new(0);
    let normal = TextureReadId::new(1);
    let stub = Stub::default()
        .with(
            "objects/test/head_diff.dds",
            &texture([200, 100, 50, 255], false)?.with_read(diffuse),
        )
        .with(
            "objects/test/head_ddna.dds",
            &texture([128, 128, 255, 200], true)?.with_read(normal),
        );
    let body = body()?;
    let mut piece = piece()?;
    add_uvless_mesh(&mut piece)?;
    let material = library(&skin_material())?;
    let pieces = [RigPiece::new("head", &piece, &material)];
    let options = GlbOptions::default().with_materials(MaterialMode::Textures);
    let mut bytes = Vec::new();
    let (_, emission) =
        write_rigged_glb_animated_observed(&body.bones, &pieces, &[], &options, &stub, &mut bytes)?;
    let glb = check_glb(&bytes)?;
    let mut bindings = BTreeMap::new();

    for primitive in glb.document.meshes().flat_map(|mesh| mesh.primitives()) {
        let index = primitive.material().index().ok_or("material index")?;
        let has_uvs = primitive.get(&Semantic::TexCoords(0)).is_some();
        let slot = source_material_slot(&emission, index)?;
        let material = emission.material(index).ok_or("material emission")?;
        let reads: Vec<_> = material
            .texture_bindings()
            .flat_map(|(_, index)| emission.image_sources().nth(index).into_iter().flatten())
            .filter_map(|source| source.read)
            .collect();

        if has_uvs {
            assert!(reads.contains(&diffuse));
            assert!(reads.contains(&normal));
        } else {
            assert_eq!(
                reads,
                [] as [TextureReadId; 0],
                "a UV-less material has no texture reads"
            );
        }
        let (plain, textured) = bindings.entry(slot).or_insert((None, None));
        if has_uvs {
            *textured = Some(index);
        } else {
            *plain = Some(index);
        }
    }
    assert!(!bindings.is_empty());
    for (plain, textured) in bindings.values() {
        assert!(plain.is_some() && textured.is_some());
        assert_ne!(plain, textured);
    }
    Ok(())
}

fn source_material_slot(emission: &RiggedGlbEmission, index: usize) -> Result<usize, &'static str> {
    emission
        .json_addresses()
        .iter()
        .find_map(|address| match *address {
            RiggedJsonAddress::Material {
                index: emitted_index,
                part: 0,
                slot: Some(slot),
                ..
            } if emitted_index == index => Some(slot),
            _ => None,
        })
        .ok_or("source material slot")
}
