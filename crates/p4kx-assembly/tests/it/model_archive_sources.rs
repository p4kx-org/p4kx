// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_assembly::{ArchiveModels, ModelFailure, ModelProvider};
use p4kx_extraction::model::ModelFile;
use p4kx_extraction::{Archive, OpenOptions, SourceEncryption};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestModel, test_key};

type TestResult = Result<(), Box<dyn Error>>;

const BASE: &str = r"Data\Objects\Test\cube.cgf";
const GEOMETRY: &str = r"Data\Objects\Test\cube.cgfm";
const DETAIL: &str = r"Data\Objects\Test\cube_lod1.cgfm";

fn run_fixture() -> Result<(tempfile::TempDir, Archive), Box<dyn Error>> {
    let model = TestModel::cube().with_lod(TestModel::cube().scaled(0.5));
    let geometry = model.lod_cgfm(1).ok_or("fixture LOD geometry")?;
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("model.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(BASE, model.cgf()))
        .entry(TestEntry::stored(GEOMETRY, model.cgfm()).encrypted())
        .entry(TestEntry::stored(DETAIL, geometry))
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    Ok((dir, archive))
}

#[test]
fn only_the_retained_geometry_and_effective_model_inputs_supply_the_selected_lod() -> TestResult {
    let (_directory, archive) = run_fixture()?;
    let models = ArchiveModels::new(&archive);
    for (lod, geometry, file, encryption) in [
        (
            0,
            GEOMETRY,
            ModelFile::Geometry,
            SourceEncryption::Encrypted,
        ),
        (
            1,
            DETAIL,
            ModelFile::LodGeometry(1),
            SourceEncryption::Unencrypted,
        ),
        (
            2,
            DETAIL,
            ModelFile::LodGeometry(1),
            SourceEncryption::Unencrypted,
        ),
    ] {
        let model = models.model("Objects/Test/cube.cgf", lod)?;
        let actual: Vec<_> = model
            .archive_sources()
            .iter()
            .filter_map(|source| {
                source
                    .origin()
                    .map(|origin| (source.file(), origin.entry().path(), origin.encryption()))
            })
            .collect();
        assert_eq!(
            actual,
            [
                (ModelFile::Model, BASE, SourceEncryption::Unencrypted),
                (file, geometry, encryption)
            ],
            "requested level {lod}"
        );
    }
    Ok(())
}

#[test]
fn an_unreadable_selected_companion_does_not_return_model_evidence() -> TestResult {
    let model = TestModel::cube();
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("model.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(BASE, model.cgf()))
        .entry(TestEntry::stored(GEOMETRY, b"invalid geometry").encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let result = ArchiveModels::new(&archive).model("Objects/Test/cube.cgf", 0);
    assert!(
        matches!(result, Err(ModelFailure::Unreadable { .. })),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn the_character_definition_is_bound_separately_from_the_model_and_geometry() -> TestResult {
    const DEFINITION: &str = r"Data\Objects\Test\character.cdf";
    let model = TestModel::cube();
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("character.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            DEFINITION,
            br#"<CharacterDefinition><Model File="Objects\Test\cube.cgf"/></CharacterDefinition>"#,
        ).encrypted())
        .entry(TestEntry::stored(BASE, model.cgf()))
        .entry(TestEntry::stored(GEOMETRY, model.cgfm()))
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let models = ArchiveModels::new(&archive);
    let character = models.model("Objects/Test/character.cdf", 0)?;
    let definition = character.definition_origin().ok_or("definition origin")?;
    assert_eq!(definition.entry().path(), DEFINITION);
    assert_eq!(definition.encryption(), SourceEncryption::Encrypted);
    assert!(character.archive_sources().iter().all(|source| {
        source
            .origin()
            .is_some_and(|origin| origin.encryption() == SourceEncryption::Unencrypted)
    }));
    assert!(
        models
            .model("Objects/Test/cube.cgf", 0)?
            .definition_origin()
            .is_none()
    );
    Ok(())
}
