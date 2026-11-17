// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, LoadoutResolver};
use p4kx_blender::{
    Manifest, PackageInfo, PackageWriteContext, PaletteEntry, write_package_with_context,
};
use p4kx_extraction::{Archive, ArchiveWriteProtection, CancelToken, DataForgeEntry};
use p4kx_gltf::MaterialMode;
use p4kx_testkit::{CheckedGlb, check_glb, test_key};
use serde_json::Value;

use crate::support::{TestResult, fixture, glb_options};

#[path = "package_layer_tint_mask.rs"]
mod mask;
#[path = "package_layer_tint_modes_source.rs"]
mod source;

use source::{CASES, Case, Lane, PALETTE_DIFFUSE, PALETTE_METAL, ROOT_CLASS};

#[test]
fn layer_tint_mode_baseline_preserves_raw_values_and_matches_glb_colour() -> TestResult {
    let fixture = fixture(source::entries()?)?;
    let archive = Archive::open_with_key(&fixture.path, Some(test_key()))?;
    let models = ArchiveModels::new(&archive);
    let assembly = assemble(&archive, &models, ROOT_CLASS)?;
    let textures = models.textures();
    let mut glb = Vec::new();
    assembly.write_glb_with(&glb_options(0), &textures, &mut glb)?;
    verify_glb(&check_glb(&glb)?)?;

    let temporary = tempfile::tempdir()?;
    let package = std::env::var_os("P4KX_SYNTHETIC_LAYER_TINT_MODE_PACKAGE")
        .map_or_else(|| temporary.path().join("package"), PathBuf::from);
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let context =
        PackageWriteContext::new(&protection, cancel.flag()).with_archive_texture_source(&textures);
    let options = glb_options(0).with_materials(MaterialMode::Textures);
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
    verify_inputs(&manifest, &package)?;
    verify_palette_fields(&manifest)
}

fn assemble(
    archive: &Archive,
    models: &ArchiveModels<'_>,
    root_class: &str,
) -> Result<Assembly, Box<dyn Error>> {
    let dcb = DataForgeEntry::read(archive)?;
    let forge = dcb.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&dcb);
    let record = resolver.entities().find(root_class)?;
    let root = resolver.resolve(&record)?;
    let palettes = ForgePalettes::new(&forge).with_entry(&dcb);
    Ok(Assembly::build_with_palettes(
        &root,
        models,
        &palettes,
        &AssemblyOptions::default(),
    )?)
}

fn verify_glb(glb: &CheckedGlb) -> TestResult {
    for case in CASES {
        for lane in Lane::ALL {
            let name = lane.material_name(case);
            let pbr = glb
                .document
                .materials()
                .find(|material| material.name() == Some(name.as_str()))
                .ok_or_else(|| format!("exported material {name}"))?
                .pbr_metallic_roughness();
            let [r, g, b, _] = pbr.base_color_factor();
            let observed = [r, g, b];
            let expected = expected_colour(lane, case);
            assert!(
                observed
                    .into_iter()
                    .zip(expected)
                    .all(|(a, e)| (a - e).abs() < 1e-6),
                "{name}: {observed:?} versus {expected:?}"
            );
            let metal = if matches!(lane, Lane::Metal) {
                1.0_f32
            } else {
                0.0
            };
            assert_eq!(
                pbr.metallic_factor().to_bits(),
                metal.to_bits(),
                "{name}: metal share"
            );
            writeln!(
                io::stdout().lock(),
                "{name}: GLB colour {observed:?}, metallic {metal}"
            )?;
        }
    }
    Ok(())
}

fn expected_colour(lane: Lane, case: Case) -> [f32; 3] {
    match (lane, case.selected) {
        (Lane::Dielectric, false) => [1.0; 3],
        (Lane::Metal, false) => lane.authored_reflectance(),
        (Lane::Dielectric, true) => PALETTE_DIFFUSE.map(srgb),
        (Lane::Metal, true) => PALETTE_METAL.map(srgb),
    }
}

fn srgb(code: u8) -> f32 {
    let v = f32::from(code) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn verify_inputs(manifest: &Manifest, package: &Path) -> TestResult {
    for case in CASES {
        for lane in Lane::ALL {
            verify_layer(manifest, package, case, lane)?;
        }
    }
    Ok(())
}

fn verify_layer(manifest: &Manifest, package: &Path, case: Case, lane: Lane) -> TestResult {
    let name = lane.material_name(case);
    let hull = manifest
        .materials
        .iter()
        .find(|material| material.name == name)
        .ok_or("emitted parent material")?;
    let [layer] = hull.layers.as_slice() else {
        return Err("one selected source layer".into());
    };
    let slot = match lane {
        Lane::Dielectric => PaletteEntry::Primary,
        Lane::Metal => PaletteEntry::Tertiary,
    };
    assert_eq!(
        layer.palette,
        case.selected.then_some(slot),
        "{name}: independent palette slot"
    );
    let id = layer.material.as_deref().ok_or("resolved layer material")?;
    let child = manifest
        .materials
        .iter()
        .find(|material| material.id == id)
        .ok_or("child material")?;
    assert_eq!(
        layer.sub_material.as_deref(),
        Some(child.name.as_str()),
        "{name}: exact Submtl"
    );
    assert_eq!(child.shader, "Layer", "{name}: authored layer shader");
    assert_eq!(
        child.specular.map(f32::to_bits),
        lane.authored_reflectance().map(f32::to_bits),
        "{name}: original specular remains independent of palette"
    );
    let raw: Vec<_> = child
        .parameters
        .iter()
        .map(|entry| (entry.name.as_str(), entry.value.as_str()))
        .collect();
    let expected: Vec<_> = std::iter::once(("LayerTiling", "1"))
        .chain(case.parameters.iter().copied())
        .collect();
    assert_eq!(raw, expected, "{name}: every original token in file order");
    let library = child
        .source_library
        .and_then(|index| manifest.material_libraries.get(index))
        .ok_or("resolved material library source")?;
    let authored = library
        .authored_file
        .as_ref()
        .ok_or("source receipt for the original library")?;
    assert!(
        package.join(&authored.file).is_file(),
        "{name}: actual retained source"
    );
    Ok(())
}

fn verify_palette_fields(manifest: &Manifest) -> TestResult {
    let provenance = manifest
        .palette_provenance
        .as_ref()
        .ok_or("palette proof")?;
    let uses = provenance
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .ok_or("registered uses")?;
    let fields = provenance
        .pointer("/payloads/0/fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("palette fields")?;
    for (address, expected) in [
        ("/primary/tint/0", "root.entryA.tintColor.r"),
        ("/primary/specular/1", "root.entryA.specColor.g"),
        ("/tertiary/specular/2", "root.entryC.specColor.b"),
    ] {
        let origin = fields.get(address).ok_or("separate palette field")?;
        assert_eq!(
            origin
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(true),
            "{address}"
        );
        assert_eq!(
            origin.get("sourceSetComplete").and_then(Value::as_bool),
            Some(true),
            "{address}"
        );
        let id = origin
            .pointer("/sourceUsesRefs/0")
            .ok_or("exact contributor")?;
        let contributor = uses
            .iter()
            .find(|site| site.get("id") == Some(id))
            .ok_or("palette claim")?;
        assert_eq!(
            contributor
                .pointer("/location/field")
                .and_then(Value::as_str),
            Some(expected),
            "{address}"
        );
    }
    Ok(())
}
