// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_extraction::{Archive, GlbOptions, MaterialMode, ModelSource};
use p4kx_testkit::{
    ArchiveBuilder, MATERIAL_REFERENCE_MODEL, check_glb, material_reference_entries,
};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn a_virtual_root_material_keeps_its_real_primitive_assignments() -> TestResult {
    assigned("/Objects/Test/paint.mtl")
}

#[test]
fn an_authoring_material_keeps_its_real_primitive_assignments() -> TestResult {
    assigned("//authoring-host/branch/Data/Objects/Test/paint.mtl")
}

fn assigned(reference: &str) -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive_path = folder.path().join("fixture.p4k");
    material_reference_entries(reference, Some("Data/Objects/Test/paint.mtl"))
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&archive_path)?;
    let archive = Archive::open(&archive_path)?;
    let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
    assert!(
        source.model().materials.library().is_some(),
        "the available material must load for {reference}: {:?}",
        source.model().materials.problem()
    );
    let target = folder.path().join("cube.glb");
    let options = GlbOptions::default().with_materials(MaterialMode::Colours);
    let summary = source.write_glb(&options, &target)?;
    assert_eq!(
        summary.default_material_ids, 0,
        "no grey replacement material"
    );
    let glb = check_glb(&fs::read(target)?)?;
    let mesh = glb
        .document
        .meshes()
        .next()
        .ok_or("exported mesh missing")?;
    assert_eq!(
        mesh.primitives().count(),
        2,
        "both authored primitive groups survive"
    );
    for (primitive, expected) in mesh.primitives().zip(["caps", "sides"]) {
        assert_eq!(
            primitive.material().name(),
            Some(expected),
            "the authored material is assigned"
        );
    }
    Ok(())
}
