// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_chunks::ChunkFile;
use p4kx_model::{
    CompanionError, CompanionPath, LodSelection, MaterialReading, MaterialStatus, Model,
    PositionBox, TintEntry, TintPalette, Transform,
};
use p4kx_testkit::{CheckedGlb, TestModel, TestMtl, TestMtlMaterial, check_glb};
use p4kx_texture::{DdsPngEncoding, DdsTransfer, DxgiFormat, RgbaImage, TextureFormat};

use super::{SLOT, fingerprint, matches};
use crate::{
    GlbOptions, LoadedTexture, MaterialMode, Scene, SceneNode, ScenePart, TextureLoadError,
    TextureLoader, write_scene_glb_with,
};

const LIBRARY: &str = "objects/test/fixture_bow";
const PALETTE: &str = "Fixture Bow Factory";
const MAP: &str = "objects/test/bow_spec.dds";
const DECLARATION: &str = "Objects/Test/bow_spec.tif";
const NORMAL: &str = "objects/test/bow_ddna.dds";
const SELECTED: &str = "selected";
const NEIGHBOUR: &str = "neighbour";

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct Textures {
    fail: bool,
    greatest: u8,
    transfer: DdsTransfer,
}

impl TextureLoader for Textures {
    fn dimensions(&self, path: &str, _: u32) -> Result<Option<(u32, u32)>, TextureLoadError> {
        Ok((path.eq_ignore_ascii_case(MAP) || path.eq_ignore_ascii_case(NORMAL)).then_some((2, 2)))
    }

    fn load(&self, path: &str, _: u32) -> Result<LoadedTexture, TextureLoadError> {
        let specular = path.eq_ignore_ascii_case(MAP);
        let pixels = if specular && !self.fail {
            [0, 32, 64, self.greatest].map(|x| [x, x, x, 19])
        } else if path.eq_ignore_ascii_case(NORMAL) {
            [[128, 128, 255, 127]; 4]
        } else {
            return Err(TextureLoadError::NotFound);
        };
        let image = RgbaImage::new(2, 2, pixels.concat())
            .map_err(|error| TextureLoadError::Unreadable(error.to_string()))?;
        let loaded = LoadedTexture::new(image, true);
        if specular && self.transfer != DdsTransfer::Unknown {
            let format = TextureFormat::Dxgi(DxgiFormat(28));
            return Ok(
                loaded.with_dds_transfer(self.transfer, DdsPngEncoding::of(format, self.transfer))
            );
        }
        Ok(loaded)
    }
}

fn model() -> TestResult<Model> {
    let cube = TestModel::cube();
    let geometry = cube.cgfm();
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(matches!(path, CompanionPath::Sibling("cube.cgfm")).then(|| geometry.clone()))
    };
    let mut model = Model::read_with_materials(
        "cube.cgf",
        &ChunkFile::parse(cube.cgf())?,
        &mut companions,
        LodSelection::All,
        PositionBox::default(),
        MaterialReading::Skipped,
    )?;
    let mesh = model
        .lods
        .first_mut()
        .and_then(|lod| lod.meshes.first_mut())
        .ok_or("fixture mesh")?;
    let [first, second] = mesh.submeshes.get_mut(..2).ok_or("two fixture draws")? else {
        return Err("two fixture draws".into());
    };
    first.material_id = u16::try_from(SLOT)?;
    second.material_id = u16::try_from(SLOT + 1)?;
    Ok(model)
}

fn library(name: &str, flags: &str, alias: &str) -> MaterialStatus {
    let wrapper = |name| {
        TestMtlMaterial::new(name)
            .shader("HardSurface")
            .string_gen_mask(flags)
            .public_param("WearBlendBase", "0.03")
            .layer_with(&[
                ("Name", "Primary"),
                ("Path", "layers/bow.mtl"),
                ("PaletteTint", "2"),
            ])
            .layer_with(&[
                ("Name", "Wear"),
                ("Path", "layers/wear.mtl"),
                ("PaletteTint", "2"),
            ])
    };
    let mut rows = vec![TestMtlMaterial::new("unused"); SLOT];
    rows.extend([wrapper(SELECTED), wrapper(NEIGHBOUR)]);
    let root = TestMtl::multi(&rows).cryxml();
    let source = TestMtl::single(
        &TestMtlMaterial::new("foundation")
            .shader("Layer")
            .texture("TexSlot2", "Objects/Test/bow_ddna.tif")
            .texture("TexSlot6", DECLARATION)
            .texture("TexSlot8", alias),
    )
    .cryxml();
    let wear = TestMtl::single(
        &TestMtlMaterial::new("wear")
            .shader("Layer")
            .texture("TexSlot2", "Objects/Test/bow_ddna.tif"),
    )
    .cryxml();
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(path) if path == name => Some(root.clone()),
            CompanionPath::Material("layers/bow") => Some(source.clone()),
            CompanionPath::Material("layers/wear") => Some(wear.clone()),
            _ => None,
        })
    };
    MaterialStatus::load(Some(name), &mut companions)
}

fn write(
    model: &Model,
    library: &MaterialStatus,
    palette: &str,
    maps: &Textures,
) -> TestResult<CheckedGlb> {
    let entry = TintEntry {
        tint: [0.2, 0.3, 0.4],
        specular: [0.05; 3],
        glossiness: 100.0,
    };
    let palette = TintPalette::new(palette, [entry; 3], [1.0; 3]);
    let mut scene = Scene::new();
    scene.push(
        SceneNode::new("fixture", None, Transform::IDENTITY)
            .with_part(ScenePart::new(model, library).with_palette(&palette)),
    )?;
    let mut bytes = Vec::new();
    let options = GlbOptions::default().with_materials(MaterialMode::Textures);
    write_scene_glb_with(&scene, &options, maps, &mut bytes)?;
    Ok(check_glb(&bytes)?)
}

#[test]
fn only_the_factory_slot_gets_the_dielectric_colour_texture() -> TestResult {
    let model = model()?;
    let maps = Textures {
        fail: false,
        greatest: 128,
        transfer: DdsTransfer::Srgb,
    };
    for flags in ["", "%USE_SPECULAR_MAPS"] {
        let library = library(LIBRARY, flags, DECLARATION);
        let glb = write(&model, &library, PALETTE, &maps)?;
        let alternate = write(&model, &library, "alternate fixture paint", &maps)?;
        let selected = glb
            .document
            .materials()
            .find(|m| m.name() == Some(SELECTED))
            .ok_or("selected glTF material")?;
        let witness = selected
            .specular()
            .ok_or("factory dielectric specular binding")?;
        assert_eq!(witness.specular_factor().to_bits(), 1.0_f32.to_bits());
        assert!(
            witness.specular_texture().is_none(),
            "grazing strength is unscaled"
        );
        let texture = witness
            .specular_color_texture()
            .ok_or("linear colour binding")?;
        assert_eq!(texture.tex_coord(), 0);
        let decoded = glb.decode(&texture.texture().source())?;
        for (at, encoded) in [0, 36, 72, 142].into_iter().enumerate() {
            let at = u32::try_from(at)?;
            let [red, green, blue, alpha] = decoded
                .pixel(at % 2, at / 2)
                .ok_or("the dielectric texture contains every source texel")?;
            assert_eq!([red, green, blue, alpha], [red, red, red, 255]);
            assert!(
                red.abs_diff(encoded) <= 1,
                "sRGB encoding stays within one byte"
            );
        }
        assert!(
            glb.document
                .extensions_used()
                .any(|name| name == "KHR_materials_specular")
        );
        assert!(
            !glb.document
                .extensions_required()
                .any(|name| name == "KHR_materials_specular")
        );
        assert!(
            glb.document
                .materials()
                .any(|m| m.name() == Some(NEIGHBOUR) && m.specular().is_none())
        );
        assert_core_material_matches(&glb, &alternate)?;
    }
    Ok(())
}

fn assert_core_material_matches(glb: &CheckedGlb, alternate: &CheckedGlb) -> TestResult {
    let selected = glb
        .document
        .materials()
        .find(|m| m.name() == Some(SELECTED))
        .ok_or("selected glTF material")?;
    let reference = alternate
        .document
        .materials()
        .find(|m| m.name() == Some(SELECTED))
        .ok_or("alternate glTF material")?;
    assert!(reference.specular().is_none());
    let pbr = selected.pbr_metallic_roughness();
    let plain = reference.pbr_metallic_roughness();
    assert_eq!(
        pbr.base_color_factor().map(f32::to_bits),
        plain.base_color_factor().map(f32::to_bits)
    );
    assert_eq!(
        pbr.metallic_factor().to_bits(),
        plain.metallic_factor().to_bits()
    );
    assert_eq!(
        pbr.roughness_factor().to_bits(),
        plain.roughness_factor().to_bits()
    );
    let normal = selected
        .normal_texture()
        .ok_or("selected normal texture")?
        .texture()
        .source();
    let alternate_normal = reference
        .normal_texture()
        .ok_or("reference normal texture")?
        .texture()
        .source();
    assert_eq!(glb.png(&normal)?, alternate.png(&alternate_normal)?);
    let finish = pbr
        .metallic_roughness_texture()
        .ok_or("selected finish texture")?
        .texture()
        .source();
    let alternate_finish = plain
        .metallic_roughness_texture()
        .ok_or("reference finish texture")?
        .texture()
        .source();
    assert_eq!(glb.png(&finish)?, alternate.png(&alternate_finish)?);
    Ok(())
}

#[test]
fn an_unverified_scope_or_missing_texture_keeps_the_existing_material() -> TestResult {
    let model = model()?;
    let good = Textures {
        fail: false,
        greatest: 128,
        transfer: DdsTransfer::Srgb,
    };
    let missing = Textures {
        fail: true,
        greatest: 128,
        transfer: DdsTransfer::Srgb,
    };

    let unknown = Textures {
        fail: false,
        greatest: 128,
        transfer: DdsTransfer::Unknown,
    };
    for (library_name, alias, map) in [
        ("objects/test/other_bow", DECLARATION, &good),
        (LIBRARY, "Objects/Test/other_spec.tif", &good),
        (LIBRARY, DECLARATION, &missing),
        (LIBRARY, DECLARATION, &unknown),
    ] {
        let glb = write(&model, &library(library_name, "", alias), PALETTE, map)?;
        assert!(
            glb.document
                .materials()
                .all(|material| material.specular().is_none())
        );
        assert!(
            !glb.document
                .extensions_used()
                .any(|name| name == "KHR_materials_specular")
        );
        assert!(
            glb.document
                .materials()
                .any(|material| material.normal_texture().is_some())
        );
    }
    Ok(())
}

#[test]
fn a_bright_texel_saturates_without_dropping_the_selected_texture() -> TestResult {
    let model = model()?;
    let library = library(LIBRARY, "", DECLARATION);
    let glb = write(
        &model,
        &library,
        PALETTE,
        &Textures {
            fail: false,
            greatest: 255,
            transfer: DdsTransfer::Srgb,
        },
    )?;
    let selected = glb
        .document
        .materials()
        .find(|m| m.name() == Some(SELECTED))
        .ok_or("selected glTF material")?;
    let specular = selected
        .specular()
        .ok_or("factory dielectric reflectance binding")?;
    let texture = specular
        .specular_color_texture()
        .ok_or("dielectric reflectance image")?;
    let image = glb.decode(&texture.texture().source())?;
    assert_eq!(image.pixel(1, 1), Some([255; 4]));
    assert_eq!(image.pixel(0, 1), Some([72, 72, 72, 255]));
    Ok(())
}

#[test]
fn factory_fingerprint_matches_only_the_normalized_path_and_exact_palette() {
    assert!(matches(LIBRARY, PALETTE));
    assert!(matches(r"OBJECTS\Test\FIXTURE_BOW", PALETTE));
    assert!(matches(r"Data\OBJECTS\Test\FIXTURE_BOW.mtl", PALETTE));
    assert!(!matches(
        "data/objects/test/../test/fixture_bow.mtl",
        PALETTE
    ));
    assert!(!matches("objects/test/other_bow", PALETTE));
    assert!(!matches(LIBRARY, "fixture bow factory"));
    assert!(!matches(LIBRARY, "Fixture Bow Factory "));
    assert_ne!(fingerprint("a", "bc"), fingerprint("ab", "c"));
}
