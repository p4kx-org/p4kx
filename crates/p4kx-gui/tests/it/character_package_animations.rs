// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The animations of the Blender package of a Character: the choice in the setup, the clips read
//! from the animation folders inside the export job, and the Cancel.

use std::path::Path;

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{AssemblyExportForm, CharacterExportStatus, Language};
use p4kx_settings::AssemblyExportSettings;
use p4kx_testkit::{TestAnimation, TestEntry, TestSkeleton, TestTrack};
use serde_json::Value;

use crate::character_export_window::compose;
use crate::characters::{TestResult, Window, entries, opened_sized, wait_until};

const CLIP_FOLDER: &str = "Data\\Animations\\Characters\\Human\\male_v7";

fn clip_entries() -> Vec<TestEntry> {
    let nod = TestAnimation::new("nod.caf", 30).track(
        TestTrack::new(TestSkeleton::controller_id(2))
            .rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (10, [0.0, 0.0, 0.0, 1.0])]),
    );
    let mut entries = entries();
    entries.push(TestEntry::stored(
        format!("{CLIP_FOLDER}\\nod.caf"),
        nod.caf(),
    ));
    entries
}

fn composed(language: Language) -> Result<Window, Box<dyn std::error::Error>> {
    compose(opened_sized(clip_entries(), language, [1400.0, 900.0])?)
}

fn ended(window: &mut Window) -> TestResult {
    wait_until(&mut window.1, |model| {
        model
            .characters()
            .export()
            .is_some_and(|export| !export.is_running())
    })
}

fn start(window: &mut Window, folder: &Path, animations: bool) -> Result<bool, &'static str> {
    let model = &mut window.1.state_mut().model;
    if !model.begin_character_export() {
        return Err("the setup did not open");
    }
    let setup = model
        .characters_mut()
        .export_setup_mut()
        .ok_or("no setup")?;
    setup.set_form(AssemblyExportForm::Package);
    setup.set_animations(animations);
    setup.set_package_folder(folder.to_path_buf());
    Ok(model.submit_character_export(|| {}))
}

fn read_manifest(folder: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&std::fs::read(
        folder.join("scene.json"),
    )?)?)
}

fn clip_names(manifest: &Value) -> Vec<&str> {
    manifest
        .pointer("/meshes/0/rig/animations")
        .and_then(Value::as_array)
        .map_or_default(|clips| {
            clips
                .iter()
                .filter_map(|clip| clip.get("name").and_then(Value::as_str))
                .collect()
        })
}

fn assert_nod_origin(manifest: &Value) -> TestResult {
    assert_eq!(
        manifest.get("version").and_then(Value::as_str),
        Some("1.25"),
        "an Archive clip contributes addressed scene evidence"
    );
    let scene = manifest
        .get("scene_provenance")
        .ok_or("GUI package scene evidence")?;
    let origin = scene
        .get("fieldOrigins")
        .and_then(|fields| fields.get("/meshes/0/rig/animations/0/name"))
        .ok_or("the GUI clip name has an addressed origin")?;
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "the Archive clip name has a complete directory origin"
    );
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(false),
        "the fixture's root directory is unencrypted"
    );
    let references = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("clip name source references")?;
    let [reference] = references.as_slice() else {
        return Err("a root Archive clip name has one directory use".into());
    };
    assert_nod_directory_use(scene, reference)
}

fn assert_nod_directory_use(scene: &Value, reference: &Value) -> TestResult {
    let name_use = scene
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .and_then(|rows| rows.iter().find(|row| row.get("id") == Some(reference)))
        .ok_or("clip name use resolves in the same scene inventory")?;
    assert_eq!(
        name_use.get("scope").and_then(Value::as_str),
        Some("archiveDirectoryMetadata"),
        "the clip name consumes the observed directory entry"
    );
    assert_eq!(
        name_use.pointer("/location/field").and_then(Value::as_str),
        Some("path"),
        "the name is derived from the original path"
    );
    let source_id = name_use.get("source").ok_or("clip directory source id")?;
    let source = scene
        .pointer("/sourceInventory/sources")
        .and_then(Value::as_array)
        .and_then(|rows| rows.iter().find(|row| row.get("id") == Some(source_id)))
        .ok_or("clip source resolves in the same scene inventory")?;
    let expected_path = format!("{CLIP_FOLDER}\\nod.caf");
    assert_eq!(
        source.pointer("/path/value").and_then(Value::as_str),
        Some(expected_path.as_str()),
        "the GUI package binds the selected nod clip's physical CAF"
    );
    Ok(())
}

#[test]
fn all_is_the_default_and_the_package_carries_the_clips_of_the_animation_folders() -> TestResult {
    let mut window = composed(Language::BritishEnglish)?;
    let folder = window.0.path().join("runner");
    assert!(window.1.state_mut().model.begin_character_export());
    let setup = window
        .1
        .state()
        .model
        .characters()
        .export_setup()
        .ok_or("no setup")?;
    assert!(
        setup.animations(),
        "all of the animation folders by default"
    );
    window
        .1
        .state_mut()
        .model
        .characters_mut()
        .close_export_setup();

    assert!(start(&mut window, &folder, true)?, "started");
    ended(&mut window)?;

    let export = window
        .1
        .state()
        .model
        .characters()
        .export()
        .ok_or("no export")?;
    let CharacterExportStatus::Finished(summary) = export.status() else {
        return Err(format!("not finished: {export:?}").into());
    };
    let package = summary.package().ok_or("no package")?;
    assert_eq!(package.animations(), 1, "{package:?}");
    let manifest = read_manifest(&folder)?;
    let names = clip_names(&manifest);
    assert!(
        names.iter().any(|name| name.contains("nod")),
        "the clip is in the manifest: {names:?}"
    );
    assert_nod_origin(&manifest)
}

#[test]
fn none_writes_a_package_without_clips() -> TestResult {
    let mut window = composed(Language::BritishEnglish)?;
    let folder = window.0.path().join("runner");

    assert!(start(&mut window, &folder, false)?, "started");
    ended(&mut window)?;

    let export = window
        .1
        .state()
        .model
        .characters()
        .export()
        .ok_or("no export")?;
    let CharacterExportStatus::Finished(summary) = export.status() else {
        return Err(format!("not finished: {export:?}").into());
    };
    assert_eq!(summary.package().ok_or("no package")?.animations(), 0);
    let manifest = read_manifest(&folder)?;
    assert_eq!(clip_names(&manifest).len(), 0, "no clip in the manifest");
    Ok(())
}

#[test]
fn the_setup_offers_the_choice_in_both_languages_and_the_choice_is_remembered() -> TestResult {
    for (language, heading, none) in [
        (Language::BritishEnglish, "Animations", "None"),
        (Language::German, "Animationen", "Keine"),
    ] {
        let mut window = composed(language)?;
        assert!(window.1.state_mut().model.begin_character_export());
        window.1.run_steps(3);
        assert!(
            window.1.query_by_label(heading).is_none(),
            "only with the package"
        );
        window
            .1
            .state_mut()
            .model
            .characters_mut()
            .export_setup_mut()
            .ok_or("no setup")?
            .set_form(AssemblyExportForm::Package);
        window.1.run_steps(3);
        assert!(window.1.query_by_label(heading).is_some(), "{language:?}");

        window.1.get_by_role_and_label(Role::Button, none).click();
        window.1.run_steps(3);
        let setup = window
            .1
            .state()
            .model
            .characters()
            .export_setup()
            .ok_or("no setup")?;
        assert!(!setup.animations(), "none chosen");
    }
    Ok(())
}

#[test]
fn the_setup_starts_on_the_glb_file_whatever_the_entity_export_chose() -> TestResult {
    let mut window = crate::characters::opened(Language::BritishEnglish)?;
    window
        .1
        .state_mut()
        .model
        .set_assembly_export_settings(AssemblyExportSettings {
            package: true,
            ..AssemblyExportSettings::default()
        });
    let mut window = compose(window)?;

    assert!(window.1.state_mut().model.begin_character_export());

    let setup = window
        .1
        .state()
        .model
        .characters()
        .export_setup()
        .ok_or("no setup")?;
    assert_eq!(setup.form(), AssemblyExportForm::Glb);
    Ok(())
}

#[test]
fn a_cancel_before_the_clips_are_read_writes_no_package() -> TestResult {
    let mut window = composed(Language::BritishEnglish)?;
    let folder = window.0.path().join("cancelled");
    let hold = window.1.state_mut().model.hold_character_export();

    assert!(start(&mut window, &folder, true)?, "started");
    window.1.state().model.characters().cancel_export();
    drop(hold);
    ended(&mut window)?;

    let export = window
        .1
        .state()
        .model
        .characters()
        .export()
        .ok_or("no export")?;
    assert!(
        matches!(export.status(), CharacterExportStatus::Cancelled),
        "cancelled, not {export:?}"
    );
    assert!(!folder.exists(), "no folder");
    Ok(())
}

#[test]
fn the_export_waits_while_the_character_is_built_again_and_says_why() -> TestResult {
    let mut window = composed(Language::BritishEnglish)?;
    assert!(window.1.state_mut().model.begin_character_export());
    window.1.state_mut().model.characters_mut().set_lod(1);
    window.1.run_steps(2);

    let busy = window
        .1
        .query_by_label_contains("is being built again")
        .is_some();
    let building = window.1.state().model.characters().is_building();
    assert!(
        !building || busy,
        "while it is built again the dialog says why"
    );
    wait_until(&mut window.1, crate::characters::built)?;
    window.1.run_steps(3);
    assert!(
        window
            .1
            .query_by_label_contains("is being built again")
            .is_none(),
        "the notice is gone once it is built"
    );
    Ok(())
}
