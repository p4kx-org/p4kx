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

fn clip_names(folder: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(folder.join("scene.json"))?)?;
    Ok(manifest
        .pointer("/meshes/0/rig/animations")
        .and_then(serde_json::Value::as_array)
        .map_or_default(|clips| {
            clips
                .iter()
                .filter_map(|clip| clip.get("name").and_then(serde_json::Value::as_str))
                .map(str::to_owned)
                .collect()
        }))
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
    let names = clip_names(&folder)?;
    assert!(
        names.iter().any(|name| name.contains("nod")),
        "the clip is in the manifest: {names:?}"
    );
    Ok(())
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
    assert_eq!(clip_names(&folder)?.len(), 0, "no clip in the manifest");
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
