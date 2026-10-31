// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The palette colour a material takes and a tint palette applied to it.

use std::error::Error;

use p4kx_model::{MaterialLibrary, PaletteSlot, TintEntry, TintPalette, srgb8_to_linear};
use p4kx_testkit::{TestMtl, TestMtlMaterial};

type TestResult = Result<(), Box<dyn Error>>;

fn library(materials: &[TestMtlMaterial]) -> Result<MaterialLibrary, Box<dyn Error>> {
    Ok(MaterialLibrary::parse(
        "objects/test/ship",
        &TestMtl::multi(materials).cryxml(),
    )?)
}

fn palette() -> TintPalette {
    let entry = |tint: f32, glossiness: f32| TintEntry {
        tint: [tint; 3],
        specular: [tint / 2.0; 3],
        glossiness,
    };
    TintPalette::new(
        "test",
        [entry(0.1, 10.0), entry(0.2, 20.0), entry(0.3, 30.0)],
        [0.4, 0.5, 0.6],
    )
}

#[test]
fn a_material_takes_the_palette_entry_of_its_first_layer_with_a_palette_tint() -> TestResult {
    let library = library(&[
        TestMtlMaterial::new("unlayered"),
        TestMtlMaterial::new("primary").layer("Primary", "1"),
        TestMtlMaterial::new("wear_first")
            .layer("Wear", "0")
            .layer("Base", "3")
            .layer("Other", "1"),
        TestMtlMaterial::new("out_of_range").layer("Primary", "4"),
        TestMtlMaterial::new("garbled").layer("Primary", "red"),
        TestMtlMaterial::new("secondary").layer("Primary", " 2 "),
    ])?;

    let slots: Vec<Option<PaletteSlot>> = library.materials.iter().map(|m| m.palette).collect();

    assert_eq!(
        slots,
        [
            None,
            Some(PaletteSlot::Primary),
            Some(PaletteSlot::Tertiary),
            None,
            None,
            Some(PaletteSlot::Secondary),
        ]
    );
    Ok(())
}

#[test]
fn only_a_glass_shader_that_uses_the_tint_palette_takes_the_glass_colour() -> TestResult {
    let library = library(&[
        TestMtlMaterial::new("tinted_glass")
            .shader("GlassPBR")
            .string_gen_mask("%NORMAL_MAP%USE_TINT_PALETTE"),
        TestMtlMaterial::new("plain_glass")
            .shader("GlassPBR")
            .string_gen_mask("%NORMAL_MAP"),
        TestMtlMaterial::new("tinted_other")
            .shader("Illum")
            .string_gen_mask("%USE_TINT_PALETTE"),
    ])?;

    let slots: Vec<Option<PaletteSlot>> = library.materials.iter().map(|m| m.palette).collect();

    assert_eq!(slots, [Some(PaletteSlot::Glass), None, None]);
    Ok(())
}

#[test]
fn applying_a_palette_replaces_the_colours_of_a_painted_material() -> TestResult {
    let library = library(&[TestMtlMaterial::new("paint")
        .layer("Primary", "2")
        .diffuse([1.0; 3])
        .specular([0.9; 3])
        .shininess(200.0)])?;
    let material = library.material(0).ok_or("a material")?;

    let tinted = palette().apply(material).ok_or("a tinted material")?;

    assert_eq!(tinted.diffuse, [0.2; 3]);
    assert_eq!(tinted.specular, [0.1; 3]);
    assert!((tinted.shininess - 20.0).abs() < f32::EPSILON);
    assert!(tinted.tinted);
    assert_eq!(tinted.name, "paint", "the rest is kept");
    assert!(!material.tinted, "the library is unchanged");
    Ok(())
}

#[test]
fn applying_a_palette_gives_glass_the_glass_colour_and_leaves_other_materials_alone() -> TestResult
{
    let library = library(&[
        TestMtlMaterial::new("glass")
            .shader("GlassPBR")
            .string_gen_mask("%USE_TINT_PALETTE")
            .specular([0.7; 3]),
        TestMtlMaterial::new("plain"),
    ])?;
    let glass = library.material(0).ok_or("glass")?;
    let plain = library.material(1).ok_or("plain")?;

    let tinted = palette().apply(glass).ok_or("a tinted material")?;

    assert_eq!(tinted.diffuse, [0.4, 0.5, 0.6]);
    assert_eq!(tinted.specular, glass.specular, "glass keeps its specular");
    assert_eq!(palette().apply(plain), None);
    Ok(())
}

fn stencil_decal(over: &str) -> TestMtlMaterial {
    TestMtlMaterial::new("logo")
        .shader("MeshDecal")
        .string_gen_mask("%STENCIL_MAP")
        .texture("TexSlot7", "objects/test/logo_stencil.tif")
        .public_param("StencilTintOverride", over)
}

#[test]
fn a_stencil_decal_that_takes_the_decal_colours_gets_them_and_a_white_diffuse() -> TestResult {
    let decals = [[0.1, 0.0, 0.0], [0.0, 0.2, 0.0], [0.0, 0.0, 0.3]];
    let library = library(&[stencil_decal("2"), stencil_decal("1")])?;
    let decal = library.material(0).ok_or("the decal")?;
    let stand_in = library.material(1).ok_or("the primary decal")?;
    assert_eq!(decal.palette, Some(PaletteSlot::Decal));

    let tinted = palette()
        .with_decal(decals)
        .apply(decal)
        .ok_or("a tinted decal")?;

    assert_eq!(tinted.decal_colours, Some(decals));
    assert_eq!(tinted.diffuse, [1.0; 3]);
    assert!(tinted.tinted);
    assert_eq!(
        palette().apply(decal),
        None,
        "a palette without decal colours"
    );
    let primary = palette()
        .with_decal(decals)
        .apply(stand_in)
        .ok_or("primary")?;
    assert_eq!(
        primary.diffuse, [0.1; 3],
        "an override of 1 takes the primary entry"
    );
    assert_eq!(primary.decal_colours, None);
    Ok(())
}

#[test]
fn srgb_bytes_convert_to_linear_values() {
    assert!(srgb8_to_linear(0).abs() < f32::EPSILON);
    assert!((srgb8_to_linear(255) - 1.0).abs() < 1e-6);
    assert!((srgb8_to_linear(128) - 0.215_86).abs() < 1e-4);
    assert!((srgb8_to_linear(10) - 0.003_035).abs() < 1e-5);
}
