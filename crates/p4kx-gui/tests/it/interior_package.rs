// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Export of an Interior container as a Blender package, driven headless over the synthetic
//! Archive of the containers: the choice in the dialog, the folder checks, the progress, the
//! report and the Cancel.

use std::path::Path;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{
    AssemblyExport, AssemblyExportForm, AssemblyExportStage, AssemblyExportStatus,
    AssemblyExportSummary, Language, Model, PackageProblem,
};
use p4kx_testkit::INTERIOR_ROOT;

use crate::entity_picking::TestResult;
use crate::interior_export::{container_window, container_window_in, finished, select_socpak};
use crate::waiting::wait_for_model;

fn begin(harness: &mut Harness<'_, Model>) -> TestResult {
    select_socpak(harness, INTERIOR_ROOT)?;
    harness.state_mut().begin_interior_export();
    harness.run_steps(3);
    if harness.state().assembly_export_setup().is_none() {
        return Err("the setup did not open".into());
    }
    Ok(())
}

fn export_to(harness: &mut Harness<'_, Model>, folder: &Path) -> TestResult {
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?;
    setup.set_form(AssemblyExportForm::Package);
    setup.set_package_folder(folder.to_path_buf());
    if !harness.state_mut().submit_assembly_export(|| {}) {
        return Err("the export did not start".into());
    }
    finished(harness)
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

fn manifest(folder: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&std::fs::read(
        folder.join("scene.json"),
    )?)?)
}

#[test]
fn the_dialog_offers_the_package_next_to_the_glb_file_for_a_container() -> TestResult {
    let (_dir, mut harness) = container_window()?;
    begin(&mut harness)?;
    let setup = harness.state().assembly_export_setup().ok_or("no setup")?;
    assert!(setup.is_container());
    assert_eq!(setup.form(), AssemblyExportForm::Glb, "a GLB file first");
    assert!(harness.query_by_label("GLB file").is_some());

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
    Ok(())
}

#[test]
fn the_german_dialog_of_a_container_names_the_package() -> TestResult {
    let (_dir, mut harness) = container_window_in(Language::German)?;
    begin(&mut harness)?;

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
fn the_package_holds_the_containers_models_and_lights_as_nodes() -> TestResult {
    let (dir, mut harness) = container_window()?;
    begin(&mut harness)?;
    let folder = dir.path().join("ship_interior");

    export_to(&mut harness, &folder)?;

    let summary = summary(&harness)?;
    let package = summary.package().ok_or("no package in the summary")?;
    let interior = summary.interior().ok_or("no Interior in the summary")?;
    assert!(!interior.of_entity, "a container on its own");
    assert!(interior.containers >= 2, "{interior:?}");
    assert!(package.containers >= 2, "{package:?}");
    assert!(package.lights >= 1, "{package:?}");
    assert!(package.meshes > 0, "{package:?}");
    let manifest = manifest(&folder)?;
    assert_eq!(manifest["format"], "p4kx-blender-package");
    assert_eq!(manifest["source"]["entity"]["class_name"], "ship_interior");
    let kinds: Vec<&str> = manifest["nodes"]
        .as_array()
        .ok_or("no nodes")?
        .iter()
        .filter_map(|node| node["kind"].as_str())
        .collect();
    for kind in ["container", "object", "light"] {
        assert!(kinds.contains(&kind), "no {kind} node in {kinds:?}");
    }
    assert!(
        harness.query_by_label("Blender package written").is_some(),
        "the summary is the package's"
    );
    assert!(
        harness
            .query_by_label_contains("Interior: containers opened")
            .is_some(),
        "and it counts the containers opened"
    );
    let remembered = harness.state().assembly_export_settings();
    assert!(remembered.package, "remembered");
    assert_eq!(remembered.folder.as_deref(), Some(dir.path()));
    Ok(())
}

#[test]
fn an_earlier_package_is_replaced_and_anything_else_is_left_alone() -> TestResult {
    let (dir, mut harness) = container_window()?;
    begin(&mut harness)?;
    let folder = dir.path().join("ship_interior");
    export_to(&mut harness, &folder)?;
    let first = std::fs::read(folder.join("scene.json"))?;
    std::fs::write(folder.join("meshes").join("m9999.glb"), b"old")?;

    harness.state_mut().begin_interior_export();
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?;
    assert_eq!(
        setup.form(),
        AssemblyExportForm::Package,
        "the choice is remembered"
    );
    setup.set_package_folder(folder.clone());
    assert!(setup.package_replaces(), "the earlier package is named");
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
    harness.state_mut().begin_interior_export();
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?;
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
fn a_cancelled_package_export_writes_no_folder() -> TestResult {
    let (dir, mut harness) = container_window()?;
    begin(&mut harness)?;
    let folder = dir.path().join("cancelled");
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?;
    setup.set_form(AssemblyExportForm::Package);
    setup.set_package_folder(folder.clone());
    let hold = harness.state_mut().hold_interior_export();
    assert!(harness.state_mut().submit_assembly_export(|| {}));
    wait_for_model(&mut harness, "the export to reach the Interior", |model| {
        matches!(
            model.assembly_export().map(AssemblyExport::status),
            Some(AssemblyExportStatus::Running(progress))
                if progress.stage == AssemblyExportStage::Interior
        )
    })?;
    harness.run_steps(3);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Cancel")
            .is_some(),
        "the progress window offers Cancel"
    );

    harness.state().cancel_assembly_export();
    finished(&mut harness)?;
    drop(hold);

    let status = harness
        .state()
        .assembly_export()
        .map(AssemblyExport::status)
        .ok_or("no export")?;
    assert!(
        matches!(status, AssemblyExportStatus::Cancelled),
        "cancelled, not {status:?}"
    );
    assert!(!folder.exists(), "no folder");
    Ok(())
}
