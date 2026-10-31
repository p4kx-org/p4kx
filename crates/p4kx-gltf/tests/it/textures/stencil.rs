// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Stencil decals: the red channel of the stencil map is the opacity of the motif, its colour comes
//! from the stencil parameters, and a decal without a motif is left out.

use p4kx_gltf::{GlbOptions, MaterialMode};
use p4kx_testkit::TestMtlMaterial;
use p4kx_testkit::gltf::material::AlphaMode;

use super::support::{
    Stub, TestResult, base_image, export, material, near, plain, texture, textured,
};

const STENCIL_FILE: &str = "Objects/Test/logo_stencil.tif";

fn decal(name: &str) -> TestMtlMaterial {
    decal_with(name, "0.5")
}

fn decal_with(name: &str, stencil_opacity: &str) -> TestMtlMaterial {
    TestMtlMaterial::new(name)
        .shader("MeshDecal")
        .string_gen_mask("%VERTDATA%STENCIL_MAP")
        .diffuse([1.0; 3])
        .opacity(0.8)
        .public_param("StencilDiffuseColor", "0.1,0.2,0.3")
        .public_param("StencilOpacity", stencil_opacity)
}

fn stub() -> Result<Stub, Box<dyn std::error::Error>> {
    Ok(Stub::default().with(
        "objects/test/logo_stencil.dds",
        &texture([64, 200, 10, 255], false)?,
    ))
}

#[test]
fn a_stencil_decal_is_cut_out_by_the_red_of_its_stencil_map_in_the_stencil_colour() -> TestResult {
    let logo = decal("logo").texture("TexSlot7", STENCIL_FILE);

    let (glb, summary) = export(
        &[plain("unused"), plain("caps"), logo],
        &stub()?,
        &textured(),
    )?;

    let logo = material(&glb, "logo")?;
    assert_eq!(logo.alpha_mode(), AlphaMode::Blend);
    let base = base_image(&glb, &logo)?;
    assert!(
        near(base.pixel(0, 0), [255, 255, 255, 64]),
        "white with the red channel as alpha: {:?}",
        base.pixel(0, 0)
    );
    let [red, green, blue, alpha] = logo.pbr_metallic_roughness().base_color_factor();
    for (found, wanted) in [(red, 0.1), (green, 0.2), (blue, 0.3), (alpha, 0.4)] {
        assert!((found - wanted).abs() < 1e-6, "{found} vs {wanted}");
    }
    assert_eq!(summary.hidden_primitives, 0);
    Ok(())
}

#[test]
fn the_green_and_blue_of_the_stencil_map_do_not_select_the_second_and_third_colour() -> TestResult {
    let logo = decal("logo")
        .texture("TexSlot7", STENCIL_FILE)
        .public_param("StencilDiffuseColor2", "0.9,0.0,0.0")
        .public_param("StencilDiffuseColor3", "0.0,0.9,0.0");

    let (glb, _) = export(
        &[plain("unused"), plain("caps"), logo],
        &stub()?,
        &textured(),
    )?;

    let logo = material(&glb, "logo")?;
    let base = base_image(&glb, &logo)?;
    assert!(
        near(base.pixel(0, 0), [255, 255, 255, 64]),
        "a map with live green and blue channels is cut out by its red one: {:?}",
        base.pixel(0, 0)
    );
    let [red, green, blue, _] = logo.pbr_metallic_roughness().base_color_factor();
    for (found, wanted) in [(red, 0.1), (green, 0.2), (blue, 0.3)] {
        assert!((found - wanted).abs() < 1e-6, "{found} vs {wanted}");
    }
    Ok(())
}

#[test]
fn a_stencil_decal_whose_motif_the_game_fills_in_is_left_out() -> TestResult {
    let emblem = decal("emblem").texture("TexSlot7", "$TintPaletteDecal");

    let (_, summary) = export(
        &[plain("unused"), plain("caps"), emblem],
        &Stub::default(),
        &textured(),
    )?;

    assert_eq!(summary.hidden_primitives, 1);
    assert_eq!(summary.primitives, 1);
    Ok(())
}

#[test]
fn a_stencil_decal_with_a_diffuse_map_is_drawn_by_it() -> TestResult {
    let both = decal("both")
        .texture("TexSlot1", "Objects/Test/both_diff.tif")
        .texture("TexSlot7", STENCIL_FILE);
    let stub = stub()?.with(
        "objects/test/both_diff.dds",
        &texture([200, 100, 50, 255], false)?,
    );

    let (glb, summary) = export(&[plain("unused"), plain("caps"), both], &stub, &textured())?;

    let both = material(&glb, "both")?;
    let base = base_image(&glb, &both)?;
    assert!(
        near(base.pixel(0, 0), [200, 100, 50, 255]),
        "{:?}",
        base.pixel(0, 0)
    );
    assert_eq!(summary.hidden_primitives, 0);
    let [red, green, blue, alpha] = both.pbr_metallic_roughness().base_color_factor();
    assert_eq!(
        [red, green, blue],
        [1.0; 3],
        "the stencil colour tints the motif only"
    );
    assert!(
        (alpha - 0.8).abs() < 1e-6,
        "nor does the stencil opacity: {alpha}"
    );
    Ok(())
}

#[test]
fn a_diffuse_map_that_cannot_be_drawn_leaves_a_stencil_decal_fully_transparent() -> TestResult {
    for (name, file) in [
        ("missing", "Objects/Test/lost_diff.tif"),
        ("unsafe", "../Objects/Test/both_diff.tif"),
    ] {
        let broken = decal(name)
            .texture("TexSlot1", file)
            .texture("TexSlot7", STENCIL_FILE);

        let (glb, _) = export(
            &[plain("unused"), plain("caps"), broken],
            &stub()?,
            &textured(),
        )?;

        let decal = material(&glb, name)?;
        assert!(
            decal
                .pbr_metallic_roughness()
                .base_color_texture()
                .is_none(),
            "{name}"
        );
        assert!(
            decal.pbr_metallic_roughness().base_color_factor()[3].abs() < f32::EPSILON,
            "{name} is no solid quad"
        );
    }
    Ok(())
}

#[test]
fn a_stencil_map_that_cannot_be_drawn_leaves_a_fully_transparent_decal() -> TestResult {
    for (name, file) in [
        ("missing", "Objects/Test/lost_stencil.tif"),
        ("unsafe", "../Objects/Test/logo_stencil.tif"),
    ] {
        let broken = decal(name).texture("TexSlot7", file);
        let materials = [plain("unused"), plain("caps"), broken];

        let (glb, summary) = export(&materials, &stub()?, &textured())?;

        let decal = material(&glb, name)?;
        assert_eq!(decal.alpha_mode(), AlphaMode::Blend, "{name}");
        assert!(
            decal
                .pbr_metallic_roughness()
                .base_color_texture()
                .is_none(),
            "{name}"
        );
        assert!(
            decal.pbr_metallic_roughness().base_color_factor()[3].abs() < f32::EPSILON,
            "{name} is no solid quad"
        );
        assert!(!summary.texture_fallbacks.is_empty(), "{name}: {summary:?}");
    }
    Ok(())
}

#[test]
fn colours_mode_leaves_a_stencil_decal_fully_transparent() -> TestResult {
    let logo = decal("logo").texture("TexSlot7", STENCIL_FILE);
    let options = GlbOptions::default().with_materials(MaterialMode::Colours);

    let (glb, _) = export(&[plain("unused"), plain("caps"), logo], &stub()?, &options)?;

    let logo = material(&glb, "logo")?;
    assert!(logo.pbr_metallic_roughness().base_color_factor()[3].abs() < f32::EPSILON);
    Ok(())
}

#[test]
fn a_slot_one_file_that_is_no_diffuse_map_leaves_the_stencil_map_in_charge() -> TestResult {
    let logo = decal("logo")
        .texture("TexSlot1", "Objects/Test/logo_id.tif")
        .texture("TexSlot7", STENCIL_FILE);

    let (glb, _) = export(
        &[plain("unused"), plain("caps"), logo],
        &stub()?,
        &textured(),
    )?;

    let base = base_image(&glb, &material(&glb, "logo")?)?;
    assert!(
        near(base.pixel(0, 0), [255, 255, 255, 64]),
        "{:?}",
        base.pixel(0, 0)
    );
    Ok(())
}

#[test]
fn a_stencil_opacity_of_zero_makes_the_motif_invisible() -> TestResult {
    let logo = decal_with("logo", "0").texture("TexSlot7", STENCIL_FILE);

    let (glb, _) = export(
        &[plain("unused"), plain("caps"), logo],
        &stub()?,
        &textured(),
    )?;

    let logo = material(&glb, "logo")?;
    assert_eq!(logo.alpha_mode(), AlphaMode::Blend);
    assert!(logo.pbr_metallic_roughness().base_color_factor()[3].abs() < f32::EPSILON);
    Ok(())
}
