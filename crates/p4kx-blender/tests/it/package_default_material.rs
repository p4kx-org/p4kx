// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_assembly::ArchiveModels;
use p4kx_blender::{FallbackMaterialCause, Manifest, PackageInfo, manifest_schema, write_package};
use p4kx_extraction::{Archive, DefaultMaterialState, ModelSource, PathIndex, TextureVerdicts};
use p4kx_testkit::{
    CUBE_CAPS_MATERIAL, CUBE_SIDES_MATERIAL, DataForgeBuilder, SplitDds, TestEntity, TestEntry,
    TestMtl, TestMtlMaterial, TestTextureFormat, check_glb, entity_schema,
    material_reference_entries, test_key,
};

use crate::support::{Fixture, TestResult, assemble, fixture, glb_options, never};

const ENTITY: &str = "TEST_ArchiveDefaultMaterial";
const AUTHORED_REFERENCE: &str = "/Objects/Test/paint.mtl";
const AUTHORED_ENTRY: &str = "Data/Objects/Test/paint.mtl";
const DEFAULT_ENTRY: &str = "Data/Materials/material_default.mtl";
const FALLBACK_NAME: &str = "archive-fallback";
const DIFFUSE: [f32; 3] = [0.1, 0.2, 0.3];
const TEXTURE: &str = "Textures/Test/fallback_diff.dds";

fn archive_with_fallback(
    authored: Option<&[u8]>,
    encrypted: bool,
) -> Result<Fixture, Box<dyn Error>> {
    let entity = TestEntity::new(1, ENTITY, "entities/synthetic_archive_default_material.xml")
        .geometry("Objects/Test/cube.cgf", None);
    let database = entity_schema(DataForgeBuilder::new())
        .record(entity.record())
        .build();
    let mut entries = material_reference_entries(AUTHORED_REFERENCE, None);
    entries.push(TestEntry::stored("Data/Game2.dcb", database));
    if let Some(authored) = authored {
        entries.push(TestEntry::stored(AUTHORED_ENTRY, authored));
    }
    let library = TestMtl::single(
        &TestMtlMaterial::new(FALLBACK_NAME)
            .diffuse(DIFFUSE)
            .texture(p4kx_model::DIFFUSE_SLOT, TEXTURE),
    );
    let fallback = TestEntry::zstd(DEFAULT_ENTRY, library.cryxml());
    entries.push(if encrypted {
        fallback.encrypted()
    } else {
        fallback
    });
    entries.extend(
        SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4)
            .entries("Data/Textures/Test/fallback_diff.dds"),
    );
    fixture(entries)
}

fn write(archive: &Archive, out: &Path) -> Result<Manifest, Box<dyn Error>> {
    let assembly = assemble(archive, ENTITY)?;
    let models = ArchiveModels::new(archive);
    write_package(
        &assembly,
        &models,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::default(),
        out,
        &never,
    )?;
    let raw = serde_json::from_slice(&std::fs::read(out.join("scene.json"))?)?;
    let validator = jsonschema::draft202012::new(&manifest_schema())?;
    assert!(validator.is_valid(&raw), "package wire is schema-valid");
    Ok(serde_json::from_value(raw)?)
}

#[test]
fn physical_default_and_authored_failure_remain_independent() -> TestResult {
    let archive_file = archive_with_fallback(None, true)?;
    let archive = Archive::open(&archive_file.path)?;
    let folder = tempfile::tempdir()?;
    let without_key = write(&archive, &folder.path().join("without-key"))?;
    verify_synthetic(without_key)?;
    archive.set_key(test_key());
    let out = folder.path().join("with-key");
    let manifest = write(&archive, &out)?;
    let expected =
        ModelSource::archive_default_material(&PathIndex::new(&archive), &TextureVerdicts::new());
    assert_eq!(
        manifest.default_material.as_ref(),
        Some(&expected.receipt()),
        "package and model share exact source receipt"
    );
    assert!(
        manifest
            .issues
            .iter()
            .any(|issue| issue.kind == "material_library"),
        "authored lookup diagnostic matches the original result"
    );
    verify_resolved(&out, &manifest, &[CUBE_CAPS_MATERIAL, CUBE_SIDES_MATERIAL])
}

fn verify_synthetic(without_key: Manifest) -> TestResult {
    let receipt = without_key.default_material.ok_or("unreadable lookup")?;
    assert_eq!(
        receipt.state,
        DefaultMaterialState::Unreadable,
        "missing key is a typed read failure"
    );
    assert_eq!(
        receipt
            .source
            .ok_or("attempted source")?
            .contains_encrypted_source,
        Some(true),
        "attempted original retains its encryption marker"
    );
    assert!(
        receipt.decoded_sha256.is_none(),
        "unread payload has no digest"
    );
    let synthetic = &without_key
        .meshes
        .first()
        .ok_or("synthetic mesh")?
        .materials;
    assert_eq!(
        synthetic.len(),
        1,
        "both raw IDs share a synthetic fallback"
    );
    let synthetic = synthetic.first().ok_or("synthetic material")?;
    assert_eq!(
        synthetic.source_material_ids,
        [CUBE_CAPS_MATERIAL, CUBE_SIDES_MATERIAL],
        "synthetic assignment retains the same raw IDs"
    );
    assert!(
        !synthetic.archive_default && synthetic.material.is_none(),
        "synthetic slot has no physical material"
    );

    assert!(
        synthetic
            .fallback_slots
            .iter()
            .all(|slot| slot.cause == FallbackMaterialCause::MissingLibrary),
        "the unresolved authored library explains both original slots",
    );
    Ok(())
}

#[test]
fn sparse_authored_slots_keep_raw_ids_and_one_dense_fallback() -> TestResult {
    let library = br#"<Material><SubMaterials><Material/><Empty/><Material Name="side" Shader="Illum"/></SubMaterials></Material>"#;
    let archive_file = archive_with_fallback(Some(library), false)?;
    let archive = Archive::open(&archive_file.path)?;
    let folder = tempfile::tempdir()?;
    let out = folder.path().join("sparse");
    let manifest = write(&archive, &out)?;
    assert!(
        !manifest
            .issues
            .iter()
            .any(|issue| issue.kind == "material_library"),
        "authored lookup diagnostic matches the original result"
    );
    verify_resolved(&out, &manifest, &[CUBE_CAPS_MATERIAL])?;
    let mesh = manifest.meshes.first().ok_or("sparse mesh")?;
    assert_eq!(
        mesh.materials.len(),
        2,
        "one authored and one fallback material"
    );
    let side = mesh.materials.get(1).ok_or("authored assignment")?;
    assert_eq!(
        side.source_material_ids,
        [CUBE_SIDES_MATERIAL],
        "the authored ID stays distinct"
    );
    assert!(
        !side.archive_default,
        "authored material retains its own source"
    );
    assert_eq!(side.name, "side", "authored name is unchanged");
    Ok(())
}

#[test]
fn authored_root_is_retained_when_no_authored_slot_draws() -> TestResult {
    let library = b"<Material><SubMaterials><Material/><Empty/></SubMaterials></Material>";
    let archive_file = archive_with_fallback(Some(library), false)?;
    let archive = Archive::open(&archive_file.path)?;
    let folder = tempfile::tempdir()?;
    let out = folder.path().join("fallback-only");
    let manifest = write(&archive, &out)?;
    let mesh = manifest.meshes.first().ok_or("fallback-only mesh")?;
    assert_eq!(
        mesh.materials.len(),
        1,
        "both fallback causes share one dense material"
    );
    let assignment = mesh.materials.first().ok_or("dense fallback assignment")?;
    let library = manifest
        .material_libraries
        .get(
            assignment
                .authored_library
                .ok_or("authored library reference")?,
        )
        .ok_or("retained authored root")?;
    assert_eq!(
        library.path, AUTHORED_REFERENCE,
        "the authored root survives without a drawn material"
    );
    let causes: Vec<_> = assignment
        .fallback_slots
        .iter()
        .map(|slot| (slot.source_material_id, slot.cause))
        .collect();
    assert_eq!(
        causes,
        [
            (CUBE_CAPS_MATERIAL, FallbackMaterialCause::EmptySlot),
            (CUBE_SIDES_MATERIAL, FallbackMaterialCause::MissingSlot)
        ],
        "an explicit empty slot stays distinguishable from an out-of-range ID",
    );
    verify_resolved(&out, &manifest, &[CUBE_CAPS_MATERIAL, CUBE_SIDES_MATERIAL])
}

fn verify_resolved(out: &Path, manifest: &Manifest, source_ids: &[u16]) -> TestResult {
    let receipt = manifest
        .default_material
        .as_ref()
        .ok_or("fallback receipt")?;
    let source = receipt.source.as_ref().ok_or("original Entry")?;
    assert_eq!(
        receipt.state,
        DefaultMaterialState::Resolved,
        "verified fallback state"
    );
    assert_eq!(source.entry_path, DEFAULT_ENTRY, "original fallback path");
    assert!(
        receipt.decoded_sha256.is_some(),
        "verified original payload has a digest"
    );
    let mesh = manifest.meshes.first().ok_or("physical fallback mesh")?;
    let material = mesh.materials.first().ok_or("physical fallback material")?;
    assert_eq!(material.index, 0, "first emitted dense material");
    assert_eq!(material.name, FALLBACK_NAME, "physical fallback name");
    assert!(
        material.archive_default,
        "dense slot references the physical fallback"
    );
    assert_eq!(
        material.source_material_ids, source_ids,
        "raw IDs survive dense assignment"
    );
    verify_description(manifest, material)?;
    verify_glb(out, mesh, source_ids.len())
}

fn verify_description(manifest: &Manifest, material: &p4kx_blender::MeshMaterial) -> TestResult {
    let description = manifest
        .materials
        .iter()
        .find(|value| Some(&value.id) == material.material.as_ref())
        .ok_or("physical fallback material description")?;
    assert_eq!(
        description.diffuse, DIFFUSE,
        "physical authored diffuse color"
    );
    let library = manifest
        .material_libraries
        .get(description.source_library.ok_or("source library ordinal")?)
        .ok_or("physical fallback authored XML")?;
    assert_eq!(library.path, DEFAULT_ENTRY, "physical library source root");
    let diffuse = material
        .textures
        .diffuse
        .as_ref()
        .ok_or("fallback diffuse")?;
    assert!(
        manifest
            .textures
            .iter()
            .any(|texture| &texture.id == diffuse),
        "the physical fallback diffuse is written and linked"
    );
    Ok(())
}

fn verify_glb(out: &Path, mesh: &p4kx_blender::Mesh, source_count: usize) -> TestResult {
    let bytes = std::fs::read(out.join(&mesh.file))?;
    let glb = check_glb(&bytes)?;
    let dense = glb
        .document
        .materials()
        .next()
        .ok_or("GLB fallback material")?;
    assert_eq!(dense.name(), Some(FALLBACK_NAME), "physical GLB material");
    assert_eq!(
        dense.pbr_metallic_roughness().base_color_factor(),
        [0.1, 0.2, 0.3, 1.0],
        "physical GLB color with alpha"
    );
    let primitive_count = glb
        .document
        .meshes()
        .flat_map(|mesh| mesh.primitives())
        .filter(|primitive| primitive.material().index() == Some(0))
        .count();
    assert_eq!(
        primitive_count, source_count,
        "each affected primitive references the dense fallback"
    );
    Ok(())
}
