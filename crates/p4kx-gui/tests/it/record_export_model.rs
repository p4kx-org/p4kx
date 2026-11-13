// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Record export of the window driven headless: the selection from the Record tree or a
//! record-path filter, both forms, cancel and the summary.

use crate::exporting;

use std::fs;
use std::time::{Duration, Instant};

use exporting::{
    TestResult, database, ended, files_below, many, opened, opened_with, select_node,
    wait_for_writing, wait_until,
};
use p4kx_extraction::ConflictPolicy;
use p4kx_gui::{RecordExportStatus, RecordForm, RecordScope, TargetKind};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

fn choose(model: &mut p4kx_gui::Model, out: &std::path::Path, form: RecordForm) -> TestResult {
    let setup = model
        .record_export_setup_mut()
        .ok_or("the Record export setup did not open")?;
    setup.set_form(form);
    setup.set_kind(TargetKind::ExistingFolder);
    setup.set_folder(out.to_path_buf());
    Ok(())
}

#[test]
fn record_export_needs_an_archive_with_a_dataforge() -> TestResult {
    let (_with, with, _) = opened(&database())?;
    let (_without, mut without, _) = opened_with(
        &ArchiveBuilder::new().entry(TestEntry::stored("Data\\Libs\\config.xml", "config")),
    )?;

    without.begin_record_export();

    assert!(with.record_export_available(), "offered with a DataForge");
    assert!(!without.record_export_available(), "not without one");
    assert!(without.record_export_setup().is_none(), "no setup opens");
    Ok(())
}

#[test]
fn the_selected_record_folder_is_exported_as_json_files() -> TestResult {
    let (_dir, mut model, out) = opened(&database())?;
    select_node(&mut model, "libs/foundry/records/weapons")?;

    model.begin_record_export();
    let setup = model.record_export_setup().ok_or("no setup")?;
    assert_eq!(setup.scope(), RecordScope::Tree, "the tree selection first");
    assert_eq!(
        setup.tree().map(|tree| (tree.name(), tree.len())),
        Some(("weapons", 2))
    );
    choose(&mut model, &out, RecordForm::Json)?;
    assert!(model.submit_record_export(|| {}), "started");
    let RecordExportStatus::Finished(summary) = ended(&mut model)? else {
        return Err("no summary".into());
    };

    assert_eq!(summary.written(), 2, "both weapons");
    assert_eq!(summary.records_with_errors(), 0, "no error values");
    assert_eq!(summary.target(), Some(out.as_path()), "the existing folder");
    assert_eq!(
        files_below(&out)?,
        [
            "libs/foundry/records/weapons/AmmoParams.Ballistic_S1.json",
            "libs/foundry/records/weapons/AmmoParams.Laser_S1.json",
        ],
        "one file per Record under its record path"
    );
    Ok(())
}

#[test]
fn a_record_path_filter_that_is_refused_is_not_exported() -> TestResult {
    let (_dir, mut model, out) = opened(&database())?;

    model.begin_record_export();
    choose(&mut model, &out, RecordForm::Ndjson)?;
    for text in ["a".repeat(2000), "*a".repeat(40)] {
        let setup = model.record_export_setup_mut().ok_or("no setup")?;
        setup.set_filter(&text);
        assert!(setup.filter_error().is_some(), "{} bytes", text.len());
        assert!(!setup.is_ready(), "not ready with {} bytes", text.len());
        assert!(!model.submit_record_export(|| {}), "not started");
    }
    Ok(())
}

#[test]
fn a_record_path_filter_is_exported_as_one_ndjson_file() -> TestResult {
    let (_dir, mut model, out) = opened(&database())?;

    model.begin_record_export();
    choose(&mut model, &out, RecordForm::Ndjson)?;
    let setup = model.record_export_setup_mut().ok_or("no setup")?;
    assert_eq!(setup.scope(), RecordScope::Filter, "no tree selection");
    assert!(!setup.is_ready(), "not without a filter");
    setup.set_filter("*spaceships*");
    assert!(model.submit_record_export(|| {}), "started");
    let RecordExportStatus::Finished(summary) = ended(&mut model)? else {
        return Err("no summary".into());
    };

    assert_eq!(summary.selected(), 2, "both ships match");
    assert_eq!(summary.written(), 2, "two lines");
    assert_eq!(
        summary.records_with_errors(),
        1,
        "the Avenger's dangling Reference"
    );
    assert_eq!(summary.error_values(), 1, "one error value");
    assert_eq!(summary.file(), Some(out.join("records.ndjson")), "the file");
    let text = fs::read_to_string(out.join("records.ndjson"))?;
    let paths: Vec<String> = text
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).map(|v| v["$path"].to_string()))
        .collect::<Result<_, _>>()?;
    assert_eq!(
        paths,
        [
            "\"libs/foundry/records/entities/spaceships/aegs_avenger\"",
            "\"libs/foundry/records/entities/spaceships/anvl_arrow\"",
        ],
        "Record order"
    );
    Ok(())
}

#[test]
fn a_filter_that_matches_nothing_writes_nothing() -> TestResult {
    let (_dir, mut model, out) = opened(&database())?;

    model.begin_record_export();
    choose(&mut model, &out, RecordForm::Json)?;
    model
        .record_export_setup_mut()
        .ok_or("no setup")?
        .set_filter("nothing/here");
    model.submit_record_export(|| {});
    let RecordExportStatus::Finished(summary) = ended(&mut model)? else {
        return Err("no summary".into());
    };

    assert_eq!(summary.selected(), 0, "no match");
    assert!(!summary.was_cancelled(), "not cancelled");
    assert!(files_below(&out)?.is_empty(), "nothing written");
    Ok(())
}

#[test]
fn a_new_folder_is_created_inside_the_chosen_folder() -> TestResult {
    let (_dir, mut model, out) = opened(&database())?;

    model.begin_record_export();
    let setup = model.record_export_setup_mut().ok_or("no setup")?;
    setup.set_folder(out.clone());
    setup.set_filter("weapons");
    model.submit_record_export(|| {});
    let RecordExportStatus::Finished(summary) = ended(&mut model)? else {
        return Err("no summary".into());
    };

    let target = summary.target().ok_or("no target")?;
    let created = target.strip_prefix(&out)?.to_string_lossy().into_owned();
    assert!(created.starts_with("SC-EXPORT-"), "a new folder: {created}");
    assert_eq!(files_below(target)?.len(), 2, "both weapons in it");
    assert_eq!(model.extraction_folder(), Some(out.as_path()), "remembered");
    Ok(())
}

#[test]
fn existing_files_follow_the_conflict_policy() -> TestResult {
    let (_dir, mut model, out) = opened(&database())?;
    let file = out.join("libs/foundry/records/weapons/AmmoParams.Laser_S1.json");
    fs::create_dir_all(file.parent().ok_or("no parent")?)?;
    fs::write(&file, "old")?;

    model.begin_record_export();
    choose(&mut model, &out, RecordForm::Json)?;
    let setup = model.record_export_setup_mut().ok_or("no setup")?;
    setup.set_filter("weapons");
    setup.set_policy(ConflictPolicy::Skip);
    model.submit_record_export(|| {});
    let RecordExportStatus::Finished(summary) = ended(&mut model)? else {
        return Err("no summary".into());
    };

    assert_eq!(summary.written(), 1, "the other weapon");
    assert_eq!(
        summary.skipped(),
        ["libs/foundry/records/weapons/AmmoParams.Laser_S1.json"],
        "kept"
    );
    assert_eq!(fs::read(&file)?, b"old", "unchanged");
    Ok(())
}

#[test]
fn cancelling_stops_promptly_and_the_summary_says_so() -> TestResult {
    let (_dir, mut model, out) = opened(&many(4000))?;

    model.begin_record_export();
    choose(&mut model, &out, RecordForm::Ndjson)?;
    model
        .record_export_setup_mut()
        .ok_or("no setup")?
        .set_filter("items");
    assert!(model.submit_record_export(|| {}), "started");
    model.cancel_record_export();
    let cancelled_at = Instant::now();
    let RecordExportStatus::Finished(summary) = ended(&mut model)? else {
        return Err("no summary".into());
    };

    assert!(
        cancelled_at.elapsed() < Duration::from_secs(5),
        "stops promptly"
    );
    assert!(summary.was_cancelled(), "the summary says so");
    assert_eq!(
        summary.written(),
        0,
        "a cancelled NDJSON export keeps no file"
    );
    assert!(!out.join("records.ndjson").exists(), "no file");
    assert!(
        model
            .record_export()
            .is_some_and(p4kx_gui::RecordExport::is_cancelled),
        "the export knows it was cancelled"
    );
    Ok(())
}

#[test]
fn a_second_export_reuses_the_dataforge_read_by_the_first() -> TestResult {
    let (_dir, mut model, out) = opened(&database())?;

    model.begin_record_export();
    choose(&mut model, &out, RecordForm::Json)?;
    model
        .record_export_setup_mut()
        .ok_or("no setup")?
        .set_filter("weapons");
    model.submit_record_export(|| {});
    ended(&mut model)?;
    let kept = model.record_export_keeps_dataforge();
    model.begin_record_export();
    choose(&mut model, &out, RecordForm::Json)?;
    let setup = model.record_export_setup_mut().ok_or("no setup")?;
    setup.set_filter("spaceships");
    setup.set_policy(ConflictPolicy::Overwrite);
    model.submit_record_export(|| {});
    let RecordExportStatus::Finished(summary) = ended(&mut model)? else {
        return Err("no summary".into());
    };

    assert!(kept, "kept after the first export");
    assert_eq!(summary.written(), 2, "both ships");
    assert_eq!(files_below(&out)?.len(), 4, "weapons and ships");
    Ok(())
}

#[test]
fn only_one_record_export_runs_at_a_time() -> TestResult {
    let (_dir, mut model, out) = opened(&many(4000))?;
    model.begin_record_export();
    choose(&mut model, &out, RecordForm::Json)?;
    model
        .record_export_setup_mut()
        .ok_or("no setup")?
        .set_filter("items");
    model.submit_record_export(|| {});

    let available = model.record_export_available();
    model.stop_record_export();

    assert!(!available, "not while one runs");
    assert!(
        model.record_export().is_some_and(|run| !run.is_running()),
        "ended once stop returns"
    );
    wait_until(&mut model, p4kx_gui::Model::record_export_available)
}

#[test]
fn a_cancelled_export_can_be_restarted_at_once_and_keeps_dataforge() -> TestResult {
    let (_dir, mut model, out) = opened(&database())?;
    let hold = model.hold_record_export_writing();
    model.begin_record_export();
    choose(&mut model, &out, RecordForm::Json)?;
    model
        .record_export_setup_mut()
        .ok_or("no setup")?
        .set_filter("weapons");
    assert!(model.submit_record_export(|| {}), "started");
    wait_for_writing(&mut model)?;

    model.cancel_record_export();
    let RecordExportStatus::Finished(summary) = ended(&mut model)? else {
        return Err("no summary".into());
    };
    let cancelled = summary.was_cancelled();
    drop(hold);
    model.begin_record_export();
    choose(&mut model, &out, RecordForm::Json)?;
    model
        .record_export_setup_mut()
        .ok_or("no setup")?
        .set_filter("weapons");
    let restarted = model.submit_record_export(|| {});
    let RecordExportStatus::Finished(again) = ended(&mut model)? else {
        return Err("no summary for the restart".into());
    };
    let handled = again.written() + again.skipped().len();

    assert!(cancelled, "the summary says so");
    assert!(restarted, "a new export starts at once");
    assert_eq!(handled, 2, "the restart writes every Record");
    assert!(model.record_export_keeps_dataforge(), "DataForge is kept");
    Ok(())
}

fn rewritten(dir: &std::path::Path, forge: &p4kx_testkit::DataForgeBuilder) -> TestResult {
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Libs\\config.xml", "config"))
        .entry(TestEntry::stored("Data\\patch.txt", "patch"))
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge.build()))
        .write_to(dir.join("Data.p4k"))?;
    Ok(())
}

#[test]
fn an_open_setup_closes_and_dataforge_is_forgotten_when_the_archive_changes() -> TestResult {
    let (dir, mut model, out) = opened(&database())?;
    model.begin_record_export();
    choose(&mut model, &out, RecordForm::Json)?;
    model
        .record_export_setup_mut()
        .ok_or("no setup")?
        .set_filter("weapons");
    model.submit_record_export(|| {});
    ended(&mut model)?;
    model.begin_record_export();
    let kept_before = model.record_export_keeps_dataforge();

    rewritten(dir.path(), &database())?;
    let noticed = model.check_archive();

    assert!(kept_before, "kept before the change");
    assert!(noticed, "the change is noticed");
    assert!(model.record_export_setup().is_none(), "the setup is closed");
    assert!(
        !model.record_export_keeps_dataforge(),
        "the DataForge read before is forgotten"
    );
    assert!(!model.record_export_available(), "no new export is offered");
    Ok(())
}

#[test]
fn a_running_export_goes_on_when_the_archive_changes() -> TestResult {
    let (dir, mut model, out) = opened(&database())?;
    let hold = model.hold_record_export_writing();
    model.begin_record_export();
    choose(&mut model, &out, RecordForm::Json)?;
    model
        .record_export_setup_mut()
        .ok_or("no setup")?
        .set_filter("weapons");
    assert!(model.submit_record_export(|| {}), "started");
    wait_for_writing(&mut model)?;

    rewritten(dir.path(), &many(10))?;
    assert!(
        model.check_archive(),
        "archive change observed before writing"
    );
    drop(hold);
    let RecordExportStatus::Finished(summary) = ended(&mut model)? else {
        return Err("no summary".into());
    };

    assert!(!summary.was_cancelled(), "not cancelled");
    assert_eq!(summary.written(), 2, "every Record");
    Ok(())
}
