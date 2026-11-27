// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Export menu and the Record export windows driven headless through `egui_kittest`.

use crate::exporting;
use crate::waiting::RunQuiet;

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use exporting::{
    TestResult, database, ended, files_below, many, opened, opened_with, opened_with_key,
    select_node, wait_until,
};
use p4kx_gui::{
    Language, Model, RecordExport, RecordExportStatus, RecordExportSummary, TargetKind, ui,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use tempfile::TempDir;

fn harness(model: Model) -> Harness<'static, Model> {
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1400.0, 900.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    harness.run_quiet();
    harness
}

fn window(opened: (TempDir, Model, PathBuf)) -> (TempDir, PathBuf, Harness<'static, Model>) {
    let (dir, mut model, out) = opened;
    model.set_extraction_folder(Some(out.clone()));
    (dir, out, harness(model))
}

fn wait_for_the_end(harness: &mut Harness<'_, Model>) -> TestResult {
    harness.step();
    wait_until(harness.state_mut(), |model| {
        model.record_export().is_some_and(|run| !run.is_running())
    })?;
    harness.run_quiet();
    Ok(())
}

fn open_dialog(harness: &mut Harness<'_, Model>, menu: &str, entry: &str) {
    harness.get_by_role_and_label(Role::Button, menu).click();
    harness.run_quiet();
    harness.get_by_label(entry).click();
    harness.run_quiet();
}

#[test]
fn the_export_menu_offers_record_export_only_with_a_dataforge() -> TestResult {
    let (_with, _, mut with) = window(opened(&database())?);
    let (_without, _, mut without) = window(opened_with(
        &ArchiveBuilder::new().entry(TestEntry::stored("readme.txt", "readme")),
    )?);

    open_dialog(&mut without, "Export", "Record export…");
    open_dialog(&mut with, "Export", "Record export…");

    assert!(
        without.state().record_export_setup().is_none(),
        "disabled without a DataForge"
    );
    assert!(
        with.state().record_export_setup().is_some(),
        "the setup opens"
    );
    with.get_by_label("Export Records");
    with.get_by_label("Selected in the Record tree (nothing selected)");
    with.get_by_label("JSON, one file per Record under its record path");
    with.get_by_label("NDJSON, one file records.ndjson with one Record per line");
    Ok(())
}

#[test]
fn the_tree_selection_is_exported_and_summarised() -> TestResult {
    let (dir, mut model, out) = opened(&database())?;
    select_node(&mut model, "libs/foundry/records/entities/spaceships")?;
    let (_dir, out, mut harness) = window((dir, model, out));

    open_dialog(&mut harness, "Export", "Record export…");
    harness.get_by_label("Selected in the Record tree: spaceships (2)");
    harness.get_by_label("Existing folder").click();
    harness.run_quiet();
    harness
        .get_by_label("NDJSON, one file records.ndjson with one Record per line")
        .click();
    harness.run_quiet();
    harness.get_by_label("Start export").click();
    wait_for_the_end(&mut harness)?;

    let window = harness.get_by_label("Record export finished").rect();
    let close = harness.get_by_label("Close").rect();
    assert!(
        window.max.y - close.max.y < 40.0,
        "no empty space under the foot"
    );
    harness.get_by_label("Records written 2, skipped 0, failed 0, cancelled 0");
    harness.get_by_label("Records with error values: 1, error values: 1");
    let file = out.join("records.ndjson");
    harness.get_by_label(&format!("Target: {}", file.display()));
    assert_eq!(fs::read_to_string(&file)?.lines().count(), 2, "two lines");
    harness.get_by_label("Close").click();
    harness.run_quiet();
    assert!(harness.state().record_export().is_none(), "dismissed");
    Ok(())
}

#[test]
fn the_record_export_count_shows_only_committed_dataforge_origins() -> TestResult {
    let dcb = database().build();
    let archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game.dcb", dcb.clone()))
        .entry(TestEntry::deflated("Data\\Game2.dcb", dcb).encrypted());
    let (dir, mut model, out) = opened_with_key(&archive, Some(test_key()))?;
    select_node(&mut model, "libs/foundry/records/entities/spaceships")?;
    let skipped = out.join("libs/foundry/records/entities/spaceships/aegs_avenger.json");
    fs::create_dir_all(skipped.parent().ok_or("Record folder")?)?;
    fs::write(&skipped, b"existing bytes")?;
    let (_dir, _, mut harness) = window((dir, model, out));
    harness.state_mut().begin_record_export();
    harness
        .state_mut()
        .record_export_setup_mut()
        .ok_or("Record export setup")?
        .set_kind(TargetKind::ExistingFolder);
    assert!(
        harness.state_mut().submit_record_export(|| {}),
        "Record export starts through the window model"
    );
    wait_for_the_end(&mut harness)?;
    assert_single_committed_definition(harness.state())?;
    assert_eq!(
        fs::read(&skipped)?,
        b"existing bytes",
        "kept output untouched"
    );

    let counts = harness.get_by_label("Records written 1, skipped 1, failed 0, cancelled 0");
    let point = counts.rect().center();
    harness.ctx.style_mut_of(harness.ctx.theme(), |style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    harness.event(egui::Event::PointerMoved(point));
    harness.run_steps(4);
    let _written = harness.get_by_label("Records written");
    let _origin = harness.get_by_label("Includes encrypted source data");
    Ok(())
}

fn assert_single_committed_definition(model: &Model) -> TestResult {
    let Some(RecordExportStatus::Finished(summary)) =
        model.record_export().map(RecordExport::status)
    else {
        return Err("finished summary".into());
    };
    let proof = summary
        .provenance()
        .ok_or("Record export provenance")?
        .map_err(ToString::to_string)?;
    assert_eq!(
        proof.written.contains_encrypted_source(),
        Some(true),
        "selected Game2.dcb"
    );
    assert!(
        proof.written.source_set_complete(),
        "physical origin of the committed Record"
    );
    assert_eq!(proof.inventory().source_count(), 1, "the actual DataForge");
    let mut contributors = proof
        .inventory()
        .iter_uses()
        .filter(|site| proof.written.source_uses_refs().contains(site.id()));
    let used = contributors.next().ok_or("committed definition range")?;
    assert!(
        contributors.next().is_none(),
        "only one committed definition range contributes to the counter"
    );
    let p4kx_extraction::SourceLocation::Subresource {
        namespace,
        indices,
        field,
    } = used.location()
    else {
        return Err("DataForge occurrence".into());
    };
    assert_eq!(
        namespace, "dataforgeRecordTable",
        "original DataForge definition table"
    );
    assert_eq!(
        field, "committedDefinitions",
        "only committed definitions contributed"
    );
    assert_eq!(indices, &[1, 2], "only the other ship was written");
    Ok(())
}

#[test]
fn a_typed_filter_selects_the_records_by_record_path() -> TestResult {
    let (_dir, out, mut harness) = window(opened(&database())?);

    open_dialog(&mut harness, "Export", "Record export…");
    let start_ready = harness
        .state()
        .record_export_setup()
        .is_some_and(p4kx_gui::RecordExportSetup::is_ready);
    let filter = (Role::TextInput, "Record path filter");
    harness.get_by_role_and_label(filter.0, filter.1).click();
    harness.run_quiet();
    harness
        .get_by_role_and_label(filter.0, filter.1)
        .type_text("weapons");
    harness.run_quiet();
    harness.get_by_label("Existing folder").click();
    harness.run_quiet();
    harness.get_by_label("Start export").click();
    wait_for_the_end(&mut harness)?;

    assert!(!start_ready, "no Records before a filter is typed");
    harness.get_by_label("Records written 2, skipped 0, failed 0, cancelled 0");
    assert_eq!(
        files_below(&out)?,
        [
            "libs/foundry/records/weapons/AmmoParams.Ballistic_S1.json",
            "libs/foundry/records/weapons/AmmoParams.Ballistic_S1.json.p4kx-origin.jsonl",
            "libs/foundry/records/weapons/AmmoParams.Laser_S1.json",
            "libs/foundry/records/weapons/AmmoParams.Laser_S1.json.p4kx-origin.jsonl",
        ],
        "the two selected weapons and their companions"
    );
    Ok(())
}

#[test]
fn the_flat_forms_are_offered_and_write_the_values_file() -> TestResult {
    let (_dir, out, mut harness) = window(opened(&database())?);

    open_dialog(&mut harness, "Export", "Record export…");
    harness.get_by_label("Values as NDJSON, values.ndjson, one object per value");
    harness.get_by_label("Values as CSV, exact: no byte order mark, no ' before text");
    let filter = (Role::TextInput, "Record path filter");
    harness.get_by_role_and_label(filter.0, filter.1).click();
    harness.run_quiet();
    harness
        .get_by_role_and_label(filter.0, filter.1)
        .type_text("weapons");
    harness.run_quiet();
    harness.get_by_label("Existing folder").click();
    harness.run_quiet();
    harness
        .get_by_label("Values as CSV, values.csv, one row per value")
        .click();
    harness.run_quiet();
    harness.get_by_label("Start export").click();
    wait_for_the_end(&mut harness)?;

    let bytes = fs::read(out.join("values.csv"))?;
    assert!(bytes.starts_with(b"\xEF\xBB\xBFrecord_guid,"));
    Ok(())
}

#[test]
fn the_running_export_can_be_cancelled_from_its_window() -> TestResult {
    let (_dir, out, mut harness) = window(opened(&many(6000))?);
    open_dialog(&mut harness, "Export", "Record export…");
    let filter = (Role::TextInput, "Record path filter");
    harness.get_by_role_and_label(filter.0, filter.1).click();
    harness.run_quiet();
    harness
        .get_by_role_and_label(filter.0, filter.1)
        .type_text("items");
    harness.run_quiet();
    harness.get_by_label("Existing folder").click();
    harness.run_quiet();
    harness.get_by_label("Start export").click();
    harness.step();
    harness.step();
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click();
    harness.step();
    let cancelled = matches!(
        ended(harness.state_mut())?,
        RecordExportStatus::Finished(summary) if summary.was_cancelled()
    );
    harness.run_quiet();

    assert!(cancelled, "the summary says so");
    assert!(
        harness
            .state()
            .record_export()
            .is_some_and(RecordExport::is_cancelled),
        "cancelled"
    );
    harness.get_by_label("Record export cancelled");
    harness.get_by_label("The Record export was cancelled before every Record was written.");
    assert!(files_below(&out)?.len() < 6000, "stopped early");
    Ok(())
}

#[test]
fn the_dialog_speaks_german() -> TestResult {
    let (dir, mut model, out) = opened(&database())?;
    model.set_language(Language::German);
    let (_dir, _out, mut harness) = window((dir, model, out));

    open_dialog(&mut harness, "Export", "Record-Export …");

    harness.get_by_label("Records exportieren");
    harness.get_by_role_and_label(Role::RadioButton, "Filter nach Record-Pfad");
    harness.get_by_label("Export starten");
    Ok(())
}

#[test]
fn an_export_cancelled_while_writing_reports_what_it_wrote() -> TestResult {
    let (_dir, out, mut harness) = window(opened(&many(20000))?);
    open_dialog(&mut harness, "Export", "Record export…");
    let filter = (Role::TextInput, "Record path filter");
    harness.get_by_role_and_label(filter.0, filter.1).click();
    harness.run_quiet();
    harness
        .get_by_role_and_label(filter.0, filter.1)
        .type_text("items");
    harness.run_quiet();
    harness.get_by_label("Existing folder").click();
    harness.run_quiet();
    harness.get_by_label("Start export").click();
    harness.step();
    wait_until(harness.state_mut(), |model| {
        model.record_export().is_some_and(|run| {
            !matches!(
                run.status(),
                RecordExportStatus::Running(progress) if progress.done == 0
            )
        })
    })?;
    harness.step();
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click();
    harness.step();
    let RecordExportStatus::Finished(summary) = ended(harness.state_mut())? else {
        return Err("no summary".into());
    };
    assert!(summary.was_cancelled(), "cancelled");
    assert!(summary.written() > 0, "some Records were written first");
    assert!(
        summary.failures().is_empty(),
        "cancellation does not report failed Records: {:?}",
        summary.failures()
    );
    assert_eq!(
        summary.written() + summary.cancelled(),
        20000,
        "every Record is accounted for"
    );
    assert_cancelled_record_files(&out, summary)?;
    harness.run_quiet();
    harness.get_by_label("Record export cancelled");
    harness.get_by_label(&format!("Target: {}", out.display()));
    Ok(())
}

fn assert_cancelled_record_files(out: &Path, summary: &RecordExportSummary) -> TestResult {
    let files = files_below(out)?;
    let payloads = files
        .iter()
        .filter(|path| Path::new(path).extension() == Some(OsStr::new("json")))
        .count();
    let companions = summary.origin_receipts();
    let written = summary.written();
    assert_eq!(payloads, written, "committed payload count");
    let other_files = files.len() - payloads;
    assert_eq!(other_files, companions.len(), "only companions remain");
    let all_exist = companions.iter().all(|path| path.is_file());
    assert!(all_exist, "every reported companion exists");
    let reported = companions.len() + summary.origin_failures().len();
    assert!(reported >= written, "missing companions are reported");
    Ok(())
}
