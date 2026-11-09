// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Basetool catalog entry of the Export menu and its windows driven headless through
//! `egui_kittest`.

#[path = "catalog_category_lists_window.rs"]
mod category_lists;

use crate::cataloging;
use crate::waiting::{RunQuiet, wait_for};

use cataloging::{TestResult, files_in, makers, opened};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_export::CatalogPublicationMode;
use p4kx_gui::ui::{self, Dialog};
use p4kx_gui::{CatalogExportOptions, CatalogExportStage, CatalogExportStatus, Language, Model};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

struct Window {
    model: Model,
    dialog: Option<Dialog>,
}

fn harness(model: Model) -> Harness<'static, Window> {
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1400.0, 900.0])
        .build_ui_state(
            |ui, window: &mut Window| {
                let requests = ui::show(ui, &mut window.model);
                if requests.dialog.is_some() {
                    window.dialog = requests.dialog;
                }
            },
            Window {
                model,
                dialog: None,
            },
        );
    harness.run_steps(2);
    harness
}

fn pick(harness: &mut Harness<'_, Window>, menu: &str, entry: &str) {
    harness.get_by_role_and_label(Role::Button, menu).click();
    harness.run_quiet();
    harness.get_by_label(entry).click();
    harness.run_quiet();
}

fn wait_for_the_end(harness: &mut Harness<'_, Window>) -> TestResult {
    wait_for(harness, "the catalog to end", |harness| {
        harness
            .state()
            .model
            .catalog()
            .is_some_and(|run| !run.is_running())
    })?;
    harness.run_quiet();
    Ok(())
}

#[test]
fn the_export_menu_asks_for_the_catalog_file_only_with_a_dataforge() -> TestResult {
    let (_with, with, _) = opened(&makers())?;
    let (_without, without, _) =
        opened(&ArchiveBuilder::new().entry(TestEntry::stored("readme.txt", "readme")))?;
    let mut with = harness(with);
    let mut without = harness(without);

    pick(&mut without, "Export", "Basetool catalog…");
    pick(&mut with, "Export", "Basetool catalog…");

    assert_eq!(without.state().dialog, None, "disabled without a DataForge");
    assert_eq!(
        with.state().dialog,
        Some(Dialog::CatalogFile),
        "the save dialog"
    );
    Ok(())
}

#[test]
fn the_diagnostic_menu_requests_a_separate_save_dialog() -> TestResult {
    let (_dir, model, _) = opened(&makers())?;
    let mut harness = harness(model);

    pick(&mut harness, "Export", "Diagnostic Basetool catalog…");

    assert_eq!(
        harness.state().dialog,
        Some(Dialog::DiagnosticCatalogFile),
        "diagnostic permission is an explicit menu choice"
    );
    Ok(())
}

#[test]
fn a_normal_incomplete_job_explains_the_refusal_without_a_success_window() -> TestResult {
    let (_dir, mut model, out) = opened(&cataloging::unreadable())?;
    model.start_catalog(out.join("catalog.json"), || {});
    let mut harness = harness(model);

    wait_for_the_end(&mut harness)?;

    harness.get_by_label("Basetool catalog failed");
    harness.get_by_label_contains("No file was created or replaced.");
    assert!(
        harness.query_by_label("Basetool catalog created").is_none(),
        "the refusal cannot look like successful publication"
    );
    assert!(files_in(&out)?.is_empty(), "nothing was published");
    Ok(())
}

#[test]
fn the_summary_shows_the_kinds_the_texts_the_size_and_the_file() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    let file = out.join("basetool-catalog.json");
    model.start_catalog(file.clone(), || {});
    let mut harness = harness(model);

    wait_for_the_end(&mut harness)?;

    harness.get_by_label("Basetool catalog created");
    harness
        .get_by_label("Export health: all selected sources were read and projected successfully.");
    harness.get_by_label("Manufacturers: 2 written, 1 skipped, complete");
    harness.get_by_label("Resource types: 0 written, 0 skipped, complete");
    harness.get_by_label("Global tags: 0 written, 0 skipped, complete");
    harness.get_by_label("Mission property sets: 0 written, 0 skipped, complete");
    harness.get_by_label("Records skipped as development leftovers: 1");
    harness.get_by_label("Playability");
    harness.get_by_label("Items: 0 player, 0 AI, 0 mission, 0 special, 0 unclassified");
    harness.get_by_label("Ships: 0 player, 0 AI, 0 mission, 0 special, 0 unclassified");
    harness.get_by_label("English: 4 resolved, 0 missing, 0 placeholders");
    harness.get_by_label("German: 3 resolved, 1 missing, 0 placeholders");
    harness.get_by_label("Missing texts: 1");
    harness.get_by_label(&format!("File: {}", file.display()));
    assert!(
        harness
            .query_by_label_contains("is close to the basetool upload limit")
            .is_none(),
        "no size warning"
    );
    let window = harness.get_by_label("Basetool catalog created").rect();
    let close = harness.get_by_label("Close").rect();
    assert!(
        window.max.y - close.max.y < 40.0,
        "no empty space under the foot"
    );
    harness.get_by_label("Close").click();
    harness.run_quiet();
    assert!(harness.state().model.catalog().is_none(), "dismissed");
    Ok(())
}

#[test]
fn an_unreadable_record_is_listed_in_an_explicit_diagnostic_summary() -> TestResult {
    let (_dir, mut model, out) = opened(&cataloging::unreadable())?;
    let file = out.join("basetool-catalog.json");
    model.start_catalog_with_options(
        file.clone(),
        CatalogExportOptions {
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..CatalogExportOptions::default()
        },
        || {},
    );
    let mut harness = harness(model);

    wait_for_the_end(&mut harness)?;

    harness.get_by_label("Diagnostic Basetool catalog written");
    harness.get_by_label_contains("Export health: incomplete.");
    harness.get_by_label_contains("Records that could not be read and were left out (1)");
    harness.get_by_label_contains(
        "libs/foundry/records/crafting/blueprints/crafting/broken (90000000-0000-4000-8000-000000000009)",
    );
    harness.get_by_label("Manufacturers: 1 written, 0 skipped, complete");
    assert!(file.is_file(), "the catalog is written");
    Ok(())
}

#[test]
fn a_compressed_catalog_shows_both_sizes() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    model.start_catalog(out.join("catalog.json.zst"), || {});
    let mut harness = harness(model);

    wait_for_the_end(&mut harness)?;

    harness.get_by_label_contains(" compressed (of the upload limit of 64 MB), ");
    harness.get_by_label_contains(" uncompressed");
    Ok(())
}

#[test]
fn a_compressed_catalog_shows_both_sizes_in_german() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    model.set_language(Language::German);
    model.start_catalog(out.join("catalog.json.zst"), || {});
    let mut harness = harness(model);

    wait_for_the_end(&mut harness)?;

    harness.get_by_label("Globale Tags: 0 geschrieben, 0 übersprungen, vollständig");
    harness.get_by_label_contains(" komprimiert (vom Upload-Limit 64 MB), ");
    harness.get_by_label_contains(" unkomprimiert");
    Ok(())
}

#[test]
fn a_plain_catalog_shows_one_size() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    model.start_catalog(out.join("catalog.json"), || {});
    let mut harness = harness(model);

    wait_for_the_end(&mut harness)?;

    harness.get_by_label_contains("of the upload limit of 64 MB");
    assert!(
        harness.query_by_label_contains("uncompressed").is_none(),
        "no second size"
    );
    Ok(())
}

#[test]
fn a_catalog_near_the_upload_limit_shows_the_warning() -> TestResult {
    let (_dir, mut model, out) = opened(&cataloging::near_upload_limit())?;
    model.start_catalog(out.join("catalog.json"), || {});
    let mut harness = harness(model);

    wait_for_the_end(&mut harness)?;

    harness.get_by_label("The plain JSON catalog is close to the basetool upload limit of 64 MB. Write the catalog as .json.zst for the upload; plain .json is for inspection.");
    Ok(())
}

#[test]
fn a_catalog_over_the_upload_limit_shows_the_danger_alert() -> TestResult {
    let (_dir, mut model, out) = opened(&cataloging::wordy(34, 1_000_000))?;
    model.start_catalog(out.join("catalog.json"), || {});
    let mut harness = harness(model);

    wait_for_the_end(&mut harness)?;

    harness.get_by_label(
        "The plain JSON catalog is over the basetool upload limit of 64 MB; the basetool will reject it. Write the catalog as .json.zst for the upload; plain .json is for inspection.",
    );
    assert!(
        harness
            .query_by_label_contains("is close to the basetool upload limit")
            .is_none(),
        "no warning besides the danger alert"
    );
    Ok(())
}

#[test]
fn a_second_catalog_shows_its_own_summary() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    let first = out.join("first.json");
    let second = out.join("second.json");
    model.start_catalog(first.clone(), || {});
    let mut harness = harness(model);
    wait_for_the_end(&mut harness)?;
    harness.get_by_label(&format!("File: {}", first.display()));
    harness.get_by_label("Close").click();
    harness.run_quiet();

    let started = harness
        .state_mut()
        .model
        .start_catalog(second.clone(), || {});
    wait_for_the_end(&mut harness)?;

    assert!(started, "the second starts");
    harness.get_by_label(&format!("File: {}", second.display()));
    assert!(
        harness
            .query_by_label(&format!("File: {}", first.display()))
            .is_none(),
        "not the first summary"
    );
    Ok(())
}

#[test]
fn the_open_archive_as_the_catalog_file_is_explained() -> TestResult {
    let (dir, mut model, _) = opened(&makers())?;
    let archive = dir.path().join("Data.p4k");
    model.start_catalog(archive.clone(), || {});
    let mut harness = harness(model);

    harness.run_quiet();

    harness.get_by_label("Basetool catalog failed");
    harness.get_by_label(&format!(
        "The Basetool catalog cannot be written to {}, because that is the open Archive. Choose another file.",
        archive.display()
    ));
    Ok(())
}

#[test]
fn the_running_catalog_can_be_cancelled_from_its_window() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    let hold = model.hold_catalog();
    model.start_catalog(out.join("basetool-catalog.json"), || {});
    let mut harness = harness(model);
    wait_for(&mut harness, "the catalog to build", |harness| {
        harness.state().model.catalog().is_some_and(|run| {
            matches!(
                run.status(),
                CatalogExportStatus::Running(progress)
                    if progress.stage == CatalogExportStage::Building
            )
        })
    })?;

    harness.get_by_label("Creating the Basetool catalog");
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click();
    wait_for(&mut harness, "the catalog to be cancelled", |harness| {
        harness
            .state()
            .model
            .catalog()
            .is_some_and(|run| matches!(run.status(), CatalogExportStatus::Cancelled))
    })?;
    harness.run_quiet();
    drop(hold);

    harness.get_by_label("Basetool catalog cancelled");
    harness.get_by_label("The Basetool catalog was cancelled. No file was written.");
    assert!(files_in(&out)?.is_empty(), "no file");
    Ok(())
}

#[test]
fn the_catalog_windows_speak_german() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    model.set_language(Language::German);
    let mut harness = harness(model);

    harness
        .get_by_role_and_label(Role::Button, "Export")
        .click();
    harness.run_quiet();
    harness.get_by_label("Basetool-Katalog …");
    let started = harness
        .state_mut()
        .model
        .start_catalog(out.join("basetool-catalog.json"), || {});
    wait_for_the_end(&mut harness)?;

    assert!(started, "started");

    harness.get_by_label("Basetool-Katalog erstellt");
    harness.get_by_label("Hersteller: 2 geschrieben, 1 übersprungen, vollständig");
    harness.get_by_label("Deutsch: 3 aufgelöst, 1 fehlen, 0 Platzhalter");
    harness.get_by_label("Spielbarkeit");
    harness.get_by_label("Schiffe: 0 Spieler, 0 KI, 0 Mission, 0 Sonderfall, 0 nicht eingeordnet");
    Ok(())
}
