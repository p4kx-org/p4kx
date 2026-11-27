// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::PathBuf;

use p4kx_assembly::ArchiveModels;
use p4kx_blender::{
    AuthoredEmission, EmissionSample, Manifest, PackageInfo, PackageMaterialEmission,
    PackageWriteContext, write_package_with_context,
};
use p4kx_extraction::{Archive, ArchiveWriteProtection, CancelToken, FieldPath};
use p4kx_gltf::MaterialMode;
use p4kx_testkit::{CheckedGlb, check_glb, gltf, test_key};
use serde_json::{Value, json};

use crate::support::{TestResult, assemble, fixture, glb_options};

#[path = "package_forward_illum_source.rs"]
mod source;

use source::{
    CASES, Case, DIFFUSE, DIFFUSE_PIXEL, DIFFUSE_SLOT, EMITTANCE, OPACITY, ROOT_CLASS, TINT,
};

#[test]
fn forward_illum_package_preserves_original_controls_and_glb_alpha_independently() -> TestResult {
    let fixture = fixture(source::entries()?)?;
    let archive = Archive::open_with_key(&fixture.path, Some(test_key()))?;
    let assembly = assemble(&archive, ROOT_CLASS)?;
    let models = ArchiveModels::new(&archive);
    let textures = models.textures();
    let options = glb_options(0).with_materials(MaterialMode::Textures);
    let mut glb = Vec::new();
    let summary = assembly.write_glb_with(&options, &textures, &mut glb)?;
    assert!(
        summary.texture_fallbacks.is_empty(),
        "diffuse alpha uses its actual DDS"
    );
    verify_glb(&check_glb(&glb)?)?;

    let output = tempfile::tempdir()?;
    let package = std::env::var_os("P4KX_SYNTHETIC_FORWARD_ILLUM_PACKAGE")
        .map_or_else(|| output.path().join("package"), PathBuf::from);
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
    verify_package(&manifest)
}

fn verify_glb(glb: &CheckedGlb) -> TestResult {
    for case in CASES {
        let material = glb
            .document
            .materials()
            .find(|material| material.name() == Some(case.name))
            .ok_or_else(|| format!("ordinary GLB material {}", case.name))?;
        verify_glb_coverage(glb, &material, case.name)?;
        assert!(
            material.emissive_texture().is_none(),
            "{}: current GLB retains flat emission",
            case.name
        );
        let json = glb
            .document
            .as_json()
            .materials
            .get(material.index().ok_or("original GLB material index")?)
            .ok_or("GLB material row")?;
        let extension = json
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.emissive_strength.as_ref());
        assert_eq!(
            extension.is_some(),
            case.glow == Some("2"),
            "{}: only authored positive strength needs the extension",
            case.name
        );
        let strength = extension.map_or(1.0, |value| value.emissive_strength.0);
        let glow = case.glow.map_or(Ok(0.0), str::parse::<f32>)?;
        for (channel, tint) in material.emissive_factor().into_iter().zip(TINT) {
            let expected = tint * glow;
            assert!(
                channel.mul_add(strength, -expected).abs() < 1e-6,
                "{}: original flat emission",
                case.name
            );
        }
    }
    Ok(())
}

fn verify_glb_coverage(glb: &CheckedGlb, material: &gltf::Material<'_>, name: &str) -> TestResult {
    assert_eq!(
        material.alpha_mode(),
        gltf::material::AlphaMode::Blend,
        "{name}: authored coverage"
    );
    assert!(
        material.alpha_cutoff().is_none(),
        "{name}: blend coverage has no cutoff"
    );
    let pbr = material.pbr_metallic_roughness();
    let [_, _, _, opacity] = pbr.base_color_factor();
    assert_eq!(
        opacity.to_bits(),
        OPACITY.to_bits(),
        "{name}: independent scalar opacity"
    );
    let diffuse = pbr
        .base_color_texture()
        .ok_or("actual diffuse texture")?
        .texture()
        .source();
    assert_eq!(
        glb.decode(&diffuse)?
            .pixel(0, 0)
            .map(|[_, _, _, alpha]| alpha),
        DIFFUSE_PIXEL.last().copied(),
        "{name}: original DDS alpha in GLB"
    );
    Ok(())
}

fn verify_package(manifest: &Manifest) -> TestResult {
    for case in CASES {
        let material = manifest
            .materials
            .iter()
            .find(|material| material.name == case.name)
            .ok_or_else(|| format!("real package material {}", case.name))?;
        let emission = material
            .emission_source
            .as_ref()
            .ok_or("authored emission declarations")?;
        assert!(emission.source_complete, "complete source XML");
        assert_eq!(
            emission.forward_enabled,
            Some(true),
            "authored forward flag"
        );
        let library = manifest
            .material_libraries
            .get(emission.source_library)
            .ok_or("emission source library")?;
        let inventory = library
            .emission_provenance
            .as_ref()
            .ok_or("observed XML input")?;
        let slot = case.slot;
        verify_control(
            emission,
            inventory,
            "MtlFlags",
            Some("32768"),
            &[0, slot, 2],
        )?;
        verify_control(emission, inventory, "Glow", case.glow, &[0, slot, 6])?;
        verify_control(
            emission,
            inventory,
            "AlphaGlowCutoff",
            case.cutoff,
            &[0, slot, 1, 0],
        )?;
        let diffuse = material
            .textures
            .iter()
            .find(|texture| texture.slot == DIFFUSE_SLOT)
            .ok_or("original Diffuse slot")?;
        assert_eq!(diffuse.file, DIFFUSE, "verbatim Diffuse selector");
        verify_image_origin(manifest, diffuse.texture.as_deref(), DIFFUSE, true)?;
        verify_emittance(manifest, material, emission, inventory, case)?;
    }
    Ok(())
}

fn verify_control(
    emission: &PackageMaterialEmission,
    inventory: &Value,
    name: &str,
    expected: Option<&str>,
    indices: &[usize],
) -> TestResult {
    let actual = emission
        .attributes
        .iter()
        .enumerate()
        .find(|(_, value)| value.source.name == name);
    let Some(expected) = expected else {
        assert!(actual.is_none(), "{name}: source attribute is absent");
        return Ok(());
    };
    let (index, actual) = actual.ok_or("original control attribute")?;
    assert_eq!(actual.source.raw, expected, "{name}: original spelling");
    if name != "MtlFlags" {
        let AuthoredEmission::Scalar(EmissionSample::Finite(value)) = actual.value else {
            return Err("finite scalar control".into());
        };
        assert_eq!(
            value.to_bits(),
            expected.parse::<f32>()?.to_bits(),
            "{name}: typed scalar"
        );
    }
    verify_xml_use(
        emission,
        inventory,
        &format!("/attributes/{index}/source/raw"),
        name,
        indices,
    )
}

fn verify_emittance(
    manifest: &Manifest,
    material: &p4kx_blender::MaterialDescription,
    emission: &PackageMaterialEmission,
    inventory: &Value,
    case: Case,
) -> TestResult {
    assert_eq!(
        emission.emittance_requested,
        case.mapped.then_some(true),
        "explicit named route"
    );
    if !case.mapped {
        assert!(
            emission.preview.is_none() && emission.texture_slots.is_empty(),
            "Diffuse remains a separate ordinary texture"
        );
        return Ok(());
    }
    let [slot] = emission.texture_slots.as_slice() else {
        return Err("one explicit emittance declaration".into());
    };
    assert_eq!(slot.map.raw, "Emittance", "original named slot");
    let file = slot.file.as_ref().ok_or("original emittance file")?;
    assert_eq!(file.raw, EMITTANCE, "verbatim Emittance selector");
    verify_xml_use(
        emission,
        inventory,
        "/texture_slots/0/file/raw",
        "File",
        &[0, case.slot, 0, 1, 1],
    )?;
    let preview = emission.preview.as_ref().ok_or("explicit static preview")?;
    assert_eq!(
        preview.tint.map(f32::to_bits),
        TINT.map(f32::to_bits),
        "authored scene-linear tint"
    );
    assert_eq!(
        preview.strength.to_bits(),
        2.0_f32.to_bits(),
        "authored positive Glow"
    );
    let texture = material
        .textures
        .iter()
        .find(|texture| texture.slot == "Emittance")
        .ok_or("resolved Emittance slot")?;
    verify_image_origin(manifest, texture.texture.as_deref(), EMITTANCE, false)
}

fn verify_xml_use(
    emission: &PackageMaterialEmission,
    inventory: &Value,
    pointer: &str,
    field: &str,
    indices: &[usize],
) -> TestResult {
    let origin = emission
        .field_origins
        .get(&FieldPath::parse(pointer)?)
        .ok_or("exact original XML token")?;
    assert!(
        origin.source_set_complete(),
        "{field}: complete exact XML proof"
    );
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "{field}: encrypted material input"
    );
    let [reference] = origin.source_uses_refs() else {
        return Err("exactly one original XML token use".into());
    };
    let reference = serde_json::to_value(reference)?;
    let registered = inventory
        .get("uses")
        .and_then(Value::as_array)
        .ok_or("material source uses")?;
    let observed = registered
        .iter()
        .find(|word| word.get("id") == Some(&reference))
        .ok_or("original XML use")?;
    assert_eq!(
        observed
            .pointer("/location/namespace")
            .and_then(Value::as_str),
        Some("materialXmlAttribute"),
        "{field}: original material XML namespace"
    );
    assert_eq!(
        observed.pointer("/location/field").and_then(Value::as_str),
        Some(field),
        "{field}: original attribute name"
    );
    let coordinates: Vec<_> = indices.iter().map(usize::to_string).collect();
    assert_eq!(
        observed.pointer("/location/indices"),
        Some(&json!(coordinates)),
        "{field}: original slot and attribute"
    );
    Ok(())
}

fn verify_image_origin(
    manifest: &Manifest,
    id: Option<&str>,
    path: &str,
    encrypted: bool,
) -> TestResult {
    let id = id.ok_or("package texture binding")?;
    let texture = manifest
        .textures
        .iter()
        .find(|texture| texture.id == id)
        .ok_or("package texture")?;
    assert!(
        texture.source.eq_ignore_ascii_case(path),
        "selected physical texture name"
    );
    let metadata = texture
        .source_metadata
        .as_deref()
        .ok_or("actual DDS load metadata")?;
    let origin = metadata
        .field_origins
        .get(&FieldPath::parse("/image")?)
        .ok_or("image source")?;
    assert!(
        origin.source_set_complete(),
        "complete loaded texture evidence"
    );
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(encrypted),
        "the actual DDS is independent of the XML selector"
    );
    Ok(())
}
