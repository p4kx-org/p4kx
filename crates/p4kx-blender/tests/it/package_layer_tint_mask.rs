// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use p4kx_assembly::ArchiveModels;
use p4kx_blender::{
    Manifest, PackageInfo, PackageWriteContext, PaletteEntry, TextureUse,
    write_package_with_context,
};
use p4kx_extraction::{Archive, ArchiveWriteProtection, CancelToken};
use p4kx_gltf::MaterialMode;
use p4kx_testkit::{
    DataForgeBuilder, SplitDds, TestEntity, TestEntry, TestModel, TestMtl, TestMtlMaterial,
    TestTextureFormat, check_glb, entity_schema, test_key, tint_palette_record_with_specular,
};

use crate::support::{self, TestResult};

const ROOT_CLASS: &str = "TEST_MaskedLayerTintInputs";
const MATERIAL: &str = "MaskedLayerTintInputs";
const MODEL: &str = "Objects/Test/masked_layer_tint.cgf";
const PARENT: &str = "Objects/Test/masked_layer_tint";
const COMPANION: &str = "Objects/Test/masked_layer_tint_inputs.mtl";
const DIFFUSE_MAP: &str = "Objects/Test/masked_layer_tint_diff.dds";
const SPECULAR_MAP: &str = "Objects/Test/masked_layer_tint_spec.dds";
const MASK: &str = "Objects/Test/masked_layer_tint_blend.dds";
const MASK_RED: u8 = 64;
const PALETTE_GUID: u128 = 0xA680;
const ENTITY_GUID: u128 = 0xA681;
const PALETTE_RGB: [[u8; 3]; 2] = [[192, 128, 96], [64, 160, 224]];
const PALETTE_SPECULAR: [[u8; 3]; 2] = [[80, 64, 48], [48, 64, 80]];
const RASTER_TOLERANCE: f32 = 0.005;
const GLB_BASELINE: [f32; 3] = [0.061_875_3, 0.165_512_9, 0.084_469_0];
const BLENDER_BASELINE: [f32; 3] = [0.118_422_6, 0.243_935_9, 0.374_639_2];

struct LayerInput {
    name: &'static str,
    diffuse: [f32; 3],
    reflectance: [f32; 3],
    tint: [f32; 3],
    tint_token: &'static str,
    mode: &'static str,
    slot: PaletteEntry,
    slot_token: &'static str,
}

const LAYERS: [LayerInput; 2] = [
    LayerInput {
        name: "TestPaintA",
        diffuse: [0.25, 0.625, 0.5],
        reflectance: [0.031_25, 0.0625, 0.015_625],
        tint: [0.75, 0.25, 0.875],
        tint_token: "0.75,0.25,0.875",
        mode: "1",
        slot: PaletteEntry::Primary,
        slot_token: "1",
    },
    LayerInput {
        name: "TestPaintB",
        diffuse: [0.75, 0.5, 0.125],
        reflectance: [0.0625, 0.015_625, 0.031_25],
        tint: [0.5, 0.875, 0.625],
        tint_token: "0.5,0.875,0.625",
        mode: "2",
        slot: PaletteEntry::Secondary,
        slot_token: "2",
    },
];

#[test]
fn masked_layer_tint_preserves_distinct_authored_inputs_and_measures_glb() -> TestResult {
    let fixture = support::fixture(entries()?)?;
    let archive = Archive::open_with_key(&fixture.path, Some(test_key()))?;
    let models = ArchiveModels::new(&archive);
    let assembly = super::assemble(&archive, &models, ROOT_CLASS)?;
    let textures = models.textures();
    let options = support::glb_options(0).with_materials(MaterialMode::Textures);
    let mut glb = Vec::new();
    assembly.write_glb_with(&options, &textures, &mut glb)?;
    measure_glb(&glb)?;

    let temporary = tempfile::tempdir()?;
    let package = std::env::var_os("P4KX_SYNTHETIC_LAYER_TINT_MASK_PACKAGE")
        .map_or_else(|| temporary.path().join("package"), PathBuf::from);
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let context =
        PackageWriteContext::new(&protection, cancel.flag()).with_archive_texture_source(&textures);
    write_package_with_context(
        &assembly,
        &models,
        &textures,
        &options,
        &PackageInfo::default(),
        &package,
        &context,
    )?;
    verify_carried_inputs(&package)
}

fn measure_glb(bytes: &[u8]) -> TestResult {
    let glb = check_glb(bytes)?;
    let pbr = glb
        .document
        .materials()
        .find(|material| material.name() == Some(MATERIAL))
        .ok_or("masked GLB material")?
        .pbr_metallic_roughness();
    assert_eq!(
        pbr.metallic_factor().to_bits(),
        0.0_f32.to_bits(),
        "dielectric control"
    );
    let factor = pbr.base_color_factor();
    let texture = pbr.base_color_texture().ok_or("baked layered colour")?;
    let image = glb.decode(&texture.texture().source())?;
    assert_eq!((image.width, image.height), (2, 2), "blend mask dimensions");
    for y in 0..image.height {
        for x in 0..image.width {
            let [r, g, b, _] = image.pixel(x, y).ok_or("masked texel")?;
            let [r, g, b] = [r, g, b].map(super::srgb);
            let [fr, fg, fb, _] = factor;
            let observed = [r * fr, g * fg, b * fb];
            assert!(
                observed
                    .into_iter()
                    .zip(GLB_BASELINE)
                    .all(|(a, e)| (a - e).abs() < RASTER_TOLERANCE),
                "{x},{y}: {observed:?} versus {GLB_BASELINE:?}"
            );
            assert!(
                observed
                    .into_iter()
                    .zip(BLENDER_BASELINE)
                    .any(|(a, e)| (a - e).abs() > 0.05),
                "{x},{y}: the inputs discriminate the two current projections"
            );
            writeln!(
                io::stdout().lock(),
                "GLB linear Base Color ({x},{y}): {observed:?}"
            )?;
        }
    }
    Ok(())
}

fn verify_carried_inputs(package: &Path) -> TestResult {
    let manifest: Manifest = serde_json::from_slice(&fs::read(package.join("scene.json"))?)?;
    let hull = manifest
        .materials
        .iter()
        .find(|material| material.name == MATERIAL)
        .ok_or("masked package material")?;
    assert!(
        hull.textures
            .iter()
            .any(|texture| texture.usage == TextureUse::Blend && texture.texture.is_some()),
        "the authored mask remains usable in the package"
    );
    assert_eq!(
        hull.layers.len(),
        LAYERS.len(),
        "both authored wrapper layers"
    );
    for (layer, source) in hull.layers.iter().zip(LAYERS) {
        assert_eq!(
            layer.tint_colour.map(|v| v.map(f32::to_bits)),
            Some(source.tint.map(f32::to_bits)),
            "wrapper tint"
        );
        assert_eq!(layer.palette, Some(source.slot), "independent palette slot");
        assert!(
            layer
                .attributes
                .iter()
                .any(|attr| attr.name == "TintColor" && attr.value == source.tint_token),
            "the wrapper retains the original token"
        );
        let child_id = layer.material.as_deref().ok_or("resolved layer material")?;
        let child = manifest
            .materials
            .iter()
            .find(|material| material.id == child_id)
            .ok_or("child material")?;
        assert_eq!(
            child.diffuse.map(f32::to_bits),
            source.diffuse.map(f32::to_bits),
            "child diffuse"
        );
        assert_eq!(
            child.specular.map(f32::to_bits),
            source.reflectance.map(f32::to_bits),
            "child specular"
        );
        assert!(
            child
                .parameters
                .iter()
                .any(|param| param.name == "TintMode" && param.value == source.mode),
            "the resolved material retains its authored mode"
        );
    }
    Ok(())
}

fn data_forge() -> Vec<u8> {
    let palette = tint_palette_record_with_specular(
        PALETTE_GUID,
        "p_masked_layer_tint",
        [PALETTE_RGB[0], PALETTE_RGB[1], [0; 3]],
        [255; 3],
        [184; 3],
        [PALETTE_SPECULAR[0], PALETTE_SPECULAR[1], [0; 3]],
    );
    let entity = TestEntity::new(
        ENTITY_GUID,
        ROOT_CLASS,
        "entities/scitem/masked_layer_tint.xml",
    )
    .geometry(MODEL, Some(PARENT))
    .palette(PALETTE_GUID, "");
    entity_schema(DataForgeBuilder::new())
        .record(palette)
        .record(entity.record())
        .build()
}

fn entries() -> Result<Vec<TestEntry>, Box<dyn Error>> {
    let model = TestModel::cube();
    let hull = TestMtlMaterial::new(MATERIAL)
        .shader("LayerBlend_V2")
        .texture("TexSlot12", MASK);
    let hull = LAYERS.iter().enumerate().fold(hull, |hull, (at, layer)| {
        hull.layer_with(&[
            ("Name", &format!("BaseLayer{}", at + 1)),
            ("Path", COMPANION),
            ("Submtl", layer.name),
            ("PaletteTint", layer.slot_token),
            ("TintColor", layer.tint_token),
        ])
    });
    let layer_materials = LAYERS.map(|layer| {
        TestMtlMaterial::new(layer.name)
            .shader("Layer")
            .diffuse(layer.diffuse)
            .specular(layer.reflectance)
            .public_param("TintMode", layer.mode)
            .texture("TexSlot1", DIFFUSE_MAP)
            .texture("TexSlot4", SPECULAR_MAP)
    });
    Ok(vec![
        TestEntry::stored(format!("Data/{MODEL}"), model.cgf()),
        TestEntry::stored("Data/Objects/Test/masked_layer_tint.cgfm", model.cgfm()),
        TestEntry::stored(
            format!("Data/{PARENT}.mtl"),
            TestMtl::multi(&[
                TestMtlMaterial::new("unused"),
                hull,
                TestMtlMaterial::new("control"),
            ])
            .cryxml(),
        ),
        TestEntry::stored(
            format!("Data/{COMPANION}"),
            TestMtl::multi(&layer_materials).cryxml(),
        ),
        TestEntry::stored(format!("Data/{DIFFUSE_MAP}"), uniform_dds([255; 4])?),
        TestEntry::stored(format!("Data/{SPECULAR_MAP}"), uniform_dds([255; 4])?),
        TestEntry::stored(
            format!("Data/{MASK}"),
            uniform_dds([32, 224, MASK_RED, 255])?,
        ),
        TestEntry::zstd("Data/Game2.dcb", data_forge()).encrypted(),
    ])
}

fn uniform_dds(bgra: [u8; 4]) -> Result<Vec<u8>, Box<dyn Error>> {
    let fixture = SplitDds::new(TestTextureFormat::LegacyBgra8, 2, 2).mips(1);
    let mut bytes = fixture.merged_bytes();
    let pixels = bytes
        .get_mut(fixture.header_len()..)
        .ok_or("synthetic DDS payload")?;
    let (pixels, []) = pixels.as_chunks_mut::<4>() else {
        return Err("synthetic DDS has complete texels".into());
    };
    pixels.fill(bgra);
    Ok(bytes)
}
