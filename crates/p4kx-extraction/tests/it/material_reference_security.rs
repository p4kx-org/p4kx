// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Rejected host paths never become disk or Archive companion requests.

use std::fs;

use p4kx_extraction::ModelSource;
use p4kx_extraction::model::MaterialStatus;
use p4kx_testkit::{MATERIAL_REFERENCE_MODEL, TestModel, TestMtl, TestMtlMaterial};

use super::material_reference_support::{TARGET, TestResult, archive as fixture_archive};

#[test]
fn unsafe_or_ambiguous_archive_references_are_rejected() -> TestResult {
    for reference in [
        "/Objects/../Test/paint.mtl",
        "/Objects/./Test/paint.mtl",
        "/Objects//Test/paint.mtl",
        "/Objects/Test/paint.mtl:stream",
        "C:/Objects/Test/paint.mtl",
        "C:paint.mtl",
        "file:///Objects/Test/paint.mtl",
        r"\\authoring-host\branch\Data\Objects\Test\paint.mtl",
        r"\Objects\Test\paint.mtl",
        "//authoring-host/share/paint.mtl",
        "//authoring-host/Data/Objects/Test/paint.mtl",
        "//authoring-host/branch/Data/Objects/Data/paint.mtl",
        "//authoring-host/../Data/Objects/Test/paint.mtl",
        "//authoring-host//Data/Objects/Test/paint.mtl",
        "//authoring-host/./Data/Objects/Test/paint.mtl",
        "//authoring-host/branch:/Data/Objects/Test/paint.mtl",
        "//authoring-host/branch/Data/",
        "///authoring-host/branch/Data/Objects/Test/paint.mtl",
        "/Objects/Test/pa\u{1f}int.mtl",
    ] {
        let folder = tempfile::tempdir()?;
        let archive = fixture_archive(folder.path(), reference, Some(TARGET), [])?;
        let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
        assert!(
            matches!(
                &source.model().materials,
                MaterialStatus::Unreadable { path, target, .. }
                    if path == reference && target.is_none()
            ),
            "{reference}: {:?}",
            source.model().materials
        );
    }
    Ok(())
}

#[test]
fn disk_models_reject_rooted_and_escaping_references_even_with_matching_files() -> TestResult {
    for reference in [
        "/Objects/Test/paint.mtl",
        "//authoring-host/branch/Data/Objects/Test/paint.mtl",
        "../paint.mtl",
        r"..\paint.mtl",
        "C:/paint.mtl",
        r"\\authoring-host\share\paint.mtl",
        "file:///paint.mtl",
    ] {
        let folder = tempfile::tempdir()?;
        let model_folder = folder.path().join("model");
        fs::create_dir(&model_folder)?;
        let model = TestModel::cube().with_material_library(reference);
        let library = TestMtl::single(&TestMtlMaterial::new("sentinel")).cryxml();
        fs::write(model_folder.join("cube.cgf"), model.cgf())?;
        fs::write(model_folder.join("cube.cgfm"), model.cgfm())?;
        fs::write(model_folder.join("paint.mtl"), &library)?;
        fs::write(folder.path().join("paint.mtl"), &library)?;
        let source = ModelSource::from_file(&model_folder.join("cube.cgf"))?;
        assert!(
            matches!(
                &source.model().materials,
                MaterialStatus::Unreadable { target: None, .. }
            ),
            "{reference}: {:?}",
            source.model().materials
        );
        assert!(
            source.model().materials.library().is_none(),
            "no local or external library read"
        );
    }
    Ok(())
}
