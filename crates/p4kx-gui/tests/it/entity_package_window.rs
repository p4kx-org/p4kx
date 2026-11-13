// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Export of an Entity as a Blender package, driven headless over the synthetic Archive of the
//! Entities tab: the choice in the dialog, the folder, the progress, the report and the Conflict
//! handling of an existing folder.

use std::path::Path;

use crate::entity_picking::{TestResult, opened, opened_sized, select};
use crate::labels::labels_starting_with;
use crate::waiting::wait_for;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{
    AssemblyExport, AssemblyExportForm, AssemblyExportStatus, AssemblyExportSummary, Language,
    Model, PackageProblem,
};
use p4kx_testkit::{ENTITY_INTERIOR_CLASS, entity_interior_entries};

#[path = "entity_package_window/failure.rs"]
mod failure;

fn start(harness: &mut Harness<'_, Model>, folder: &Path) -> Result<bool, &'static str> {
    harness.state_mut().begin_assembly_export();
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("the setup did not open")?;
    setup.set_form(AssemblyExportForm::Package);
    setup.set_package_folder(folder.to_path_buf());
    Ok(harness.state_mut().submit_assembly_export(|| {}))
}

pub(crate) fn finished(harness: &mut Harness<'_, Model>) -> TestResult {
    wait_for(harness, "the export to end", |harness| {
        !harness
            .state()
            .assembly_export()
            .is_some_and(AssemblyExport::is_running)
    })?;
    harness.run_steps(3);
    Ok(())
}

fn summary(harness: &Harness<'_, Model>) -> Result<AssemblyExportSummary, &'static str> {
    match harness
        .state()
        .assembly_export()
        .map(AssemblyExport::status)
    {
        Some(AssemblyExportStatus::Finished(summary)) => Ok(summary.clone()),
        _ => Err("the export did not finish"),
    }
}

pub(crate) fn manifest(folder: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&std::fs::read(
        folder.join("scene.json"),
    )?)?)
}

#[test]
fn the_dialog_offers_the_package_next_to_the_glb_file_for_an_entity() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    harness.state_mut().begin_assembly_export();
    harness.run_steps(3);
    let setup = harness.state().assembly_export_setup().ok_or("no setup")?;
    assert_eq!(setup.form(), AssemblyExportForm::Glb, "a GLB file first");
    assert!(harness.query_by_label("GLB file").is_some());
    assert!(
        harness
            .query_by_label("Plain colours, a small and quick file")
            .is_some(),
        "the materials of a GLB file"
    );

    harness
        .get_by_role_and_label(Role::Button, "Blender package")
        .click();
    harness.run_steps(3);

    let setup = harness.state().assembly_export_setup().ok_or("no setup")?;
    assert_eq!(setup.form(), AssemblyExportForm::Package);
    assert!(!setup.is_ready(), "no folder chosen yet");
    assert!(harness.query_by_label("Package folder").is_some());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Choose folder…")
            .is_some()
    );
    assert!(
        harness
            .query_by_label("Plain colours, a small and quick file")
            .is_none(),
        "a package always carries the textures"
    );
    assert!(
        harness.query_by_label("Texture size").is_some(),
        "the size of the textures stays"
    );
    Ok(())
}

#[test]
fn the_german_dialog_names_the_package() -> TestResult {
    let (_dir, mut harness) = opened(Language::German)?;
    select(&mut harness, "TEST_Ship")?;
    harness.state_mut().begin_assembly_export();
    harness.run_steps(3);

    harness
        .get_by_role_and_label(Role::Button, "Blender-Paket")
        .click();
    harness.run_steps(3);

    assert!(harness.query_by_label("Paketordner").is_some());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Ordner wählen …")
            .is_some()
    );
    Ok(())
}

#[test]
fn the_export_writes_the_package_folder_reports_it_and_remembers_the_choices() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    let folder = dir.path().join("ship");

    assert!(start(&mut harness, &folder)?);
    finished(&mut harness)?;

    let summary = summary(&harness)?;
    let package = summary.package().ok_or("no package in the summary")?;
    assert_eq!(summary.parts(), 3, "the ship, the gun and the relay");
    assert_eq!(package.files, package.meshes + package.textures + 2);
    assert!(package.meshes > 0, "{package:?}");
    assert_eq!(manifest(&folder)?["format"], "p4kx-blender-package");
    assert!(folder.join("meshes").is_dir());
    assert!(folder.join("textures").is_dir());
    assert!(
        harness.query_by_label("Blender package written").is_some(),
        "the summary is the package's"
    );
    assert!(
        labels_starting_with(&harness, "Parts 3 (3 with a model), nodes ") == 1,
        "and it counts what the package holds"
    );
    let remembered = harness.state().assembly_export_settings();
    assert!(remembered.package, "remembered");
    assert_eq!(remembered.folder.as_deref(), Some(dir.path()));

    harness.state_mut().begin_assembly_export();
    let setup = harness.state().assembly_export_setup().ok_or("no setup")?;
    assert_eq!(
        setup.form(),
        AssemblyExportForm::Package,
        "the next one too"
    );
    Ok(())
}

#[test]
fn a_package_export_keeps_the_stored_texture_choice() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    harness
        .state_mut()
        .set_assembly_export_settings(p4kx_settings::AssemblyExportSettings {
            materials: p4kx_settings::ModelMaterials::Textures,
            ..p4kx_settings::AssemblyExportSettings::default()
        });
    select(&mut harness, "TEST_Ship")?;

    assert!(start(&mut harness, &dir.path().join("ship"))?);
    finished(&mut harness)?;

    assert_eq!(
        harness.state().assembly_export_settings().materials,
        p4kx_settings::ModelMaterials::Textures,
        "the package does not offer the choice, so it is not overwritten"
    );
    Ok(())
}

#[test]
fn the_changes_of_the_loadout_reach_the_package() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    harness
        .get_by_role_and_label(Role::CheckBox, "Include hardpoint_relay")
        .click();
    crate::entity_picking::settle(&mut harness)?;
    let folder = dir.path().join("ship");

    assert!(start(&mut harness, &folder)?);
    finished(&mut harness)?;

    assert_eq!(summary(&harness)?.parts(), 2, "the relay is left out");
    assert_eq!(
        manifest(&folder)?["options"]["exclude"][0],
        "hardpoint_relay"
    );
    Ok(())
}

#[test]
fn the_interior_goes_into_the_package_when_chosen() -> TestResult {
    let (dir, mut harness) = opened_sized(
        entity_interior_entries(),
        Language::BritishEnglish,
        [1400.0, 900.0],
    )?;
    select(&mut harness, ENTITY_INTERIOR_CLASS)?;
    harness.state_mut().begin_assembly_export();
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?;
    setup.set_interior(true);
    setup.set_form(AssemblyExportForm::Package);
    let folder = dir.path().join("hull");
    setup.set_package_folder(folder);
    assert!(harness.state_mut().submit_assembly_export(|| {}));
    finished(&mut harness)?;

    let summary = summary(&harness)?;
    let package = summary.package().ok_or("no package")?;
    assert!(package.containers >= 2, "{package:?}");
    assert!(summary.interior().is_some());
    assert!(
        harness
            .query_by_label_contains("Interior: object containers")
            .is_some()
    );
    Ok(())
}

#[test]
fn an_earlier_package_is_replaced_and_anything_else_is_left_alone() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    let folder = dir.path().join("ship");
    assert!(start(&mut harness, &folder)?);
    finished(&mut harness)?;
    let first = std::fs::read(folder.join("scene.json"))?;
    std::fs::write(folder.join("meshes").join("m9999.glb"), b"old")?;

    harness.state_mut().begin_assembly_export();
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?;
    setup.set_form(AssemblyExportForm::Package);
    setup.set_package_folder(folder.clone());
    assert!(setup.package_replaces(), "the earlier package is named");
    assert!(setup.is_ready());
    harness.run_steps(3);
    assert!(
        harness
            .query_by_label_contains("holds an earlier package")
            .is_some(),
        "the dialog says so"
    );
    assert!(harness.state_mut().submit_assembly_export(|| {}));
    finished(&mut harness)?;

    assert_eq!(std::fs::read(folder.join("scene.json"))?, first);
    assert!(
        !folder.join("meshes").join("m9999.glb").exists(),
        "replaced as a whole"
    );

    let other = dir.path().join("documents");
    std::fs::create_dir(&other)?;
    std::fs::write(other.join("notes.txt"), b"mine")?;
    harness.state_mut().begin_assembly_export();
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?;
    setup.set_form(AssemblyExportForm::Package);
    setup.set_package_folder(other.clone());
    assert!(matches!(
        setup.package_problem(),
        Some(PackageProblem::NotAPackage)
    ));
    assert!(!setup.is_ready());
    harness.run_steps(3);
    assert!(
        harness
            .query_by_label_contains("holds no package written by P4KX")
            .is_some()
    );
    assert!(!harness.state_mut().submit_assembly_export(|| {}));
    assert_eq!(std::fs::read(other.join("notes.txt"))?, b"mine");
    Ok(())
}

#[test]
fn a_cancelled_export_keeps_the_earlier_package_and_writes_no_folder() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    let folder = dir.path().join("ship");
    assert!(start(&mut harness, &folder)?);
    finished(&mut harness)?;
    let first = std::fs::read(folder.join("scene.json"))?;

    let fresh = dir.path().join("fresh");
    assert!(start(&mut harness, &fresh)?);
    harness.state().cancel_assembly_export();
    finished(&mut harness)?;
    let cancelled = matches!(
        harness
            .state()
            .assembly_export()
            .map(AssemblyExport::status),
        Some(AssemblyExportStatus::Cancelled)
    );
    assert!(
        cancelled,
        "the cancel came before the job reached the writer"
    );
    assert!(!fresh.exists(), "no folder");

    harness
        .state_mut()
        .set_assembly_export_settings(p4kx_settings::AssemblyExportSettings {
            lod: 3,
            ..p4kx_settings::AssemblyExportSettings::default()
        });
    assert!(start(&mut harness, &folder)?);
    harness.state().cancel_assembly_export();
    finished(&mut harness)?;
    assert!(matches!(
        harness
            .state()
            .assembly_export()
            .map(AssemblyExport::status),
        Some(AssemblyExportStatus::Cancelled)
    ));
    assert_eq!(
        std::fs::read(folder.join("scene.json"))?,
        first,
        "the earlier package stays"
    );
    let leftovers = std::fs::read_dir(dir.path())?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".p4kx-"))
        .count();
    assert_eq!(leftovers, 0, "no temporary folder");
    Ok(())
}
