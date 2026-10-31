// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Export of an Entity as GLB, driven headless over the synthetic Archive of the Entities
//! tab.

use std::path::Path;

use crate::entity_picking;
use crate::waiting::wait_for;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use entity_picking::{TestResult, opened, opened_sized, select, settle};
use p4kx_gui::{AssemblyExport, AssemblyExportStatus, Language, Model, Place, WorkTab};
use p4kx_testkit::layered_entries;

fn start(harness: &mut Harness<'_, Model>, file: &Path) -> Result<bool, &'static str> {
    harness.state_mut().begin_assembly_export();
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("the setup did not open")?;
    setup.set_file(file.to_path_buf());
    Ok(harness.state_mut().submit_assembly_export(|| {}))
}

fn finished(harness: &mut Harness<'_, Model>) -> TestResult {
    wait_for(harness, "the export to end", |harness| {
        !harness
            .state()
            .assembly_export()
            .is_some_and(AssemblyExport::is_running)
    })?;
    harness.run_steps(3);
    Ok(())
}

fn summary(harness: &Harness<'_, Model>) -> Result<p4kx_gui::AssemblyExportSummary, &'static str> {
    match harness
        .state()
        .assembly_export()
        .map(AssemblyExport::status)
    {
        Some(AssemblyExportStatus::Finished(summary)) => Ok(summary.clone()),
        _ => Err("the export did not finish"),
    }
}

#[test]
fn the_export_waits_for_a_resolved_loadout() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    assert!(
        !harness.state().assembly_export_available(),
        "nothing selected"
    );
    harness.state_mut().begin_assembly_export();
    assert!(harness.state().assembly_export_setup().is_none());

    select(&mut harness, "TEST_Ship")?;

    assert!(harness.state().assembly_export_available());
    Ok(())
}

#[test]
fn the_export_writes_the_entity_with_its_loadout_as_one_glb_and_remembers_the_choices() -> TestResult
{
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    harness
        .get_by_role_and_label(Role::Button, "Export…")
        .click();
    harness.run_steps(3);
    assert!(
        harness.query_by_label("Entity: TEST_Ship").is_some(),
        "the setup names the Entity"
    );
    harness.get_by_role_and_label(Role::Button, "LOD 1").click();
    harness.run_steps(2);
    let file = dir.path().join("ship.glb");

    harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("the setup is open")?
        .set_file(file.clone());
    assert!(
        harness.state_mut().submit_assembly_export(|| {}),
        "the export starts"
    );
    finished(&mut harness)?;

    let summary = summary(&harness)?;
    assert_eq!(summary.parts(), 3, "the ship, the gun and the relay");
    assert_eq!(summary.modelled_parts(), 3);
    assert_eq!(summary.lod(), 1);
    assert_eq!((summary.excluded(), summary.replaced()), (0, 0));
    let bytes = std::fs::read(&file)?;
    assert_eq!(bytes.get(..4), Some(&b"glTF"[..]), "a GLB file");
    assert_eq!(
        u64::try_from(bytes.len())?,
        summary.written().bytes,
        "the summary counts the file"
    );
    assert!(harness.query_by_label("Entity exported").is_some());
    let remembered = harness.state().assembly_export_settings();
    assert_eq!(remembered.lod, 1, "remembered");
    assert_eq!(remembered.folder.as_deref(), Some(dir.path()));
    Ok(())
}

#[test]
fn the_changes_of_the_loadout_apply_to_the_file() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    harness
        .get_by_role_and_label(Role::CheckBox, "Include hardpoint_relay")
        .click();
    settle(&mut harness)?;

    assert!(start(&mut harness, &dir.path().join("ship.glb"))?);
    finished(&mut harness)?;

    let summary = summary(&harness)?;
    assert_eq!(summary.parts(), 2, "the relay is left out");
    assert_eq!(summary.excluded(), 1);
    Ok(())
}

#[test]
fn a_replacement_that_does_not_fit_is_written_and_noted_in_the_summary() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    harness
        .state_mut()
        .entities_mut()
        .loadout_mut()
        .ok_or("no session")?
        .set_replacement("hardpoint_weapon_nose", "TEST_Shield")?;
    settle(&mut harness)?;

    assert!(start(&mut harness, &dir.path().join("ship.glb"))?);
    finished(&mut harness)?;

    let summary = summary(&harness)?;
    assert_eq!(summary.replaced(), 1);
    assert_eq!(summary.mismatches().len(), 1, "{:?}", summary.mismatches());
    assert!(
        harness
            .query_all_by_label_contains("written all the same")
            .count()
            == 1,
        "the summary notes it"
    );
    Ok(())
}

#[test]
fn parts_that_cannot_be_resolved_are_listed_in_the_report() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Cart")?;

    assert!(start(&mut harness, &dir.path().join("cart.glb"))?);
    finished(&mut harness)?;

    let summary = summary(&harness)?;
    assert_eq!(summary.issues().len(), 1, "{:?}", summary.issues());
    assert!(!summary.issues().iter().any(|issue| issue.skipped));
    assert!(
        harness
            .query_by_label_contains("the Entity names no geometry")
            .is_some(),
        "the report shows it"
    );
    Ok(())
}

#[test]
fn a_cancelled_export_leaves_no_file() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    let file = dir.path().join("ship.glb");

    assert!(start(&mut harness, &file)?);
    harness.state().cancel_assembly_export();
    finished(&mut harness)?;

    let cancelled = matches!(
        harness
            .state()
            .assembly_export()
            .map(AssemblyExport::status),
        Some(AssemblyExportStatus::Cancelled)
    );
    assert_eq!(
        file.exists(),
        !cancelled,
        "a file only if it was not cancelled"
    );
    Ok(())
}

#[test]
fn the_export_reads_the_archive_even_while_a_container_is_open_in_browse() -> TestResult {
    let container = p4kx_testkit::TestContainer::new()
        .entry(p4kx_testkit::TestEntry::stored(
            "room\\layout.xml",
            "<layout/>",
        ))
        .build();
    let mut entries = entity_picking::entries();
    entries.push(p4kx_testkit::TestEntry::stored(
        "Data\\ObjectContainers\\room.socpak",
        container,
    ));
    let (dir, mut harness) = opened_sized(entries, Language::BritishEnglish, [1400.0, 900.0])?;
    select(&mut harness, "TEST_Ship")?;
    harness.state_mut().set_work_tab(WorkTab::Archive);
    harness.run_steps(2);
    for folder in ["Data", "ObjectContainers", "room.socpak"] {
        harness.get_by_role_and_label(Role::Button, folder).click();
        harness.run_steps(3);
    }
    wait_for(&mut harness, "the container to open", |harness| {
        harness
            .state()
            .browser()
            .is_some_and(|browser| matches!(browser.place(), Place::Container(_)))
    })?;
    harness.state_mut().set_work_tab(WorkTab::Entities);
    harness.run_steps(2);

    assert!(start(&mut harness, &dir.path().join("ship.glb"))?);
    finished(&mut harness)?;

    assert_eq!(
        summary(&harness)?.modelled_parts(),
        3,
        "the models come from the Archive, not from the container"
    );
    Ok(())
}

#[test]
fn a_textures_export_writes_the_images_of_the_materials() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = crate::tagging::archive(dir.path(), p4kx_testkit::livery_entries())?;
    let mut harness =
        crate::tagging::harness_sized(Model::new(Language::BritishEnglish), [1400.0, 900.0]);
    harness.state_mut().open(path, || {});
    crate::tagging::wait_until(&mut harness, |model| model.browser().is_some())?;
    harness.state_mut().set_work_tab(WorkTab::Entities);
    harness.run_steps(2);
    crate::tagging::wait_until(&mut harness, entity_picking::listed)?;
    select(&mut harness, "TEST_Livery")?;
    harness.state_mut().begin_assembly_export();
    harness.run_steps(2);
    harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("the setup is open")?
        .set_materials(p4kx_settings::ModelMaterials::Textures);
    harness.run_steps(3);
    harness.get_by_role_and_label(Role::Button, "1/2").click();
    harness.run_steps(2);
    let file = dir.path().join("livery.glb");
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("the setup is open")?;
    assert_eq!(setup.materials(), p4kx_settings::ModelMaterials::Textures);
    assert_eq!(setup.texture_mip(), 1, "the size follows the click");
    setup.set_file(file.clone());
    assert!(harness.state_mut().submit_assembly_export(|| {}));
    finished(&mut harness)?;

    let summary = summary(&harness)?;
    assert!(summary.written().textures > 0, "{:?}", summary.written());
    assert!(summary.written().textured_materials > 0);
    let bytes = std::fs::read(&file)?;
    p4kx_testkit::check_glb(&bytes)?;
    assert!(
        bytes.windows(9).any(|window| window == b"image/png"),
        "an embedded PNG image"
    );
    Ok(())
}

#[test]
fn a_file_inside_a_missing_folder_fails_with_the_reason() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;

    assert!(start(
        &mut harness,
        &dir.path().join("none").join("ship.glb")
    )?);
    finished(&mut harness)?;

    assert!(
        matches!(
            harness
                .state()
                .assembly_export()
                .map(AssemblyExport::status),
            Some(AssemblyExportStatus::Failed(_))
        ),
        "failed"
    );
    assert!(harness.query_by_label("Export failed").is_some());
    Ok(())
}

#[test]
fn the_summary_lists_the_layer_files_the_archive_lacks_in_both_languages() -> TestResult {
    for (language, note) in [
        (
            Language::BritishEnglish,
            "One layer file of the material libraries is missing or unreadable; its materials keep the colours of their own file.",
        ),
        (
            Language::German,
            "Eine Layer-Datei der Materialbibliotheken fehlt oder ist nicht lesbar; ihre Materialien behalten die Farben der eigenen Datei.",
        ),
    ] {
        let (dir, mut harness) = opened_sized(layered_entries(), language, [1400.0, 900.0])?;
        select(&mut harness, "TEST_Layered")?;

        assert!(
            start(&mut harness, &dir.path().join("layered.glb"))?,
            "started"
        );
        finished(&mut harness)?;

        assert_eq!(summary(&harness)?.missing_layers(), 1);
        assert!(harness.query_by_label(note).is_some(), "{note}");
    }
    Ok(())
}
