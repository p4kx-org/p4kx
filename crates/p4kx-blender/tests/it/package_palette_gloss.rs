// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, LoadoutResolver};
use p4kx_blender::{
    AlphaMeaning, Manifest, PackageInfo, PackageWriteContext, Texture, TextureEncoding, TextureUse,
    write_package_with_context,
};
use p4kx_extraction::{Archive, ArchiveWriteProtection, CancelToken, DataForgeEntry, FieldPath};
use p4kx_gltf::MaterialMode;
use p4kx_testkit::{CheckedGlb, check_glb, test_key};
use serde_json::Value;

use crate::support::{TestResult, fixture, glb_options};

#[path = "package_palette_gloss_source.rs"]
mod source;

#[path = "package_palette_gloss_layers.rs"]
mod layers;

use source::{
    EXPLICIT_GLOSS_CASES, EXPLICIT_GLOSS_PATH, Multiplier, NORMAL_ALPHA, PALETTE_BYTES, ROOT_CLASS,
    TEXTURE_PATH,
};

#[test]
fn palette_gloss_fixture_compares_glb_and_blender_package_inputs() -> TestResult {
    let fixture = fixture(source::entries()?)?;
    let archive = Archive::open_with_key(&fixture.path, Some(test_key()))?;
    let models = ArchiveModels::new(&archive);
    let assembly = assemble(&archive, &models)?;
    let textures = models.textures();
    let options = glb_options(0).with_materials(MaterialMode::Textures);
    let mut glb = Vec::new();
    let summary = assembly.write_glb_with(&options, &textures, &mut glb)?;
    assert!(
        summary.texture_fallbacks.is_empty(),
        "the authored normal and gloss inputs are readable"
    );
    let glb = check_glb(&glb)?;
    verify_glb(&glb)?;
    layers::verify_glb(&glb)?;

    let temporary = tempfile::tempdir()?;
    let package = std::env::var_os("P4KX_SYNTHETIC_PALETTE_GLOSS_PACKAGE")
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
    let manifest = serde_json::from_slice(&fs::read(package.join("scene.json"))?)?;
    verify_palette(&manifest)?;
    verify_layer_tokens(&manifest)?;
    verify_image(&manifest, &package)?;
    verify_explicit_gloss_package(&manifest)?;
    layers::verify_package(&manifest)
}

fn assemble(archive: &Archive, models: &ArchiveModels<'_>) -> Result<Assembly, Box<dyn Error>> {
    let dcb = DataForgeEntry::read(archive)?;
    let forge = dcb.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&dcb);
    let record = resolver.entities().find(ROOT_CLASS)?;
    let root = resolver.resolve(&record)?;
    let palettes = ForgePalettes::new(&forge).with_entry(&dcb);
    Ok(Assembly::build_with_palettes(
        &root,
        models,
        &palettes,
        &AssemblyOptions::default(),
    )?)
}

fn cases() -> impl Iterator<Item = (u8, Multiplier, bool)> {
    source::palette_cases().flat_map(|(byte, multiplier)| {
        [true, false]
            .into_iter()
            .map(move |sampled| (byte, multiplier, sampled))
    })
}

fn verify_glb(glb: &CheckedGlb) -> TestResult {
    let pbr_of = |name: &str| {
        glb.document
            .materials()
            .find(|material| material.name() == Some(name))
            .map(|material| material.pbr_metallic_roughness())
            .ok_or_else(|| format!("exported material {name}"))
    };
    for (byte, multiplier, sampled) in cases() {
        let name = source::material_name(byte, multiplier, sampled);
        let pbr = pbr_of(&name)?;
        assert_eq!(
            pbr.base_color_factor().map(f32::to_bits),
            [1.0_f32, 0.0, 0.0, 1.0].map(f32::to_bits),
            "{name}: independent palette tint"
        );
        if sampled {
            let packed = pbr
                .metallic_roughness_texture()
                .ok_or("packed roughness texture")?;
            let image = glb.decode(&packed.texture().source())?;
            expect_unit_roughness_factor(pbr.roughness_factor(), &name);
            let [_, green, _, _] = image.pixel(0, 0).ok_or("decoded packed texel")?;
            let expected = ((f32::from(NORMAL_ALPHA) / 255.0) * multiplier.value)
                .mul_add(-(f32::from(byte) / 255.0), 1.0)
                .clamp(0.0, 1.0);
            assert!(
                (f32::from(green) / 255.0 - expected).abs() <= 0.5 / 255.0,
                "{name}: sampled palette finish"
            );
        } else {
            assert!(
                pbr.metallic_roughness_texture().is_none(),
                "{name}: no texture was authored"
            );
            let expected = (f32::from(byte) / 255.0).mul_add(-multiplier.value, 1.0);
            assert!(
                (pbr.roughness_factor() - expected).abs() < 1e-6,
                "{name}: no-map palette roughness"
            );
        }
    }
    for case in EXPLICIT_GLOSS_CASES {
        let pbr = pbr_of(case.name)?;
        let packed = pbr
            .metallic_roughness_texture()
            .ok_or("authored roughness texture")?;
        let image = glb.decode(&packed.texture().source())?;
        let [_, green, _, _] = image
            .pixel(0, 0)
            .ok_or("decoded authored roughness texel")?;
        expect_unit_roughness_factor(pbr.roughness_factor(), case.name);
        assert_eq!(green, case.green, "{}", case.name);
    }
    Ok(())
}

fn expect_unit_roughness_factor(factor: f32, name: &str) {
    assert_eq!(
        factor.to_bits(),
        1.0_f32.to_bits(),
        "{name}: roughness is in the packed image"
    );
}

fn verify_palette(manifest: &Manifest) -> TestResult {
    let provenance = manifest
        .palette_provenance
        .as_ref()
        .ok_or("palette provenance")?;
    let payloads = provenance
        .get("payloads")
        .and_then(Value::as_array)
        .ok_or("actual palette payloads")?;
    let uses = provenance
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .ok_or("palette contributors")?;
    for byte in PALETTE_BYTES {
        let (index, palette) = manifest
            .palettes
            .iter()
            .enumerate()
            .find(|(_, palette)| palette.name == source::palette_name(byte))
            .ok_or("authored palette")?;
        assert_eq!(
            palette.primary.glossiness.to_bits(),
            f32::from(byte).to_bits(),
            "the package retains the byte"
        );
        let index = u64::try_from(index)?;
        let payload = payloads
            .iter()
            .find(|value| value.get("palette").and_then(Value::as_u64) == Some(index))
            .ok_or("palette Record payload")?;
        verify_palette_origin(payload, uses)?;
    }
    Ok(())
}

fn verify_palette_origin(payload: &Value, uses: &[Value]) -> TestResult {
    let origin = payload
        .pointer("/fieldOrigins/~1primary~1glossiness")
        .ok_or("original glossiness")?;
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "palette uses encrypted DataForge bytes"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "complete palette evidence"
    );
    let id = origin
        .pointer("/sourceUsesRefs/0")
        .ok_or("palette use ID")?;
    let reference = uses
        .iter()
        .find(|value| value.get("id") == Some(id))
        .ok_or("palette use")?;
    assert_eq!(
        reference.pointer("/location/field").and_then(Value::as_str),
        Some("root.entryA.glossiness"),
        "the exact original byte field"
    );
    let record = payload
        .pointer("/record/index")
        .ok_or("palette Record index")?;
    assert_eq!(
        reference.pointer("/location/recordIndex"),
        Some(record),
        "the physical palette Record"
    );
    Ok(())
}

fn verify_layer_tokens(manifest: &Manifest) -> TestResult {
    for (byte, multiplier, sampled) in cases() {
        let name = source::material_name(byte, multiplier, sampled);
        let resource = manifest
            .materials
            .iter()
            .find(|material| material.name == name)
            .ok_or_else(|| format!("package material {name}"))?;
        let [layer] = resource.layers.as_slice() else {
            return Err("one authored paint layer per material".into());
        };
        assert_eq!(
            layer.gloss_mult.map(f32::to_bits),
            multiplier.token.map(|_| multiplier.value.to_bits()),
            "{name}: supplied multiplier or unmodified default"
        );
        assert_eq!(
            layer
                .attributes
                .iter()
                .find(|attribute| attribute.name == "GlossMult")
                .map(|attribute| attribute.value.as_str()),
            multiplier.token,
            "{name}: original token or its absence"
        );
        let material = layer.material.as_deref().ok_or("resolved layer material")?;
        let layer = manifest
            .materials
            .iter()
            .find(|entry| entry.id == material)
            .ok_or("layer description")?;
        let normal = layer.textures.iter().find(|entry| entry.slot == "TexSlot2");
        assert_eq!(
            normal.is_some(),
            sampled,
            "{name}: exact original sample slot"
        );
        if let Some(slot) = normal {
            assert_eq!(slot.file, TEXTURE_PATH, "{name}: original sample selector");
            assert!(slot.texture.is_some(), "{name}: actual sample path bound");
        }
    }
    Ok(())
}

fn verify_image(manifest: &Manifest, package: &Path) -> TestResult {
    let image = manifest
        .textures
        .iter()
        .find(|image| image.source.eq_ignore_ascii_case(TEXTURE_PATH))
        .ok_or("bound packed sample output")?;
    assert_eq!(
        image.alpha,
        AlphaMeaning::Smoothness,
        "the file declares exact alpha use"
    );
    verify_image_origin(image, false)?;
    let input = fs::File::open(package.join(&image.file))?;
    let mut png = png::Decoder::new(io::BufReader::new(input)).read_info()?;
    let mut bytes = vec![0; png.output_buffer_size().ok_or("pixel length")?];
    let decoded = png.next_frame(&mut bytes)?;
    assert_eq!(
        decoded.color_type,
        png::ColorType::Rgba,
        "normal data remains RGBA"
    );
    let rgba = bytes
        .get(..decoded.buffer_size())
        .ok_or("decoded sample pixels")?;
    let (pixels, remainder) = rgba.as_chunks::<4>();
    assert!(remainder.is_empty(), "whole RGBA pixels");
    assert!(
        pixels
            .iter()
            .all(|pixel| pixel.last() == Some(&NORMAL_ALPHA)),
        "original packed alpha is carried unchanged"
    );
    Ok(())
}

fn verify_explicit_gloss_package(manifest: &Manifest) -> TestResult {
    let image = manifest
        .textures
        .iter()
        .find(|texture| texture.source.eq_ignore_ascii_case(EXPLICIT_GLOSS_PATH))
        .ok_or("bound authored gloss image")?;
    assert_eq!(
        image.encoding,
        Some(TextureEncoding::Data),
        "Gloss samples are data"
    );
    verify_image_origin(image, true)?;
    for case in EXPLICIT_GLOSS_CASES {
        let material = manifest
            .materials
            .iter()
            .find(|material| material.name == case.name)
            .ok_or_else(|| format!("package material {}", case.name))?;
        assert!(material.layers.is_empty(), "{} is unlayered", case.name);
        let slot = material
            .textures
            .iter()
            .find(|slot| slot.usage == TextureUse::Gloss)
            .ok_or("authored Gloss slot")?;
        assert_eq!(slot.file, EXPLICIT_GLOSS_PATH, "{}", case.name);
        assert_eq!(
            slot.texture.as_deref(),
            Some(image.id.as_str()),
            "{}",
            case.name
        );
    }
    Ok(())
}

fn verify_image_origin(image: &Texture, encrypted: bool) -> TestResult {
    let metadata = image
        .source_metadata
        .as_deref()
        .ok_or("measured source DDS")?;
    let origin = metadata
        .field_origins
        .get(&FieldPath::parse("/image")?)
        .ok_or("actual output image uses")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(encrypted),
        "{}: source encryption is independent of the palette",
        image.source
    );
    assert!(
        origin.source_set_complete(),
        "{}: exact image origin",
        image.source
    );
    Ok(())
}
