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
