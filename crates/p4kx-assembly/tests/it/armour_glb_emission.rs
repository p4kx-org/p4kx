// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_assembly::{ArchiveModels, Armour, AssemblyOptions, BodyVariant, Piece};
use p4kx_extraction::{Archive, GlbOptions};
use p4kx_testkit::{TestEntry, TestModel, TestMtl, TestMtlMaterial, armour_entries};

use crate::assembly_support::fixture;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn the_observed_writer_keeps_the_glb_and_reports_only_emitted_geometry_and_material_slots()
-> TestResult {
    let hidden = TestModel::cube().with_material_library("hidden");
    let library = TestMtl::single(&TestMtlMaterial::new("hidden").shader("NoDraw"));
    let mut entries = armour_entries();
    entries.extend([
        TestEntry::stored(r"Data\Objects\Test\hidden.cgf", hidden.cgf()),
        TestEntry::stored(r"Data\Objects\Test\hidden.cgfm", hidden.cgfm()),
        TestEntry::stored(r"Data\Objects\Test\hidden.mtl", library.cryxml()),
    ]);
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive);
    let armour = Armour::build(
        BodyVariant::Male,
        &[Piece::file("hidden", "Objects/Test/hidden.cgf", None)],
        &models,
        &AssemblyOptions::default(),
    )?;
    let animated = armour.animate(&[], &models);
    let options = GlbOptions::default();
    let mut baseline = Vec::new();
    let expected =
        armour.write_glb_animated_with(&animated, &options, &models.textures(), &mut baseline)?;
    let mut actual = Vec::new();
    let (summary, emission) =
        armour.write_glb_animated_observed(&animated, &options, &models.textures(), &mut actual)?;
    assert_eq!(actual, baseline);
    assert_eq!(summary, expected);
    assert!(summary.hidden_primitives > 0);
    let [_, hidden_part] = armour.parts() else {
        return Err("expected the hidden part in the Assembly".into());
    };
    let material = hidden_part
        .material_archive_origins()
        .library()
        .ok_or("hidden material origin")?;
    assert_eq!(material.entry().path(), r"Data\Objects\Test\hidden.mtl");
    let [body, hidden] = emission.parts() else {
        return Err("expected body and hidden model dispositions".into());
    };
    assert!(body.geometry());
    assert!(body.skinned());
    assert!(!body.morphs());
    assert!(body.material_slots().next().is_some());
    assert!(!hidden.geometry());
    assert!(hidden.material_slots().next().is_none());
    Ok(())
}

#[test]
fn a_resolved_layer_with_no_rendered_share_does_not_supply_an_emitted_material() -> TestResult {
    let model = TestModel::cube().with_material_library("layered");
    let main = TestMtl::single(
        &TestMtlMaterial::new("layered")
            .layer_with(&[("Path", "layers/used")])
            .layer_with(&[("Path", "layers/unused")]),
    );
    let layer = TestMtl::single(&TestMtlMaterial::new("surface"));
    let mut entries = armour_entries();
    entries.extend([
        TestEntry::stored(r"Data\Objects\Test\layered.cgf", model.cgf()),
        TestEntry::stored(r"Data\Objects\Test\layered.cgfm", model.cgfm()),
        TestEntry::stored(r"Data\Objects\Test\layered.mtl", main.cryxml()),
        TestEntry::stored(r"Data\layers\used.mtl", layer.cryxml()),
        TestEntry::stored(r"Data\layers\unused.mtl", layer.cryxml()).encrypted(),
    ]);
    let fixture = fixture(entries)?;
    let archive = Archive::open_with_key(&fixture.path, Some(p4kx_testkit::test_key()))?;
    let models = ArchiveModels::new(&archive);
    let armour = Armour::build(
        BodyVariant::Male,
        &[Piece::file("layered", "Objects/Test/layered.cgf", None)],
        &models,
        &AssemblyOptions::default(),
    )?;
    let mut glb = Vec::new();
    let (_, emission) = armour.write_glb_animated_observed(
        &armour.animate(&[], &models),
        &GlbOptions::default(),
        &models.textures(),
        &mut glb,
    )?;
    let [_, part] = armour.parts() else {
        return Err("expected one layered piece".into());
    };
    let unused = part
        .material_archive_origins()
        .layer(0, 1)
        .ok_or("resolved unused layer")?;
    assert_eq!(
        unused.entry().encryption(),
        p4kx_extraction::SourceEncryption::Encrypted
    );
    assert_eq!(unused.source_material(), Some(0));
    let [_, rendered] = emission.parts() else {
        return Err("expected one emitted layered piece".into());
    };
    assert_eq!(rendered.material_layers(0).collect::<Vec<_>>(), [0]);
    Ok(())
}
