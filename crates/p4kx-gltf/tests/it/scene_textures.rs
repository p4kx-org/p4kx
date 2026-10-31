// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Textures of scene parts: embedded once however many parts use them, at the chosen mip, with
//! the palette of a part as the factor over the shared image.

use p4kx_gltf::{
    GlbOptions, MaterialMode, Scene, SceneNode, ScenePart, TextureLoadError, TextureRole,
    write_scene_glb,
};
use p4kx_model::{MaterialStatus, Model, TintEntry, TintPalette, Transform};
use p4kx_testkit::{TestMtlMaterial, check_glb};

use crate::scene_support::{
    TestResult, base_colours, close, library, read_cube, translation, write,
};
use crate::textures::support::{
    Stub, base_image, near, plain, plate, plate_stub, texture, textured,
};

fn scene_of<'m>(
    model: &'m Model,
    statuses: &[&'m MaterialStatus],
) -> Result<Scene<'m>, Box<dyn std::error::Error>> {
    let mut scene = Scene::new();
    for (position, status) in statuses.iter().enumerate() {
        let x = f32::from(u8::try_from(position)?) * 2.0;
        scene.push(
            SceneNode::new(format!("part{position}"), None, translation(x, 0.0, 0.0))
                .with_part(ScenePart::new(model, status)),
        )?;
    }
    Ok(scene)
}

fn scene_of_palettes<'m>(
    model: &'m Model,
    status: &'m MaterialStatus,
    palettes: &[&'m TintPalette],
) -> Result<Scene<'m>, Box<dyn std::error::Error>> {
    let mut scene = Scene::new();
    for (position, palette) in palettes.iter().enumerate() {
        scene.push(
            SceneNode::new(format!("part{position}"), None, Transform::IDENTITY)
                .with_part(ScenePart::new(model, status).with_palette(palette)),
        )?;
    }
    Ok(scene)
}

fn plates() -> Vec<TestMtlMaterial> {
    vec![plain("unused"), plate("caps"), plate("sides")]
}

fn other_plates() -> Vec<TestMtlMaterial> {
    vec![plain("unused"), plate("hull"), plate("trim")]
}

#[test]
fn the_parts_of_a_scene_embed_the_images_they_share_once() -> TestResult {
    let model = read_cube()?;
    let (first, second) = (library(&plates()), library(&other_plates()));
    let stub = plate_stub(
        [200, 100, 50, 255],
        [128, 128, 255, 255],
        [255, 255, 255, 255],
    )?;
    let alone = scene_of(&model, &[&first])?;
    let both = scene_of(&model, &[&first, &second])?;

    let (_, one) = write(&alone, &textured(), &stub)?;
    let (glb, two) = write(&both, &textured(), &stub)?;

    assert!(one.textures > 0, "{one:?}");
    assert_eq!(two.textures, one.textures, "the second part adds no image");
    assert_eq!(glb.document.images().len(), one.textures);
    assert_eq!(two.shared_meshes, 0, "two libraries, two meshes");
    assert!(two.shared_textures > one.shared_textures, "{one:?} {two:?}");
    assert_eq!(two.textured_materials, 2 * one.textured_materials);
    Ok(())
}

#[test]
fn repeated_parts_decode_their_textures_once() -> TestResult {
    let model = read_cube()?;
    let status = library(&plates());
    let single = plate_stub(
        [200, 100, 50, 255],
        [128, 128, 255, 255],
        [255, 255, 255, 255],
    )?;
    let repeated = plate_stub(
        [200, 100, 50, 255],
        [128, 128, 255, 255],
        [255, 255, 255, 255],
    )?;
    let alone = scene_of(&model, &[&status])?;
    let thrice = scene_of(&model, &[&status, &status, &status])?;

    write(&alone, &textured(), &single)?;
    let (_, summary) = write(&thrice, &textured(), &repeated)?;

    assert_ne!(single.calls().len(), 0);
    assert_eq!(
        repeated.calls().len(),
        single.calls().len(),
        "the repeated parts load nothing more"
    );
    assert_eq!(summary.shared_meshes, 2 * summary.meshes);
    Ok(())
}

#[test]
fn the_texture_mip_applies_to_every_texture_of_the_scene() -> TestResult {
    let model = read_cube()?;
    let status = library(&plates());
    let stub = plate_stub(
        [200, 100, 50, 255],
        [128, 128, 255, 255],
        [255, 255, 255, 255],
    )?;
    let scene = scene_of(&model, &[&status])?;

    write(&scene, &textured().with_texture_mip(3), &stub)?;

    let calls = stub.calls();
    assert_ne!(calls.len(), 0);
    assert!(calls.iter().all(|(_, mip)| *mip == 3), "{calls:?}");
    Ok(())
}

#[test]
fn the_colour_writer_of_a_scene_reads_no_textures_whatever_the_options_say() -> TestResult {
    let model = read_cube()?;
    let status = library(&plates());
    let scene = scene_of(&model, &[&status])?;
    let mut bytes = Vec::new();

    let summary = write_scene_glb(&scene, &textured(), &mut bytes)?;

    assert_eq!(summary.textures, 0);
    assert_eq!(summary.texture_fallbacks.len(), 0, "nothing was asked for");
    assert_eq!(check_glb(&bytes)?.document.images().len(), 0);
    Ok(())
}

#[test]
fn colours_mode_embeds_nothing_even_with_a_texture_source() -> TestResult {
    let model = read_cube()?;
    let status = library(&plates());
    let stub = plate_stub(
        [200, 100, 50, 255],
        [128, 128, 255, 255],
        [255, 255, 255, 255],
    )?;
    let scene = scene_of(&model, &[&status])?;
    let options = GlbOptions::default().with_materials(MaterialMode::Colours);

    let (_, summary) = write(&scene, &options, &stub)?;

    assert_eq!(summary.textures, 0);
    assert_eq!(stub.calls().len(), 0);
    Ok(())
}

#[test]
fn a_texture_that_cannot_be_loaded_falls_back_to_the_colour_and_is_listed_once() -> TestResult {
    let model = read_cube()?;
    let (first, second) = (library(&plates()), library(&plates()));
    let stub = Stub::default()
        .failing("objects/test/plate_diff.dds", TextureLoadError::NotFound)
        .with(
            "objects/test/plate_ddna.dds",
            &texture([128, 128, 255, 255], true)?,
        )
        .with(
            "objects/test/plate_spec.dds",
            &texture([255, 255, 255, 255], false)?,
        );
    let scene = scene_of(&model, &[&first, &second])?;

    let (glb, summary) = write(&scene, &textured(), &stub)?;

    let diffuse: Vec<_> = summary
        .texture_fallbacks
        .iter()
        .filter(|fallback| fallback.role == TextureRole::Diffuse)
        .collect();
    assert_eq!(diffuse.len(), 2, "caps and sides, once each: {diffuse:?}");
    assert!(
        diffuse
            .iter()
            .all(|fallback| fallback.file == "Objects/Test/plate_diff.tif"),
        "{diffuse:?}"
    );
    assert!(glb.document.materials().len() > 0);
    Ok(())
}

#[test]
fn parts_with_different_palettes_share_the_images_and_differ_in_the_colour_factor() -> TestResult {
    let model = read_cube()?;
    let status = library(&[
        plain("unused"),
        TestMtlMaterial::new("caps")
            .shader("HardSurface")
            .layer("Primary", "1")
            .texture("TexSlot1", "Objects/Test/plate_diff.tif"),
        TestMtlMaterial::new("sides")
            .shader("HardSurface")
            .layer("Primary", "1")
            .texture("TexSlot1", "Objects/Test/plate_diff.tif"),
    ]);
    let palette = |name: &str, tint: f32| {
        let entry = TintEntry {
            tint: [tint; 3],
            specular: [0.05; 3],
            glossiness: 100.0,
        };
        TintPalette::new(name, [entry; 3], [1.0; 3])
    };
    let (first, second) = (palette("first", 0.25), palette("second", 0.75));
    let stub = Stub::default().with(
        "objects/test/plate_diff.dds",
        &texture([200, 100, 50, 255], false)?,
    );
    let mut scene = Scene::new();
    for (name, palette) in [("a", &first), ("b", &second)] {
        scene.push(
            SceneNode::new(name, None, Transform::IDENTITY)
                .with_part(ScenePart::new(&model, &status).with_palette(palette)),
        )?;
    }

    let (glb, summary) = write(&scene, &textured(), &stub)?;

    assert_eq!(summary.textures, 1, "one diffuse image for both liveries");
    assert_eq!(summary.shared_meshes, 0);
    assert_eq!(summary.materials, 4, "caps and sides for each palette");
    assert_eq!(summary.textured_materials, 4);
    let colours = base_colours(&glb);
    assert!(close(colours[0], [0.25; 3]), "{colours:?}");
    assert!(close(colours[3], [0.75; 3]), "{colours:?}");
    Ok(())
}

#[test]
fn a_specular_map_bakes_the_palette_colours_into_the_images_of_each_livery() -> TestResult {
    let model = read_cube()?;
    let status = library(&[
        plain("unused"),
        TestMtlMaterial::new("caps")
            .shader("HardSurface")
            .layer("Primary", "1")
            .texture("TexSlot1", "Objects/Test/plate_diff.tif")
            .texture("TexSlot4", "Objects/Test/plate_spec.tif"),
    ]);
    let palette = |name: &str, tint: f32| {
        let entry = TintEntry {
            tint: [tint; 3],
            specular: [0.05; 3],
            glossiness: 100.0,
        };
        TintPalette::new(name, [entry; 3], [1.0; 3])
    };
    let (first, second) = (palette("first", 0.25), palette("second", 0.75));
    let stub = Stub::default()
        .with(
            "objects/test/plate_diff.dds",
            &texture([200, 100, 50, 255], false)?,
        )
        .with(
            "objects/test/plate_spec.dds",
            &texture([255, 255, 255, 255], false)?,
        );
    let (_, one) = write(
        &scene_of_palettes(&model, &status, &[&first])?,
        &textured(),
        &stub,
    )?;
    let (_, two) = write(
        &scene_of_palettes(&model, &status, &[&first, &second])?,
        &textured(),
        &stub,
    )?;

    assert_eq!(
        two.textures,
        one.textures + 1,
        "the base colour image of the second livery has its own colours: {one:?} {two:?}"
    );
    Ok(())
}

#[test]
fn a_stencil_decal_that_takes_the_decal_colours_is_painted_with_those_of_each_livery() -> TestResult
{
    let model = read_cube()?;
    let status = library(&[
        plain("unused"),
        plain("caps"),
        TestMtlMaterial::new("logo")
            .shader("MeshDecal")
            .string_gen_mask("%STENCIL_MAP")
            .texture("TexSlot7", "Objects/Test/logo_stencil.tif")
            .public_param("StencilDiffuseColor", "0.1,0.2,0.3")
            .public_param("StencilTintOverride", "2"),
    ]);
    let entry = TintEntry::default();
    let palette = |name: &str, decal: [[f32; 3]; 3]| {
        TintPalette::new(name, [entry; 3], [1.0; 3]).with_decal(decal)
    };
    let first = palette("first", [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    let second = palette("second", [[0.0, 0.0, 1.0]; 3]);
    let stub = Stub::default().with(
        "objects/test/logo_stencil.dds",
        &texture([255, 0, 0, 255], false)?,
    );

    let (glb, summary) = write(
        &scene_of_palettes(&model, &status, &[&first, &second, &first])?,
        &textured(),
        &stub,
    )?;

    assert_eq!(summary.textures, 2, "one image for each decal colour set");
    let logos: Vec<_> = glb
        .document
        .materials()
        .filter(|material| material.name() == Some("logo"))
        .collect();
    let pixels: Vec<_> = logos
        .iter()
        .map(|logo| base_image(&glb, logo).map(|image| image.pixel(0, 0)))
        .collect::<Result<_, _>>()?;
    assert!(
        near(pixels[0], [255, 0, 0, 255]),
        "the red mask is red: {pixels:?}"
    );
    assert!(near(pixels[1], [0, 0, 255, 255]), "{pixels:?}");
    for logo in &logos {
        assert_eq!(
            logo.pbr_metallic_roughness().base_color_factor()[..3],
            [1.0; 3],
            "the colours are in the image"
        );
    }
    Ok(())
}
