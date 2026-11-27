// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Interior in the Export as GLB: the toggle of the Entity export, remembered with the other
//! choices.

use std::path::Path;

use crate::entity_picking::{TestResult, Window, opened, opened_sized, select};
use crate::interior_socpak_export::{assert_geometry_origin, read_glb_origin};
use crate::waiting::wait_for_model;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{
    AssemblyExport, AssemblyExportStatus, AssemblyExportSummary, Language, Model, PreviewTab,
    WorkTab,
};
use p4kx_testkit::{
    ENTITY_INTERIOR_CLASS, check_glb, entity_interior_entries, lit_interior_entries,
};

pub(crate) const TOGGLE: &str = "Include the rooms and props of the object containers";

fn hull_window(language: Language) -> Result<Window, Box<dyn std::error::Error>> {
    opened_sized(entity_interior_entries(), language, [1400.0, 900.0])
}

pub(crate) fn container_window() -> Result<Window, Box<dyn std::error::Error>> {
    container_window_in(Language::BritishEnglish)
}

pub(crate) fn container_window_in(
    language: Language,
) -> Result<Window, Box<dyn std::error::Error>> {
    let (dir, mut harness) = opened_sized(lit_interior_entries(), language, [1400.0, 900.0])?;
    harness.state_mut().set_work_tab(WorkTab::Archive);
    harness.state_mut().set_preview_tab(PreviewTab::Hex);
    harness.run_steps(2);
    Ok((dir, harness))
}

/// Clicks the button `label` once its place no longer moves from one frame to the next.
pub(crate) fn click_settled(harness: &mut Harness<'_, Model>, label: &str) {
    let mut rect = harness.get_by_role_and_label(Role::Button, label).rect();
    for _ in 0..100 {
        harness.step();
        let now = harness.get_by_role_and_label(Role::Button, label).rect();
        if now == rect {
            break;
        }
        rect = now;
    }
    harness.get_by_role_and_label(Role::Button, label).click();
}

pub(crate) fn finished(harness: &mut Harness<'_, Model>) -> TestResult {
    wait_for_model(harness, "the export to end", |model| {
        !model
            .assembly_export()
            .is_some_and(AssemblyExport::is_running)
    })?;
    harness.run_steps(3);
    Ok(())
}

pub(crate) fn summary(harness: &Harness<'_, Model>) -> Result<AssemblyExportSummary, &'static str> {
    match harness
        .state()
        .assembly_export()
        .map(AssemblyExport::status)
    {
        Some(AssemblyExportStatus::Finished(summary)) => Ok(summary.clone()),
        _ => Err("the export did not finish"),
    }
}

pub(crate) fn export(harness: &mut Harness<'_, Model>, file: &Path) -> TestResult {
    harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("the setup is open")?
        .set_file(file.to_path_buf());
    if !harness.state_mut().submit_assembly_export(|| {}) {
        return Err("the export did not start".into());
    }
    finished(harness)
}

pub(crate) fn has_node(bytes: &[u8], name: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let glb = check_glb(bytes)?;
    Ok(glb.document.nodes().any(|node| node.name() == Some(name)))
}

pub(crate) fn select_socpak(harness: &mut Harness<'_, Model>, path: &str) -> TestResult {
    let browser = harness.state_mut().browser_mut().ok_or("no Archive")?;
    let id = browser
        .archive_index()
        .id_of_path(path)
        .ok_or("the container is no Entry")?;
    browser.select_entry(id, || {});
    harness.run_steps(6);
    Ok(())
}

#[test]
fn the_entity_export_offers_the_interior_and_writes_it_below_an_interior_node() -> TestResult {
    let (dir, mut harness) = hull_window(Language::BritishEnglish)?;
    select(&mut harness, ENTITY_INTERIOR_CLASS)?;
    harness.state_mut().begin_assembly_export();
    harness.run_steps(3);
    let setup = harness.state().assembly_export_setup().ok_or("no setup")?;
    assert!(setup.has_interior(), "the Entity names object containers");
    assert!(!setup.interior(), "the hull alone by default");
    assert!(!setup.is_container(), "an Entity");

    harness.get_by_label(TOGGLE).click();
    harness.run_steps(2);
    let setup = harness.state().assembly_export_setup().ok_or("no setup")?;
    assert!(setup.interior(), "switched on in the dialog");

    let file = dir.path().join("hull.glb");
    export(&mut harness, &file)?;

    let summary = summary(&harness)?;
    let interior = summary.interior().ok_or("no Interior in the summary")?;
    assert!(interior.containers >= 2, "{interior:?}");
    assert!(
        interior.parts > 0 && interior.modelled_parts > 0,
        "{interior:?}"
    );
    let bytes = std::fs::read(&file)?;
    assert!(has_node(&bytes, "Interior")?, "grouped below the hull");
    let rows = read_glb_origin(&summary, &bytes)?;
    assert_geometry_origin(&rows, "cockpit_seat.cgfm");
    assert!(
        harness.query_by_label("Entity exported").is_some(),
        "the summary is the Entity's"
    );
    assert!(
        harness
            .query_by_label_contains("Interior: object containers")
            .is_some(),
        "and it counts the Interior"
    );
    Ok(())
}

#[test]
fn the_interior_choice_is_remembered_with_the_other_choices() -> TestResult {
    let (dir, mut harness) = hull_window(Language::BritishEnglish)?;
    select(&mut harness, ENTITY_INTERIOR_CLASS)?;
    harness.state_mut().begin_assembly_export();
    harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?
        .set_interior(true);
    export(&mut harness, &dir.path().join("first.glb"))?;
    assert!(harness.state().assembly_export_settings().interior);

    harness.state_mut().begin_assembly_export();

    let setup = harness.state().assembly_export_setup().ok_or("no setup")?;
    assert!(setup.interior(), "the next export starts with it");

    harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?
        .set_interior(false);
    export(&mut harness, &dir.path().join("second.glb"))?;
    assert!(!harness.state().assembly_export_settings().interior);
    let bytes = std::fs::read(dir.path().join("second.glb"))?;
    assert!(!has_node(&bytes, "Interior")?, "the hull alone");
    assert!(
        summary(&harness)?.interior().is_none(),
        "none in the summary"
    );
    Ok(())
}

#[test]
fn the_export_starts_with_the_interior_of_the_view_once_the_view_was_changed() -> TestResult {
    let (_dir, mut harness) = hull_window(Language::BritishEnglish)?;
    select(&mut harness, ENTITY_INTERIOR_CLASS)?;
    harness
        .state_mut()
        .entities_mut()
        .loadout_mut()
        .ok_or("no Loadout")?
        .assembly_mut()
        .set_interior(true);

    harness.state_mut().begin_assembly_export();

    let setup = harness.state().assembly_export_setup().ok_or("no setup")?;
    assert!(setup.interior(), "as the view shows it");
    Ok(())
}

#[test]
fn an_entity_without_object_containers_has_no_interior_choice() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish)?;
    harness
        .state_mut()
        .set_assembly_export_settings(p4kx_settings::AssemblyExportSettings {
            interior: true,
            ..p4kx_settings::AssemblyExportSettings::default()
        });
    select(&mut harness, "TEST_Ship")?;

    harness.state_mut().begin_assembly_export();
    harness.run_steps(3);

    let setup = harness.state().assembly_export_setup().ok_or("no setup")?;
    assert!(!setup.has_interior());
    assert!(!setup.interior(), "the remembered choice does not apply");
    assert!(harness.query_by_label(TOGGLE).is_none(), "no switch shown");
    Ok(())
}
