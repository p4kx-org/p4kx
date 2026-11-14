// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Finding a model's material library through the companion resolver.

use std::cell::RefCell;
use std::error::Error;

use p4kx_chunks::ChunkFile;
use p4kx_model::{CompanionError, CompanionPath, MaterialStatus, Model};
use p4kx_testkit::{CUBE_MATERIAL_LIBRARY, TestModel, TestMtl, TestMtlMaterial};

type TestResult = Result<(), Box<dyn Error>>;

fn hull() -> TestMtlMaterial {
    TestMtlMaterial::new("hull")
}

fn glass() -> TestMtlMaterial {
    TestMtlMaterial::new("glass").shader("GlassPBR")
}
type Requests = RefCell<Vec<String>>;

fn read_with(
    model: &TestModel,
    files: &[(&str, Vec<u8>)],
    requests: &Requests,
) -> Result<Model, Box<dyn Error>> {
    let cgfm = model.cgfm();
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("cube.cgfm") => Some(cgfm.clone()),
            CompanionPath::Sibling(name) if name.starts_with("cube_lod") => None,
            CompanionPath::Sibling(name) => {
                requests.borrow_mut().push(format!("sibling {name}"));
                files
                    .iter()
                    .find(|(key, _)| *key == format!("sibling {name}"))
                    .map(|(_, bytes)| bytes.clone())
            }
            CompanionPath::Material(name) => {
                requests.borrow_mut().push(format!("material {name}"));
                files
                    .iter()
                    .find(|(key, _)| *key == format!("material {name}"))
                    .map(|(_, bytes)| bytes.clone())
            }
            _ => None,
        })
    };
    Ok(Model::read(
        "cube.cgf",
        &ChunkFile::parse(model.cgf())?,
        &mut companions,
    )?)
}

fn cube_with_library(name: &str) -> TestModel {
    TestModel::cube().with_material_library(name)
}

#[test]
fn the_model_reads_its_material_library_through_the_resolver() -> TestResult {
    let requests = Requests::default();
    let mtl = TestMtl::multi(&[glass(), hull()]).cryxml();

    let model = read_with(
        &TestModel::cube(),
        &[("material objects/test/cube", mtl)],
        &requests,
    )?;

    assert_eq!(
        *requests.borrow(),
        ["material objects/test/cube"],
        "asked once, below Data, without the extension"
    );
    let library = model.materials.library().ok_or("a loaded library")?;
    assert_eq!(library.materials.len(), 2, "both sub-materials");
    assert_eq!(model.materials.problem(), None, "no problem");
    Ok(())
}

#[test]
fn a_missing_library_is_a_status_and_the_model_still_reads() -> TestResult {
    let model = read_with(&TestModel::cube(), &[], &Requests::default())?;

    assert_eq!(
        model.materials,
        MaterialStatus::Missing {
            path: CUBE_MATERIAL_LIBRARY.to_owned(),
            target: None,
        },
        "missing"
    );
    assert!(
        model
            .materials
            .problem()
            .is_some_and(|p| p.contains(CUBE_MATERIAL_LIBRARY)),
        "the problem names the library"
    );
    assert_eq!(model.lods.len(), 1, "the geometry is read");
    Ok(())
}

#[test]
fn a_corrupt_library_is_unreadable_not_fatal() -> TestResult {
    let model = read_with(
        &TestModel::cube(),
        &[("material objects/test/cube", b"<Material".to_vec())],
        &Requests::default(),
    )?;

    assert!(
        matches!(&model.materials, MaterialStatus::Unreadable { path, .. } if path == CUBE_MATERIAL_LIBRARY),
        "{:?}",
        model.materials
    );
    assert!(model.materials.problem().is_some(), "reported");
    Ok(())
}

#[test]
fn a_resolver_failure_is_unreadable_not_fatal() -> TestResult {
    let cube = TestModel::cube();
    let cgfm = cube.cgfm();
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        match path {
            CompanionPath::Sibling("cube.cgfm") => Ok(Some(cgfm.clone())),
            CompanionPath::Material(_) => Err("the disk failed".into()),
            _ => Ok(None),
        }
    };

    let model = Model::read("cube.cgf", &ChunkFile::parse(cube.cgf())?, &mut companions)?;

    assert!(
        matches!(&model.materials, MaterialStatus::Unreadable { reason, .. } if reason.contains("the disk failed")),
        "{:?}",
        model.materials
    );
    Ok(())
}

#[test]
fn paths_that_leave_the_data_folder_are_never_asked_for() -> TestResult {
    for name in [
        "../../secret",
        "objects/../../secret",
        "/etc/passwd",
        "//authoring-host/branch/Data/Objects/Test/paint.mtl",
        "C:/secret",
        "objects//cube",
        "objects/./cube",
        "..",
    ] {
        let requests = Requests::default();

        let model = read_with(&cube_with_library(name), &[], &requests)?;

        assert!(
            requests.borrow().is_empty(),
            "{name}: {:?}",
            requests.borrow()
        );
        assert!(
            matches!(&model.materials, MaterialStatus::Unreadable { path, .. } if path == name),
            "{name}: {:?}",
            model.materials
        );
    }
    Ok(())
}

#[test]
fn backslashes_a_data_prefix_and_the_extension_are_normalised() -> TestResult {
    let requests = Requests::default();

    read_with(
        &cube_with_library("Data\\Objects\\Test\\Cube.MTL"),
        &[],
        &requests,
    )?;

    assert_eq!(*requests.borrow(), ["material Objects/Test/Cube"]);
    Ok(())
}

#[test]
fn a_bare_name_is_looked_for_beside_the_model_first() -> TestResult {
    let requests = Requests::default();
    let mtl = TestMtl::single(&hull()).cryxml();

    let model = read_with(
        &cube_with_library("cube_material"),
        &[("sibling cube_material.mtl", mtl)],
        &requests,
    )?;

    assert_eq!(*requests.borrow(), ["sibling cube_material.mtl"]);
    assert!(
        model.materials.library().is_some(),
        "found beside the model"
    );

    let requests = Requests::default();
    let model = read_with(&cube_with_library("cube_material"), &[], &requests)?;
    assert_eq!(
        *requests.borrow(),
        ["sibling cube_material.mtl", "material cube_material"],
        "then below Data"
    );
    assert!(matches!(model.materials, MaterialStatus::Missing { .. }));
    Ok(())
}

#[test]
fn a_model_that_names_no_library_has_none() -> TestResult {
    let requests = Requests::default();

    let model = read_with(&cube_with_library(""), &[], &requests)?;

    assert_eq!(model.materials, MaterialStatus::Unnamed);
    assert!(requests.borrow().is_empty(), "nothing to ask for");
    assert_eq!(model.materials.problem(), None, "no library is no problem");
    Ok(())
}
