// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The layer files shared by the libraries an [`ArchiveModels`] reads.

use p4kx_assembly::{ArchiveModels, ModelProvider};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    BONE_THRUSTER, TestEntry, TestMtl, TestMtlMaterial, bone_thruster_definition,
    bone_thruster_entries, layered_entries,
};

use crate::assembly_support::{TestResult, assemble, fixture};

const MODEL: &str = "Data\\Objects\\Test\\layered_ship.cgf";
const LIBRARY: &str = "Objects/Test/layered";

#[test]
fn the_libraries_of_one_provider_share_one_cache_of_layer_files() -> TestResult {
    let fixture = fixture(layered_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive);
    assert!(models.layer_cache().is_empty(), "nothing is read yet");

    let first = models.library(MODEL, LIBRARY);
    let kept = models.layer_cache().len();
    let second = models.library(MODEL, LIBRARY);

    assert_eq!(kept, 4, "base, wear, the lost one and the loop are kept");
    assert_eq!(
        models.layer_cache().len(),
        kept,
        "the second load adds none"
    );
    assert_eq!(first, second, "and reads the same surfaces");
    Ok(())
}

#[test]
fn the_assembly_lists_the_layer_files_the_archive_lacks_once() -> TestResult {
    let fixture = fixture(layered_entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, "TEST_Layered", 0)?;

    let unresolved = assembly.unresolved_layers();
    assert_eq!(unresolved.first(), Some("Objects/Test/layers/lost.mtl"));
    assert_eq!(
        unresolved.count(),
        1,
        "the loop is read, only the lost one is absent"
    );
    Ok(())
}

#[test]
fn a_bone_attachment_adds_the_layer_files_of_its_own_library() -> TestResult {
    let layered = TestMtlMaterial::new("housing")
        .layer_with(&[("Name", "A"), ("Path", "materials/layers/absent.mtl")]);
    let housing = r#"<Attachment Type="CA_BONE" AName="Housing" BoneName="tip"
        Binding="Objects\Test\housing.cgf" Material="Objects\Test\housing_paint.mtl"
        RelPosition="1,0,0" RelRotation="1,0,0,0"/>"#;
    let mut entries = bone_thruster_entries(&bone_thruster_definition(housing));
    entries.push(TestEntry::stored(
        r"Data\Objects\Test\housing_paint.mtl",
        TestMtl::multi(&[layered.clone(), layered.clone(), layered]).cryxml(),
    ));
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, BONE_THRUSTER, 0)?;

    let root = assembly.root().ok_or("a root")?;
    assert_eq!(root.attachments().len(), 1, "{:?}", assembly.issues());
    let unresolved = assembly.unresolved_layers();
    assert_eq!(unresolved.count(), 1, "three materials, one file");
    assert_eq!(unresolved.first(), Some("materials/layers/absent.mtl"));
    Ok(())
}
