// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Canonical package material paths retain their original source-library spelling.

use p4kx_blender::{Manifest, PackageInfo, manifest_schema, write_package};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, entity_schema, material_reference_entries,
};

use crate::support::{TestResult, assemble, fixture, glb_options, never};

#[test]
fn mapped_materials_use_data_relative_package_paths_and_keep_source_references() -> TestResult {
    for reference in [
        "/Objects/Test/paint.mtl",
        "//authoring-host/branch/Data/Objects/Test/paint.mtl",
    ] {
        let entity = TestEntity::new(1, "TEST_MaterialReference", "entities/test_material.xml")
            .geometry("Objects/Test/cube.cgf", None);
        let database = entity_schema(DataForgeBuilder::new())
            .record(entity.record())
            .build();
        let mut entries =
            material_reference_entries(reference, Some("Data/Objects/Test/paint.mtl"));
        entries.push(TestEntry::stored("Data/Game2.dcb", database));
        let fixture = fixture(entries)?;
        let archive = Archive::open(&fixture.path)?;
        let assembly = assemble(&archive, "TEST_MaterialReference")?;
        let models = p4kx_assembly::ArchiveModels::new(&archive);
        let folder = tempfile::tempdir()?;
        let target = folder.path().join("package");
        write_package(
            &assembly,
            &models,
            &models.textures(),
            &glb_options(0),
            &PackageInfo::default(),
            &target,
            &never,
        )?;
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(target.join("scene.json"))?)?;
        let validator = jsonschema::draft202012::new(&manifest_schema())?;
        assert!(validator.is_valid(&value), "the actual package validates");
        let manifest: Manifest = serde_json::from_value(value)?;
        assert_eq!(manifest.materials.len(), 2, "only the used materials");
        for material in &manifest.materials {
            assert_eq!(
                material.library, "Objects/Test/paint",
                "canonical package path"
            );
            let source = manifest
                .material_libraries
                .get(material.source_library.ok_or("source link")?)
                .ok_or("source root")?;
            assert_eq!(
                source.path, reference,
                "authored reference remains separate"
            );
        }
        assert!(
            manifest.issues.is_empty(),
            "synthetic package issues: {:?}",
            manifest.issues
        );
    }
    Ok(())
}
