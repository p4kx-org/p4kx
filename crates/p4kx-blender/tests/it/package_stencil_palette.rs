// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_assembly::{ArchiveModels, Assembly};
use p4kx_blender::{Manifest, PackageInfo, PaletteEntry, TextureUse};
use p4kx_extraction::Archive;
use p4kx_gltf::MaterialMode;
use p4kx_testkit::{
    DataForgeBuilder, SplitDds, TestEntity, TestEntry, TestModel, TestMtl, TestMtlMaterial,
    TestTextureFormat,
};

use crate::support::{self, TestResult};

#[path = "package_stencil_palette_fitted.rs"]
mod fitted;

const MODEL: &str = "Objects/Test/stencil.cgf";
const MATERIAL_PATH: &str = "Objects/Test/stencil";
const MATERIAL_NAME: &str = "TestStencil";
const TEXTURE: &str = "Objects/Test/stencil_mask.dds";
const COLOUR_GUID: u128 = 0x9701;
const PAINTED: &str = "TEST_StencilPainted";
const FALLBACK: &str = "TEST_StencilFallback";
const DECALS: [[u8; 3]; 3] = [[255, 0, 0], [0, 255, 0], [0, 0, 255]];
const MASKS: [[u8; 3]; 8] = [
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
    [255, 255, 255],
    [255, 0, 128],
    [0, 64, 0],
    [0, 0, 0],
    [128, 0, 0],
];
const CELL: usize = 4;
const STENCIL_PARAMETERS: [(&str, &str); 4] = [
    ("StencilDiffuseColor", "0.25,0.5,0.75"),
    ("StencilDiffuseColor3", "0.1250, 0.25,0.5"),
    ("StencilTintOverride", "2"),
    ("StencilDiffuseColor2", "0.75,0.50, 0.125"),
];

#[test]
fn stencil_palette_colours_are_projected_into_glb_and_the_blender_package() -> TestResult {
    let source = support::fixture(entries()?)?;
    let archive = Archive::open(&source.path)?;
    let temporary = tempfile::tempdir()?;
    let package = std::env::var_os("P4KX_SYNTHETIC_STENCIL_PACKAGE")
        .map_or_else(|| temporary.path().join("stencil"), PathBuf::from);
    archive_case(&archive, PAINTED, &package.join("painted"))?;
    archive_case(&archive, FALLBACK, &package.join("fallback"))
}

fn archive_case(archive: &Archive, class: &str, package: &Path) -> TestResult {
    let models = ArchiveModels::new(archive);
    let assembly = support::assemble(archive, class)?;
    let painted = class == PAINTED;
    let manifest = export_case(&assembly, &models, package, painted)?;
    verify_package(&manifest, painted)
}

fn export_case(
    assembly: &Assembly,
    models: &ArchiveModels<'_>,
    package: &Path,
    painted: bool,
) -> Result<Manifest, Box<dyn Error>> {
    let textures = models.textures();
    let options = support::glb_options(0).with_materials(MaterialMode::Textures);
    let mut glb = Vec::new();
    let summary = assembly.write_glb_with(&options, &textures, &mut glb)?;
    assert!(
        summary.texture_fallbacks.is_empty(),
        "the packed stencil is read by the normal GLB writer"
    );
    verify_glb(&glb, painted)?;
    p4kx_blender::write_package(
        assembly,
        models,
        &textures,
        &options,
        &PackageInfo::default(),
        package,
        &support::never,
    )?;
    Ok(serde_json::from_slice(&fs::read(
        package.join("scene.json"),
    )?)?)
}

fn verify_glb(bytes: &[u8], painted: bool) -> TestResult {
    let glb = p4kx_testkit::check_glb(bytes)?;
    let material = glb
        .document
        .nodes()
        .filter_map(|node| node.mesh())
        .flat_map(|mesh| mesh.primitives())
        .map(|primitive| primitive.material())
        .find(|material| material.name() == Some(MATERIAL_NAME))
        .ok_or("drawn synthetic stencil material")?;
    let pbr = material.pbr_metallic_roughness();
    let texture = pbr
        .base_color_texture()
        .ok_or("packed stencil base colour")?;
    let image = glb.decode(&texture.texture().source())?;
    let authored = if painted {
        [1.0; 4]
    } else {
        [0.25, 0.5, 0.75, 1.0]
    };
    assert_eq!(
        pbr.base_color_factor().map(f32::to_bits),
        authored.map(f32::to_bits),
        "GLB white for authored palette or linear material fallback"
    );
    // GLB stores RGB as sRGB bytes and coverage as an untransformed byte.
    let expected: [[u8; 4]; MASKS.len()] = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [0, 0, 255, 255],
        [255, 0, 0, 255],
        [0, 255, 0, 64],
        [0, 0, 0, 0],
        [255, 0, 0, 128],
    ];
    for (cell, (texel, mask)) in expected.into_iter().zip(MASKS).enumerate() {
        let [red, ..] = mask;
        let want = if painted { texel } else { [255, 255, 255, red] };
        let x = u32::try_from(cell * CELL + CELL / 2)?;
        let pixel = image
            .pixel(x, 0)
            .ok_or("a texel for each authored mask case")?;
        assert_eq!(pixel, want, "stencil cell {cell}, painted={painted}");
    }
    Ok(())
}

fn verify_package(manifest: &Manifest, painted: bool) -> TestResult {
    let decal = manifest
        .materials
        .iter()
        .find(|material| material.name == MATERIAL_NAME)
        .ok_or("package stencil texture binding")?;
    assert_eq!(
        decal.palette,
        Some(PaletteEntry::Decal),
        "authored override selects decal"
    );
    let parameters = decal
        .parameters
        .iter()
        .map(|parameter| (parameter.name.as_str(), parameter.value.as_str()));
    assert!(
        parameters.eq(STENCIL_PARAMETERS),
        "the package preserves the original stencil parameter text and order"
    );
    let stencil = decal.stencil.as_ref().ok_or("package stencil parameters")?;
    assert!(stencil.drawn, "written texture draws");
    assert_eq!(
        stencil.colour.map(|rgb| rgb.map(f32::to_bits)),
        Some([0.25_f32, 0.5, 0.75].map(f32::to_bits)),
        "source fallback remains available"
    );
    let slot = decal
        .textures
        .iter()
        .find(|slot| slot.usage == TextureUse::Stencil)
        .ok_or("typed stencil slot")?;
    assert_eq!(slot.file, TEXTURE, "exact authored packed source name");
    assert!(slot.texture.is_some(), "package stencil image is bound");
    let colour_sets = manifest.palettes.iter().filter_map(|palette| palette.decal);
    let expected = [[1.0_f32, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    if painted {
        let mut sets = colour_sets;
        assert_eq!(
            sets.next()
                .map(|entries| entries.map(|rgb| rgb.map(f32::to_bits))),
            Some(expected.map(|rgb| rgb.map(f32::to_bits))),
            "linear authored RGB lanes"
        );
        assert!(
            sets.next().is_none(),
            "one palette is attached to this model"
        );
    } else {
        assert_eq!(
            colour_sets.count(),
            0,
            "no decal palette is attached to the fallback model"
        );
    }
    Ok(())
}

fn entries() -> Result<Vec<TestEntry>, Box<dyn Error>> {
    let paint = TestEntity::new(0x9702, PAINTED, "entities/scitem/test_stencil_painted.xml")
        .geometry(MODEL, Some(MATERIAL_PATH))
        .palette(COLOUR_GUID, "");
    let fallback = TestEntity::new(
        0x9703,
        FALLBACK,
        "entities/scitem/test_stencil_fallback.xml",
    )
    .geometry(MODEL, Some(MATERIAL_PATH));
    let database = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(paint.record())
        .record(fallback.record())
        .record(p4kx_testkit::tint_palette_record_with_decal(
            COLOUR_GUID,
            "stencil_test",
            [[255; 3]; 3],
            [255; 3],
            [128; 3],
            DECALS,
        ))
        .build();
    let mut entries = stencil_assets()?;
    entries.push(TestEntry::zstd("Data/Game2.dcb", database));
    Ok(entries)
}

fn stencil_assets() -> Result<Vec<TestEntry>, Box<dyn Error>> {
    let model = TestModel::cube();
    let stencil = TestMtlMaterial::new(MATERIAL_NAME)
        .shader("MeshDecal")
        .specular([0.0; 3])
        .string_gen_mask("%STENCIL_MAP")
        .texture("TexSlot7", TEXTURE);
    let stencil = STENCIL_PARAMETERS
        .into_iter()
        .fold(stencil, |material, (name, value)| {
            material.public_param(name, value)
        });
    let library = TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        stencil,
        TestMtlMaterial::new("other"),
    ]);
    Ok(vec![
        TestEntry::stored(format!("Data/{MODEL}"), model.cgf()),
        TestEntry::stored(format!("Data/{MATERIAL_PATH}.cgfm"), model.cgfm()),
        TestEntry::stored(format!("Data/{MATERIAL_PATH}.mtl"), library.cryxml()),
        TestEntry::stored(format!("Data/{TEXTURE}"), mask_texture()?),
    ])
}

fn mask_texture() -> Result<Vec<u8>, Box<dyn Error>> {
    let width = u32::try_from(MASKS.len() * CELL)?;
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, width, 4).mips(1);
    let mut bytes = dds.merged_bytes();
    let sample = bytes
        .get_mut(dds.header_len()..)
        .ok_or("authored DDS mask payload")?;
    let (texels, []) = sample.as_chunks_mut::<4>() else {
        return Err("the authored mask has whole BGRA texels".into());
    };
    for (index, texel) in texels.iter_mut().enumerate() {
        let &[r, g, b] = MASKS
            .get((index / CELL) % MASKS.len())
            .ok_or("authored stripe colour")?;
        *texel = [b, g, r, 255];
    }
    Ok(bytes)
}
