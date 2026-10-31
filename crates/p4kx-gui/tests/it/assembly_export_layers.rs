// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The layer files the Archive lacks in the summaries of the Assembly exports: the GLB file with
//! the plural text, the Blender package, the Interior of an Entity and a container on its own.

use crate::entity_package_window::finished as package_finished;
use crate::entity_picking::{TestResult, Window, entries, opened_sized, select};
use crate::interior_export::{click_settled, export, select_socpak, summary};
use crate::waiting::act_until;

use egui_kittest::kittest::Queryable;
use p4kx_gui::{AssemblyExportForm, Language, PreviewTab, WorkTab};
use p4kx_testkit::{
    ENTITY_INTERIOR_CLASS, INTERIOR_ROOT, TestEntry, TestMtl, TestMtlMaterial,
    entity_interior_entries, lit_interior_entries,
};

const SIZE: [f32; 2] = [1400.0, 900.0];

const TWO_ENGLISH: &str = "2 layer files of the material libraries are missing or unreadable; their materials keep the colours of their own file.";
const TWO_GERMAN: &str = "2 Layer-Dateien der Materialbibliotheken fehlen oder sind nicht lesbar; ihre Materialien behalten die Farben der eigenen Datei.";

/// Replaces the library `ship.mtl` of `entries` by one whose two layers name files the Archive
/// lacks.
fn lacking_two_layers(entries: Vec<TestEntry>) -> Vec<TestEntry> {
    let material = TestMtlMaterial::new("hull")
        .layer_with(&[("Name", "A"), ("Path", "materials/layers/lost_a.mtl")])
        .layer_with(&[("Name", "B"), ("Path", "materials/layers/lost_b.mtl")]);
    let mut kept: Vec<TestEntry> = entries
        .into_iter()
        .filter(|entry| !entry.path().to_ascii_lowercase().ends_with("ship.mtl"))
        .collect();
    kept.push(TestEntry::stored(
        "Data\\objects\\test\\ship.mtl",
        TestMtl::multi(&[material.clone(), material.clone(), material]).cryxml(),
    ));
    kept
}

fn window(
    entries: Vec<TestEntry>,
    language: Language,
) -> Result<Window, Box<dyn std::error::Error>> {
    opened_sized(lacking_two_layers(entries), language, SIZE)
}

#[test]
fn two_missing_layer_files_use_the_plural_text_in_both_languages() -> TestResult {
    for (language, text) in [
        (Language::BritishEnglish, TWO_ENGLISH),
        (Language::German, TWO_GERMAN),
    ] {
        let (dir, mut harness) = window(entries(), language)?;
        select(&mut harness, "TEST_Ship")?;
        harness.state_mut().begin_assembly_export();

        export(&mut harness, &dir.path().join("ship.glb"))?;

        assert_eq!(
            summary(&harness)?.missing_layers(),
            2,
            "the ship and its mount share the library"
        );
        assert!(harness.query_by_label(text).is_some(), "{text}");
    }
    Ok(())
}

#[test]
fn the_package_summary_notes_the_missing_layer_files() -> TestResult {
    let (dir, mut harness) = window(entries(), Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    harness.state_mut().begin_assembly_export();
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?;
    setup.set_form(AssemblyExportForm::Package);
    setup.set_package_folder(dir.path().join("package"));
    assert!(harness.state_mut().submit_assembly_export(|| {}), "started");
    package_finished(&mut harness)?;

    let done = summary(&harness)?;

    assert!(done.package().is_some(), "a package");
    assert_eq!(done.missing_layers(), 2);
    assert!(harness.query_by_label(TWO_ENGLISH).is_some());
    Ok(())
}

#[test]
fn the_interior_of_an_entity_adds_its_libraries() -> TestResult {
    let (dir, mut harness) = window(entity_interior_entries(), Language::BritishEnglish)?;
    select(&mut harness, ENTITY_INTERIOR_CLASS)?;
    harness.state_mut().begin_assembly_export();
    harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?
        .set_interior(true);

    export(&mut harness, &dir.path().join("hull.glb"))?;

    let done = summary(&harness)?;
    assert!(done.interior().is_some(), "with the Interior");
    assert_eq!(done.missing_layers(), 2);
    assert!(harness.query_by_label(TWO_ENGLISH).is_some());
    Ok(())
}

#[test]
fn a_container_on_its_own_counts_the_missing_layer_files() -> TestResult {
    let (dir, mut harness) = window(lit_interior_entries(), Language::BritishEnglish)?;
    harness.state_mut().set_work_tab(WorkTab::Archive);
    harness.state_mut().set_preview_tab(PreviewTab::Hex);
    harness.run_steps(2);
    select_socpak(&mut harness, INTERIOR_ROOT)?;
    act_until(
        &mut harness,
        "the click on the Interior export to open the setup",
        |harness| click_settled(harness, "Export Interior…"),
        |model| model.assembly_export_setup().is_some(),
    )?;

    export(&mut harness, &dir.path().join("ship_interior.glb"))?;

    assert_eq!(summary(&harness)?.missing_layers(), 2);
    assert!(harness.query_by_label(TWO_ENGLISH).is_some());
    Ok(())
}
