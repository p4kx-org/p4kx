// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The export of the Character of the Characters tab as a Blender package: the choice in the
//! setup, the folder checks, the report and the Cancel.

use std::path::Path;

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use p4kx_gui::ui::Dialog;
use p4kx_gui::{AssemblyExportForm, CharacterExportStatus, Language, PackageProblem};
use p4kx_testkit::check_glb;

use crate::character_export_window::compose;
use crate::characters::{TestResult, Window, opened, opened_in_installation, wait_until};
use crate::labels::labels_starting_with;

fn composed() -> Result<Window, Box<dyn std::error::Error>> {
    compose(opened(Language::BritishEnglish)?)
}

fn ended(window: &mut Window) -> TestResult {
    wait_until(&mut window.1, |model| {
        model
            .characters()
            .export()
            .is_some_and(|export| !export.is_running())
    })
}

fn open_setup(window: &mut Window) -> TestResult {
    if !window.1.state_mut().model.begin_character_export() {
        return Err("the setup did not open".into());
    }
    window.1.run_steps(3);
    Ok(())
}

fn start(window: &mut Window, folder: &Path) -> Result<bool, &'static str> {
    let model = &mut window.1.state_mut().model;
    if !model.begin_character_export() {
        return Err("the setup did not open");
    }
    let setup = model
        .characters_mut()
        .export_setup_mut()
        .ok_or("no setup")?;
    setup.set_form(AssemblyExportForm::Package);
    setup.set_package_folder(folder.to_path_buf());
    Ok(model.submit_character_export(|| {}))
}

fn manifest(folder: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&std::fs::read(
        folder.join("scene.json"),
    )?)?)
}

#[test]
fn the_setup_offers_the_package_next_to_the_glb_file() -> TestResult {
    let mut window = composed()?;
    open_setup(&mut window)?;
    let setup = window
        .1
        .state()
        .model
        .characters()
        .export_setup()
        .ok_or("no setup")?;
    assert_eq!(setup.form(), AssemblyExportForm::Glb, "a GLB file first");
    assert!(window.1.query_by_label("GLB file").is_some());
    assert!(window.1.query_by_label("Package folder").is_none());

    window
        .1
        .get_by_role_and_label(Role::Button, "Blender package")
        .click();
    window.1.run_steps(3);

    let setup = window
        .1
        .state()
        .model
        .characters()
        .export_setup()
        .ok_or("no setup")?;
    assert_eq!(setup.form(), AssemblyExportForm::Package);
    assert!(!setup.is_ready(), "no folder chosen yet");
    assert!(window.1.query_by_label("Package folder").is_some());
    assert!(
        window
            .1
            .query_by_label_contains("and the animations if you choose them")
            .is_some(),
        "the note says what the package holds"
    );

    window
        .1
        .get_by_role_and_label(Role::Button, "Choose folder…")
        .click();
    window.1.run_steps(2);
    assert_eq!(
        window.1.state().dialog,
        Some(Dialog::CharacterPackage),
        "the host is asked for a folder"
    );
    Ok(())
}

#[test]
fn the_german_setup_names_the_package() -> TestResult {
    let mut window = compose(opened(Language::German)?)?;
    open_setup(&mut window)?;

    window
        .1
        .get_by_role_and_label(Role::Button, "Blender-Paket")
        .click();
    window.1.run_steps(3);

    assert!(window.1.query_by_label("Paketordner").is_some());
    assert!(
        window
            .1
            .query_by_role_and_label(Role::Button, "Ordner wählen …")
            .is_some()
    );
    assert!(
        window
            .1
            .query_by_label_contains("und die Animationen, wenn du sie wählst")
            .is_some()
    );
    Ok(())
}

#[test]
fn the_package_holds_the_skinned_character_and_is_reported() -> TestResult {
    let mut window = composed()?;
    let folder = window.0.path().join("runner");

    assert!(start(&mut window, &folder)?, "started");
    window.1.run_steps(1);
    ended(&mut window)?;

    let model = &window.1.state().model;
    let export = model.characters().export().ok_or("no export")?;
    assert_eq!(export.form(), AssemblyExportForm::Package);
    let CharacterExportStatus::Finished(summary) = export.status() else {
        return Err(format!("not finished: {export:?}").into());
    };
    let package = summary.package().ok_or("no package in the summary")?;
    assert_eq!(summary.file(), folder);
    assert_eq!(package.bones, 3, "the male skeleton");
    assert!(package.morph_targets >= 2, "{package:?}");
    assert_eq!(package.files, 2, "the manifest and the Character");

    let manifest = manifest(&folder)?;
    assert_eq!(manifest["format"], "p4kx-blender-package");
    assert_eq!(manifest["nodes"][0]["kind"], "character");
    assert_eq!(manifest["source"]["character"]["chf"], "Runner.chf");
    assert_eq!(
        manifest["source"]["character"]["name"],
        manifest["nodes"][0]["name"]
    );
    let mesh = folder.join("meshes").join("m0001.glb");
    let checked = check_glb(&std::fs::read(&mesh)?)?;
    assert_eq!(checked.document.skins().count(), 1, "one skin");

    window.1.run_steps(3);
    assert!(
        window.1.query_by_label("Blender package written").is_some(),
        "the summary is the package's"
    );
    assert!(
        labels_starting_with(&window.1, "Parts ") == 1,
        "and it counts what the package holds"
    );
    Ok(())
}

#[test]
fn the_choice_and_the_folder_are_remembered_for_the_next_export() -> TestResult {
    let mut window = composed()?;
    let folder = window.0.path().join("runner");
    assert!(start(&mut window, &folder)?);
    ended(&mut window)?;

    assert!(window.1.state_mut().model.begin_character_export());

    let setup = window
        .1
        .state()
        .model
        .characters()
        .export_setup()
        .ok_or("no setup")?;
    assert_eq!(
        setup.form(),
        AssemblyExportForm::Package,
        "the next one too"
    );
    assert_eq!(setup.folder(), Some(window.0.path()));
    Ok(())
}

#[test]
fn an_earlier_package_is_replaced_and_anything_else_is_left_alone() -> TestResult {
    let mut window = composed()?;
    let folder = window.0.path().join("runner");
    assert!(start(&mut window, &folder)?);
    ended(&mut window)?;
    let first = std::fs::read(folder.join("scene.json"))?;
    std::fs::write(folder.join("meshes").join("m9999.glb"), b"old")?;

    open_setup(&mut window)?;
    let setup = window
        .1
        .state_mut()
        .model
        .characters_mut()
        .export_setup_mut()
        .ok_or("no setup")?;
    setup.set_package_folder(folder.clone());
    assert!(setup.package_replaces(), "the earlier package is named");
    window.1.run_steps(3);
    assert!(
        window
            .1
            .query_by_label_contains("holds an earlier package")
            .is_some(),
        "the dialog says so"
    );
    assert!(window.1.state_mut().model.submit_character_export(|| {}));
    ended(&mut window)?;
    assert_eq!(std::fs::read(folder.join("scene.json"))?, first);
    assert!(
        !folder.join("meshes").join("m9999.glb").exists(),
        "replaced as a whole"
    );

    let other = window.0.path().join("documents");
    std::fs::create_dir(&other)?;
    std::fs::write(other.join("notes.txt"), b"mine")?;
    open_setup(&mut window)?;
    let setup = window
        .1
        .state_mut()
        .model
        .characters_mut()
        .export_setup_mut()
        .ok_or("no setup")?;
    setup.set_package_folder(other.clone());
    assert!(matches!(
        setup.package_problem(),
        Some(PackageProblem::NotAPackage)
    ));
    assert!(!setup.is_ready());
    window.1.run_steps(3);
    assert!(
        window
            .1
            .query_by_label_contains("holds no package written by P4KX")
            .is_some()
    );
    assert!(!window.1.state_mut().model.submit_character_export(|| {}));
    assert_eq!(std::fs::read(other.join("notes.txt"))?, b"mine");
    Ok(())
}

#[test]
fn a_folder_inside_an_installation_is_refused() -> TestResult {
    let (window, channel) = opened_in_installation(Language::BritishEnglish)?;
    let mut window = compose(window)?;
    let folder = channel.join("runner");

    assert!(
        !start(&mut window, &folder)?,
        "the setup is not ready, so nothing starts"
    );

    let setup = window
        .1
        .state()
        .model
        .characters()
        .export_setup()
        .ok_or("the setup stays open")?;
    assert!(setup.refusal().is_some(), "named in the setup");
    window.1.run_steps(3);
    assert!(
        window
            .1
            .query_by_label_contains("Star Citizen installation")
            .is_some()
    );
    assert!(!folder.exists(), "nothing written");
    Ok(())
}

#[test]
fn a_stopped_export_keeps_the_earlier_package_and_leaves_nothing_behind() -> TestResult {
    let mut window = composed()?;
    let folder = window.0.path().join("runner");
    assert!(start(&mut window, &folder)?);
    ended(&mut window)?;
    let first = std::fs::read(folder.join("scene.json"))?;

    let fresh = window.0.path().join("fresh");
    assert!(start(&mut window, &fresh)?);
    window.1.state_mut().model.characters_mut().stop_export();

    let export = window
        .1
        .state()
        .model
        .characters()
        .export()
        .ok_or("no export")?;
    match export.status() {
        CharacterExportStatus::Cancelled => assert!(!fresh.exists(), "no folder"),
        CharacterExportStatus::Finished(_) => assert!(fresh.exists(), "quicker than the stop"),
        other => return Err(format!("unexpected: {other:?}").into()),
    }
    assert_eq!(std::fs::read(folder.join("scene.json"))?, first);
    let leftovers = std::fs::read_dir(window.0.path())?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".p4kx-"))
        .count();
    assert_eq!(leftovers, 0, "no temporary folder");
    Ok(())
}
