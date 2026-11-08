// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_blender::{Manifest, PackageInfo, manifest_schema, write_package};
use p4kx_extraction::Archive;
use p4kx_extraction::model::MaterialStatus;
use p4kx_testkit::material_identity_entries;

use crate::support::{TestResult, assemble, fixture, glb_options, never};

const ROOT_MODEL: &str = "Objects/Test/root.cgf";
const SIBLING_MODEL: &str = "Objects/Test/sibling.cgf";
const ROOT_COLOUR: [f32; 3] = [0.8, 0.1, 0.1];
const SIBLING_COLOUR: [f32; 3] = [0.2, 0.4, 0.6];

#[test]
fn virtual_rooted_bare_and_sibling_materials_keep_distinct_package_bindings() -> TestResult {
    check_bindings("/paint.mtl")
}

#[test]
fn authoring_rooted_bare_and_sibling_materials_keep_distinct_package_bindings() -> TestResult {
    check_bindings("//host/branch/Data/paint.mtl")
}

#[test]
fn rejected_windows_root_does_not_poison_the_sibling_library_in_an_assembly() -> TestResult {
    let rooted = r"\paint.mtl";
    let fixture = fixture(material_identity_entries(rooted))?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = assemble(&archive, "TEST_RootMaterial")?;
    assert_eq!(assembly.parts().len(), 2, "both model parts are present");
    let root = assembly
        .parts()
        .iter()
        .find(|part| part.class_name() == "TEST_RootMaterial")
        .ok_or("root part absent")?;
    assert!(
        matches!(root.materials(), MaterialStatus::Unreadable { path, target: None, .. }
            if path == rooted),
        "the Windows-rooted reference remains rejected"
    );
    let sibling = assembly
        .parts()
        .iter()
        .find(|part| part.class_name() == "TEST_SiblingMaterial")
        .ok_or("sibling part absent")?;
    let library = sibling
        .materials()
        .library()
        .ok_or("rejected root poisoned the sibling library cache")?;
    assert_eq!(
        library.path, "paint",
        "the sibling owns its source reference"
    );
    assert_eq!(
        library.target.as_deref(),
        Some("Data/Objects/Test/paint.mtl"),
        "the sibling reads its local companion"
    );
    let material = library.materials.first().ok_or("sibling material absent")?;
    assert_eq!(
        material.name, "sibling_paint",
        "the correct library is loaded"
    );
    assert_eq!(material.diffuse, SIBLING_COLOUR, "source colour survives");
    Ok(())
}

fn check_bindings(rooted: &str) -> TestResult {
    let manifest = write(rooted)?;
    assert_eq!(
        manifest.meshes.len(),
        2,
        "both distinct model assets survive"
    );
    assert_eq!(
        manifest.materials.len(),
        2,
        "Data/paint.mtl must not reuse Data/Objects/Test/paint.mtl's description"
    );
    for (geometry, name, colour, reference) in [
        (ROOT_MODEL, "root_paint", ROOT_COLOUR, rooted),
        (SIBLING_MODEL, "sibling_paint", SIBLING_COLOUR, "paint"),
    ] {
        let mesh = manifest
            .meshes
            .iter()
            .find(|mesh| mesh.geometry.as_deref() == Some(geometry))
            .ok_or("expected model missing from the package")?;
        let [binding] = mesh.materials.as_slice() else {
            return Err("each model must bind its one authored material".into());
        };
        assert_eq!(binding.index, 0, "both models use glTF material zero");
        assert_eq!(binding.name, name, "GLB material identity");
        let description = manifest
            .materials
            .iter()
            .find(|material| Some(material.id.as_str()) == binding.material.as_deref())
            .ok_or("mesh material description missing")?;
        assert_eq!(description.name, name, "description belongs to this mesh");
        assert_eq!(description.diffuse, colour, "source colour is not replaced");
        assert_eq!(description.index, Some(0), "both source indices are zero");
        assert_eq!(
            description.library, "paint",
            "package spelling is preserved"
        );
        let source = manifest
            .material_libraries
            .get(description.source_library.ok_or("source link missing")?)
            .ok_or("source library missing")?;
        assert_eq!(source.path, reference, "each source reference survives");
    }
    assert!(
        manifest
            .issues
            .iter()
            .all(|issue| issue.kind != "material_source_ambiguity"),
        "distinct resolved libraries do not share a renderer identity"
    );
    Ok(())
}

fn write(rooted: &str) -> Result<Manifest, Box<dyn Error>> {
    let fixture = fixture(material_identity_entries(rooted))?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = assemble(&archive, "TEST_RootMaterial")?;
    assert_eq!(
        assembly.parts().len(),
        2,
        "the fixture assembles both models"
    );
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
    Ok(serde_json::from_value(value)?)
}
