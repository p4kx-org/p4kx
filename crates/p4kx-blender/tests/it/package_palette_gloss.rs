// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, LoadoutResolver};
use p4kx_blender::{
    AlphaMeaning, Manifest, PackageInfo, PackageWriteContext, write_package_with_context,
};
use p4kx_extraction::{Archive, ArchiveWriteProtection, CancelToken, DataForgeEntry, FieldPath};
use p4kx_gltf::MaterialMode;
use p4kx_testkit::{CheckedGlb, check_glb, test_key};
use serde_json::Value;

use crate::support::{TestResult, fixture, glb_options};

#[path = "package_palette_gloss_source.rs"]
mod source;

use source::{MULTIPLIERS, Multiplier, NORMAL_ALPHA, PALETTE_BYTES, ROOT_CLASS, TEXTURE_PATH};

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
        "actual DDNA is sampled"
    );
    verify_glb(&check_glb(&glb)?)?;

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
    verify_image(&manifest, &package)
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
    PALETTE_BYTES.into_iter().flat_map(|byte| {
        MULTIPLIERS.into_iter().flat_map(move |multiplier| {
            [true, false]
                .into_iter()
                .map(move |sampled| (byte, multiplier, sampled))
        })
    })
}

fn verify_glb(glb: &CheckedGlb) -> TestResult {
    for (byte, multiplier, sampled) in cases() {
        let name = source::material_name(byte, multiplier, sampled);
        let pbr = glb
            .document
            .materials()
            .find(|material| material.name() == Some(name.as_str()))
            .ok_or_else(|| format!("exported material {name}"))?
            .pbr_metallic_roughness();
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
            assert_eq!(
                pbr.roughness_factor().to_bits(),
                1.0_f32.to_bits(),
                "{name}: roughness is in the packed image"
            );
            assert_eq!(
                image.pixel(0, 0).map(|[_, green, _, _]| green),
                Some(255 - NORMAL_ALPHA),
                "{name}: inverse original alpha in GLB green"
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
    Ok(())
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
            Some(multiplier.value.to_bits()),
            "{name}: supplied multiplier"
        );
        assert!(
            layer
                .attributes
                .iter()
                .any(|attribute| attribute.name == "GlossMult"
                    && attribute.value == multiplier.token),
            "{name}: original exponent-free token"
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
        Some(false),
        "actual sample comes from clear DDS, independent of the palette"
    );
    assert!(origin.source_set_complete(), "exact image origin");
    let input = fs::File::open(package.join(&image.file))?;
    let mut png = png::Decoder::new(std::io::BufReader::new(input)).read_info()?;
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
