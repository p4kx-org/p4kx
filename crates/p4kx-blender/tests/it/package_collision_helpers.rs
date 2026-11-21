// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, LoadoutResolver};
use p4kx_blender::{Manifest, PackageInfo, PackageWriteContext, write_package_with_context};
use p4kx_chunks::ivo;
use p4kx_extraction::{Archive, ArchiveWriteProtection, DataForgeEntry};
use p4kx_testkit::{
    BONE_THRUSTER, TestEntry, TestModel, bone_thruster_definition, bone_thruster_entries,
    bone_thruster_housing,
};
use serde_json::{Value, json};

use crate::support::{TestResult, fixture, glb_options};

#[test]
fn package_collision_helpers_are_optional_and_keep_original_attachment_rig() -> TestResult {
    let entries = entries()?;
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    let output = tempfile::tempdir()?;
    let base = output.path().join("standard");
    let opts = output.path().join("opt-in");
    write_pair(&archive, &base, &opts)?;
    let default: Manifest = serde_json::from_slice(&fs::read(base.join("scene.json"))?)?;
    let optional: Manifest = serde_json::from_slice(&fs::read(opts.join("scene.json"))?)?;
    assert_eq!(
        default.nodes, optional.nodes,
        "render hierarchy and topology"
    );
    assert_eq!(default.meshes, optional.meshes, "render mesh references");
    assert_eq!(
        default.version,
        p4kx_blender::FORMAT_VERSION,
        "default format version"
    );
    assert_eq!(
        optional.version,
        p4kx_blender::FORMAT_VERSION,
        "helpers use the current package version"
    );
    for mesh in &default.meshes {
        assert_eq!(
            fs::read(base.join(&mesh.file))?,
            fs::read(opts.join(&mesh.file))?,
            "opt-in writes unchanged render GLBs",
        );
    }
    assert!(
        default.collision_helpers.is_none(),
        "ordinary default has no helper"
    );
    assert!(
        !base.join("collision-helpers.json").exists(),
        "default omits companion file"
    );
    let reference = optional
        .collision_helpers
        .ok_or("collision helper manifest reference")?;
    let document: Value = serde_json::from_slice(&fs::read(opts.join(reference.file))?)?;
    let schema = jsonschema::validator_for(&p4kx_blender::collision_helpers_schema())?;
    assert!(
        schema.is_valid(&document),
        "collision companion satisfies its schema"
    );
    sources(&document, &opts)?;
    Ok(())
}

fn sources(document: &Value, output: &Path) -> TestResult {
    assert_eq!(
        document.pointer("/models/0/slots/0/disposition"),
        Some(&json!("ready")),
        "original node-bound triangle"
    );
    assert_eq!(
        document.pointer("/models/0/slots/1/disposition"),
        Some(&json!("whole_object")),
        "whole-object slot has explicit disposition"
    );
    assert_eq!(
        document.pointer("/instances/0/node"),
        Some(&json!(1)),
        "original attachment instance"
    );
    assert_eq!(
        document.pointer("/instances/0/cdf/selected_rig/bone"),
        Some(&json!(1)),
        "actual rig tip ordinal"
    );
    assert_eq!(
        document.pointer("/instances/0/cdf/declared_skeleton_file/value"),
        Some(&json!(r"Objects\Test\declared.chr")),
        "authored skeleton is independent"
    );
    assert_eq!(
        document.pointer("/instances/0/cdf/owner_from_attachment/state"),
        Some(&json!("present")),
        "capture assembled bind frame"
    );
    assert_eq!(
        document.pointer("/instances/1/cdf/owner_from_attachment/state"),
        Some(&json!("unsupported")),
        "missing bone has no asserted helper frame"
    );
    let receipt: Value = serde_json::from_slice(&fs::read(output.join("receipt.json"))?)?;
    assert!(
        receipt
            .get("files")
            .and_then(Value::as_array)
            .is_some_and(|rows| rows
                .iter()
                .any(|file| file.get("path").and_then(Value::as_str)
                    == Some("collision-helpers.json"))),
        "helper artifact is receipted"
    );
    Ok(())
}

fn write_pair(archive: &Archive, standard: &Path, optional: &Path) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(BONE_THRUSTER)?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(archive).with_model_node_sources();
    let assembly = Assembly::build_with_palettes(
        &tree,
        &models,
        &ForgePalettes::new(&forge),
        &AssemblyOptions::default(),
    )?;
    let cancelled = AtomicBool::new(false);
    let protection = ArchiveWriteProtection::new(archive.path(), &cancelled)?;
    let context = PackageWriteContext::new(&protection, &cancelled);
    let collider = context.with_collision_helpers(archive);
    for (destination, context) in [
        (standard, context),
        (optional, collider),
        (optional, collider),
    ] {
        write_package_with_context(
            &assembly,
            &models,
            &models.textures(),
            &glb_options(0),
            &PackageInfo::new(None, None),
            destination,
            &context,
        )?;
    }
    Ok(())
}

fn entries() -> Result<Vec<TestEntry>, Box<dyn std::error::Error>> {
    let good = bone_thruster_housing("tip", r"Objects\Test\housing.cgf");
    let bad = bone_thruster_housing("unknown", r"Objects\Test\housing.cgf")
        .replace("Housing", "Missing bone");
    let cdf = bone_thruster_definition(&format!("{good}{bad}")).replace(
        "<AttachmentList>",
        r#"<Model File="Objects\Test\declared.chr"/><AttachmentList>"#,
    );
    let mut entries = bone_thruster_entries(&cdf);
    let name = r"Data\Objects\Test\housing.cgf";
    let housing = entries
        .iter_mut()
        .find(|entry| entry.path() == name)
        .ok_or("housing fixture")?;
    let cube = TestModel::cube()
        .with_material_library("Objects/Test/paint")
        .with_chunk(ivo::STAT_OBJ_PHYSICS.id(), collision()?);
    *housing = TestEntry::stored(name, cube.cgf());
    Ok(entries)
}

fn collision() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut geometry = vec![0; 68];
    geometry
        .get_mut(..4)
        .ok_or("geometry marker")?
        .copy_from_slice(&2_u32.to_le_bytes());
    for word in [1_u32, 3, 1, 3, 0, 1] {
        geometry.extend(word.to_le_bytes());
    }
    geometry.extend([1, 0, 0, 0, 0, 0, 7]);
    for value in [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        geometry.extend(value.to_le_bytes());
    }
    for index in [0_u16, 1, 2] {
        geometry.extend(index.to_le_bytes());
    }
    let mut chunk = vec![0; 80];
    chunk
        .get_mut(40..44)
        .ok_or("table count")?
        .copy_from_slice(&2_u32.to_le_bytes());
    for key in [0_u32, 0xFFFF] {
        chunk.extend(u32::try_from(geometry.len())?.to_le_bytes());
        chunk.extend(key.to_le_bytes());
    }
    chunk.extend(&geometry);
    chunk.extend(geometry);
    Ok(chunk)
}
