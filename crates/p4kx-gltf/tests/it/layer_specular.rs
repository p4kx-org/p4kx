// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gltf::{TextureLoadError, TextureRole, layered_base_key, layered_base_texture};
use p4kx_model::{TintEntry, TintPalette};
use p4kx_testkit::{TestMtl, TestMtlMaterial};

use crate::layer_textures::{base_of, export, image, status, uniform, unused};
use crate::scene_support::TestResult;
use crate::textures::support::{Stub, material, near, packed_image};

const NAME: &str = "FixtureLayered";
const DIFFUSE: &str = "objects/test/layer_diff.dds";
const SPECULAR: &str = "objects/test/layer_spec.dds";
const ORIGINAL_SPECULAR: &str = "Objects/Test/layer_spec.tif";
const DIFFUSE_PIXEL: [u8; 4] = [45, 90, 145, 255];
const REFLECTED_PIXEL: [u8; 4] = [180, 255, 180, 255];
const FLAG: &str = "%USE_SPECULAR_MAPS";

fn parent(shader: &str, flags: &str) -> TestMtlMaterial {
    TestMtlMaterial::new(NAME)
        .shader(shader)
        .string_gen_mask(flags)
        .layer_with(&[("Name", "Primary"), ("Path", "layers/primary.mtl")])
}

fn files(shader: &str, slot: &str, path: &str) -> Vec<(&'static str, Vec<u8>)> {
    let mut child = TestMtlMaterial::new("LayerFixture")
        .shader(shader)
        .diffuse([1.0; 3])
        .specular([1.0; 3])
        .texture("TexSlot1", "Objects/Test/layer_diff.tif");
    if !path.is_empty() {
        child = child.texture(slot, path);
    }
    vec![("layers/primary", TestMtl::single(&child).cryxml())]
}

fn cube_materials(parent: TestMtlMaterial) -> [TestMtlMaterial; 3] {
    [unused(), parent, unused().diffuse([0.5; 3])]
}

fn stub() -> Result<Stub, Box<dyn std::error::Error>> {
    let bright = [180, 255, 180, 18];
    let dark = [0, 0, 0, 230];
    Ok(Stub::default()
        .sized()
        .with(DIFFUSE, &uniform(DIFFUSE_PIXEL)?)
        .with(SPECULAR, &image([dark, bright, bright, dark])?))
}

#[test]
fn hard_surface_layer_specular_reaches_glb_colour_metalness_and_preview() -> TestResult {
    let hard = parent("HardSurface", FLAG);
    let loader = stub()?;
    let (glb, _) = export(
        &cube_materials(hard.clone()),
        files("Layer", "TexSlot6", ORIGINAL_SPECULAR),
        &loader,
    )?;
    let base = base_of(&glb, NAME)?;
    let material = material(&glb, NAME)?;
    let pbr = material.pbr_metallic_roughness();
    let packed = packed_image(&glb, &material)?;
    assert_eq!(
        pbr.metallic_factor().to_bits(),
        1.0_f32.to_bits(),
        "the blue channel sets metalness"
    );
    assert_eq!(
        pbr.roughness_factor().to_bits(),
        0.5_f32.to_bits(),
        "specular does not activate a roughness texture"
    );
    assert_eq!(
        packed.pixel(0, 0),
        Some([255, 255, 0, 255]),
        "the dark reflectance is dielectric"
    );
    assert_eq!(
        packed.pixel(1, 0),
        Some([255; 4]),
        "the bright reflectance is metallic"
    );
    assert!(
        near(base.pixel(0, 0), DIFFUSE_PIXEL),
        "a dielectric keeps its diffuse colour"
    );
    assert!(
        near(base.pixel(1, 0), REFLECTED_PIXEL),
        "a metallic texel uses sampled reflectance"
    );

    check_preview(hard, &loader, &base)
}

fn check_preview(
    hard: TestMtlMaterial,
    loader: &Stub,
    base: &p4kx_testkit::DecodedImage,
) -> TestResult {
    let resolved = status(
        std::slice::from_ref(&hard),
        files("Layer", "TexSlot6", ORIGINAL_SPECULAR),
    );
    let plain = status(&[hard], files("Layer", "", ""));
    let material = resolved
        .library()
        .and_then(|library| library.materials.first())
        .ok_or("selected material")?;
    let plain = plain
        .library()
        .and_then(|library| library.materials.first())
        .ok_or("selected plain material")?;
    let preview =
        layered_base_texture(material, loader, 0).ok_or("native preview's layered bake")?;
    assert_ne!(
        layered_base_key(material),
        layered_base_key(plain),
        "specular is in the native cache key"
    );
    let (pixels, trailing_bytes) = preview.image.pixels().as_chunks::<4>();
    assert!(
        trailing_bytes.is_empty(),
        "preview pixels are complete RGBA values"
    );
    for (index, pixel) in pixels.iter().enumerate() {
        let index = u32::try_from(index)?;
        let baked = base
            .pixel(index % 2, index / 2)
            .ok_or("embedded base colour texel")?;
        assert_eq!(
            pixel,
            baked.as_slice(),
            "the preview uses the same sampled base colour"
        );
    }
    Ok(())
}

#[test]
fn an_unproven_shader_flag_slot_or_role_does_not_sample_child_reflectance() -> TestResult {
    for (shader, flags, child, slot, path) in [
        (
            "LayerBlend_V2",
            FLAG,
            "Layer",
            "TexSlot6",
            ORIGINAL_SPECULAR,
        ),
        ("HardSurface", "", "Layer", "TexSlot6", ORIGINAL_SPECULAR),
        (
            "HardSurface",
            "%NO_USE_SPECULAR_MAPS",
            "Layer",
            "TexSlot6",
            ORIGINAL_SPECULAR,
        ),
        ("HardSurface", FLAG, "Illum", "TexSlot6", ORIGINAL_SPECULAR),
        ("HardSurface", FLAG, "Layer", "TexSlot4", ORIGINAL_SPECULAR),
        (
            "HardSurface",
            FLAG,
            "Layer",
            "TexSlot6",
            "Objects/Test/layer_gloss.tif",
        ),
        (
            "HardSurface",
            FLAG,
            "Layer",
            "TexSlot6",
            "Objects/Test/reflectance.tif",
        ),
    ] {
        check_ignored_child_texture(shader, flags, child, slot, path)?;
    }
    Ok(())
}

fn check_ignored_child_texture(
    shader: &str,
    flags: &str,
    child: &str,
    slot: &str,
    path: &str,
) -> TestResult {
    let parent = parent(shader, flags);
    let (control, _) = export(
        &cube_materials(parent.clone()),
        files(child, "", ""),
        &stub()?,
    )?;
    let loader = stub()?
        .with(
            "objects/test/layer_gloss.dds",
            &uniform([37, 81, 121, 255])?,
        )
        .with(
            "objects/test/reflectance.dds",
            &uniform([203, 171, 109, 255])?,
        );
    let (glb, summary) = export(&cube_materials(parent), files(child, slot, path), &loader)?;
    let scope = format!("{shader} {flags} {child} {slot} {path}");
    assert_eq!(
        base_of(&glb, NAME)?.pixel(1, 0),
        base_of(&control, NAME)?.pixel(1, 0),
        "{scope}: the child texture has no base colour effect"
    );
    let pbr = material(&glb, NAME)?.pbr_metallic_roughness();
    assert!(
        pbr.metallic_roughness_texture().is_none(),
        "{scope}: the child texture does not supply metalness"
    );
    let calls = loader.calls();
    assert!(
        matches!(calls.as_slice(), [(path, _)] if path.eq_ignore_ascii_case(DIFFUSE)),
        "{scope}: only the expected diffuse texture is loaded, got {calls:?}"
    );
    assert!(
        summary.texture_fallbacks.is_empty(),
        "{scope}: ignored child textures produce no fallback receipts"
    );
    Ok(())
}

#[test]
fn missing_child_specular_reports_the_original_declaration_and_keeps_the_flat_finish() -> TestResult
{
    let parent = parent("HardSurface", FLAG);
    let loader = stub()?.failing(SPECULAR, TextureLoadError::NotFound);
    let (control, _) = export(
        &cube_materials(parent.clone()),
        files("Layer", "", ""),
        &loader,
    )?;
    let (glb, summary) = export(
        &cube_materials(parent),
        files("Layer", "TexSlot6", ORIGINAL_SPECULAR),
        &loader,
    )?;
    assert_eq!(
        base_of(&glb, NAME)?.pixel(1, 0),
        base_of(&control, NAME)?.pixel(1, 0),
        "a failed map keeps the constant child finish"
    );
    assert!(
        material(&glb, NAME)?
            .pbr_metallic_roughness()
            .metallic_roughness_texture()
            .is_none(),
        "a failed specular map provides no packed channel"
    );
    let files: Vec<_> = summary
        .texture_fallbacks
        .iter()
        .filter(|failure| failure.role == TextureRole::Specular)
        .map(|failure| failure.file.as_str())
        .collect();
    assert_eq!(
        files,
        [ORIGINAL_SPECULAR],
        "one original specular declaration is reported"
    );
    Ok(())
}

#[test]
fn a_selected_palette_keeps_its_authored_base_colour_projection() -> TestResult {
    let hard = TestMtlMaterial::new(NAME)
        .shader("HardSurface")
        .string_gen_mask(FLAG)
        .layer_with(&[
            ("Name", "Primary"),
            ("Path", "layers/primary.mtl"),
            ("PaletteTint", "1"),
        ]);
    let entry = TintEntry {
        tint: [0.2, 0.3, 0.4],
        specular: [0.05; 3],
        glossiness: 102.0,
    };
    let palette = TintPalette::new("fixture", [entry; 3], [1.0; 3]);
    let selected = |path| {
        let resolved = status(
            std::slice::from_ref(&hard),
            files("Layer", "TexSlot6", path),
        );
        let authored = resolved.library()?.materials.first()?;
        palette.apply(authored)
    };
    let with_map = selected(ORIGINAL_SPECULAR).ok_or("painted layer with authored map")?;
    let without_map = selected("").ok_or("painted layer without authored map")?;
    let loader = stub()?;
    let render = |material| {
        layered_base_texture(material, &loader, 0)
            .map(|loaded| loaded.image)
            .ok_or("the painted base colour image")
    };
    assert_eq!(
        render(&with_map)?,
        render(&without_map)?,
        "the palette's existing base projection ignores the separate specular lobe"
    );
    assert!(
        loader
            .calls()
            .iter()
            .all(|(path, _)| !path.eq_ignore_ascii_case(SPECULAR)),
        "the painted layer does not sample this specular map into base colour"
    );
    Ok(())
}

const LAYER_DECODE_PIXELS: u32 = 1 << 27;

#[test]
fn optional_specular_budget_preserves_usable_diffuse_in_glb_and_preview() -> TestResult {
    for (loader, reason) in [
        (
            stub()?.sized_as(SPECULAR, (16_384, 16_384)),
            "exceed the texture decode budget",
        ),
        (stub()?.unknown_size(SPECULAR), "no verified dimensions"),
        (
            stub()?.sized_as(DIFFUSE, (LAYER_DECODE_PIXELS - 1, 1)),
            "exceed the texture decode budget",
        ),
    ] {
        check_diffuse_budget_fallback(&loader, reason)?;
    }
    Ok(())
}

fn check_diffuse_budget_fallback(loader: &Stub, reason: &str) -> TestResult {
    let hard = parent("HardSurface", FLAG);
    let selected_files = || files("Layer", "TexSlot6", ORIGINAL_SPECULAR);
    let (control, _) = export(
        &cube_materials(hard.clone()),
        files("Layer", "", ""),
        loader,
    )?;
    let (glb, summary) = export(&cube_materials(hard.clone()), selected_files(), loader)?;
    let base = base_of(&glb, NAME)?;
    let control = base_of(&control, NAME)?;
    assert_eq!(
        base.pixel(1, 0),
        control.pixel(1, 0),
        "budgeted specular preserves decoded diffuse"
    );
    assert!(
        material(&glb, NAME)?
            .pbr_metallic_roughness()
            .metallic_roughness_texture()
            .is_none(),
        "the omitted source does not enable per-pixel metallic"
    );
    let [failure] = summary.texture_fallbacks.as_slice() else {
        return Err("one optional source is omitted".into());
    };
    assert_eq!(
        failure.role,
        TextureRole::Specular,
        "the diffuse job survives"
    );
    assert_eq!(
        failure.file, ORIGINAL_SPECULAR,
        "the physical declaration is retained"
    );
    assert!(
        failure.reason.contains(reason),
        "distinct diagnostic for {reason}"
    );
    check_preview(hard, loader, &base)?;
    assert!(
        loader
            .calls()
            .iter()
            .all(|(path, _)| !path.eq_ignore_ascii_case(SPECULAR)),
        "a demoted source is not decoded"
    );
    Ok(())
}
