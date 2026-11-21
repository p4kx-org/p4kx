// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Missing, unreadable and ambiguous Archive targets retain actionable diagnostics.

use std::fs;

use p4kx_extraction::model::MaterialStatus;
use p4kx_extraction::{GlbOptions, MaterialMode, ModelSource};
use p4kx_testkit::{MATERIAL_REFERENCE_MODEL, TestEntry, check_glb};

use super::material_reference_support::{TARGET, TestResult, archive as fixture_archive};

#[test]
fn missing_targets_keep_both_paths_and_export_explicit_fallback_materials() -> TestResult {
    for reference in [
        "/Objects/Test/paint.mtl",
        "//authoring-host/branch/Data/Objects/Test/paint.mtl",
    ] {
        let folder = tempfile::tempdir()?;
        let archive = fixture_archive(folder.path(), reference, None, [])?;
        let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
        assert!(
            matches!(
                &source.model().materials,
                MaterialStatus::Missing { path, target }
                    if path == reference && target.as_deref() == Some(TARGET)
            ),
            "missing retains original and mapped target"
        );
        let problem = source
            .model()
            .materials
            .problem()
            .ok_or("missing diagnostic")?;
        assert!(problem.contains(reference), "diagnostic names original");
        assert!(problem.contains(TARGET), "diagnostic names mapped target");
        let path = folder.path().join("fallback.glb");
        let summary = source.write_glb(
            &GlbOptions::default().with_materials(MaterialMode::Colours),
            &path,
        )?;
        let glb = check_glb(&fs::read(path)?)?;
        assert_eq!(summary.default_material_ids, 2, "both IDs use fallback");
        assert_eq!(summary.materials, 1, "one shared fallback material");
        let mesh = glb.document.meshes().next().ok_or("mesh")?;
        assert!(
            mesh.primitives()
                .all(|part| part.material().name() == Some("default")),
            "both primitives bind the fallback",
        );
    }
    Ok(())
}

#[test]
fn an_unreadable_mapped_library_retains_its_target() -> TestResult {
    let folder = tempfile::tempdir()?;
    let reference = "/Objects/Test/paint.mtl";
    let archive = fixture_archive(
        folder.path(),
        reference,
        None,
        [TestEntry::stored(TARGET, b"<Material")],
    )?;
    let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
    assert!(
        matches!(
            &source.model().materials,
            MaterialStatus::Unreadable { path, target, .. }
                if path == reference && target.as_deref() == Some(TARGET)
        ),
        "unreadable retains original and mapped target"
    );
    let problem = source
        .model()
        .materials
        .problem()
        .ok_or("unreadable diagnostic")?;
    assert!(
        problem.contains(reference) && problem.contains(TARGET),
        "actionable diagnostic"
    );
    Ok(())
}

#[test]
fn duplicate_physical_targets_are_refused_instead_of_picking_the_first() -> TestResult {
    for second in [TARGET, r"data\objects\test\PAINT.MTL"] {
        let folder = tempfile::tempdir()?;
        let reference = "/Objects/Test/paint.mtl";
        let archive = fixture_archive(
            folder.path(),
            reference,
            Some(TARGET),
            [TestEntry::stored(second, b"<Material/>")],
        )?;
        let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
        assert!(
            matches!(
                &source.model().materials,
                MaterialStatus::Unreadable { path, reason, .. }
                    if path == reference && reason.contains("more than one Entry")
                        && reason.contains(TARGET)
            ),
            "ambiguous physical target is not silently selected"
        );
    }
    Ok(())
}
