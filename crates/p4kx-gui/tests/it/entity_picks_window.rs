// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Loadout picks field of the export dialog of an Entity: what it sets, when it blocks the
//! export, and how it is drawn.

use egui_kittest::Harness;
use p4kx_gui::{Language, Model, WorkTab, ui};
use p4kx_testkit::{ENTITY_INTERIOR_CLASS, entity_interior_entries};

use crate::entity_package_render::{render, renderer};
use crate::entity_picking::{TestResult, listed, select};

fn open_setup(harness: &mut Harness<'_, Model>, path: std::path::PathBuf) -> TestResult {
    harness.state_mut().open(path, || {});
    crate::tagging::wait_until(harness, |model| model.browser().is_some())?;
    harness.state_mut().set_work_tab(WorkTab::Entities);
    harness.run_steps(2);
    crate::tagging::wait_until(harness, listed)?;
    select(harness, ENTITY_INTERIOR_CLASS)?;
    harness.state_mut().begin_assembly_export();
    Ok(())
}

#[test]
fn the_picks_text_sets_the_picks_and_a_bad_text_blocks_the_export() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = crate::tagging::archive(dir.path(), entity_interior_entries())?;
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1280.0, 800.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            Model::new(Language::BritishEnglish),
        );
    open_setup(&mut harness, path)?;

    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?;
    setup.set_file(dir.path().join("beast.glb"));
    assert!(setup.is_ready());
    assert!(setup.changes().picks().is_empty());

    setup.set_picks_text("Body=2, 1");
    assert!(setup.is_ready());
    assert_eq!(setup.changes().picks().every_index(), Some(1));
    assert_eq!(setup.changes().picks().groups(), [("Body".to_owned(), 2)]);

    setup.set_picks_text("Body=");
    assert!(setup.picks_invalid());
    assert!(!setup.is_ready(), "a bad text blocks the export");
    assert_eq!(
        setup.changes().picks().groups(),
        [("Body".to_owned(), 2)],
        "the picks stay as they were"
    );

    setup.set_picks_text("");
    assert!(setup.is_ready());
    assert!(setup.changes().picks().is_empty());
    Ok(())
}

#[test]
fn the_picks_field_is_drawn_valid_and_invalid_in_both_languages() -> TestResult {
    for (language, suffix) in [
        (Language::BritishEnglish, "en-GB"),
        (Language::German, "de-DE"),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let dir = tempfile::tempdir()?;
        let path = crate::tagging::archive(dir.path(), entity_interior_entries())?;
        let mut harness = Harness::builder()
            .with_context_setup(p4kx_gui::theme::apply)
            .with_size([1280.0, 800.0])
            .renderer(renderer)
            .build_ui_state(
                |ui, model: &mut Model| {
                    ui::show(ui, model);
                },
                Model::new(language),
            );
        open_setup(&mut harness, path)?;
        render(&mut harness, &format!("picks-empty-{suffix}.png"))?;

        let setup = harness
            .state_mut()
            .assembly_export_setup_mut()
            .ok_or("no setup")?;
        setup.set_picks_text("Body=1, Eggs=2");
        render(&mut harness, &format!("picks-set-{suffix}.png"))?;

        let setup = harness
            .state_mut()
            .assembly_export_setup_mut()
            .ok_or("no setup")?;
        setup.set_picks_text("Body=first");
        render(&mut harness, &format!("picks-invalid-{suffix}.png"))?;
    }
    Ok(())
}
