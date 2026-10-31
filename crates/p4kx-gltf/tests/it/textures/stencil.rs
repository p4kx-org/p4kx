// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Stencil decals: the red channel of the stencil map is the opacity of the motif, its colour comes
//! from the stencil parameters, and a decal without a motif is left out.

use p4kx_testkit::TestMtlMaterial;
use p4kx_testkit::gltf::material::AlphaMode;

use super::support::{
    Stub, TestResult, base_image, export, material, near, plain, texture, textured,
};

const STENCIL_FILE: &str = "Objects/Test/logo_stencil.tif";

fn decal(name: &str) -> TestMtlMaterial {
    TestMtlMaterial::new(name)
        .shader("MeshDecal")
        .string_gen_mask("%VERTDATA%STENCIL_MAP")
        .diffuse([1.0; 3])
        .opacity(0.8)
        .public_param("StencilDiffuseColor", "0.1,0.2,0.3")
        .public_param("StencilOpacity", "0.5")
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
    Ok(())
}
