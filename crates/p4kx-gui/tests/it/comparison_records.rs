// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Record level of the Comparison tab over a pair of Archives with synthetic DataForges.

use crate::waiting::{RunQuiet, wait_polling_described};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use p4kx_comparison::{
    Archive, CompareOptions, DataForgeEntry, DataForges, Levels, compare_with_dataforge_entries,
};
use p4kx_gui::{
    ComparisonLevels, ComparisonSide, ComparisonStatus, ComparisonTab, Language, Model,
    ReportStatus, WorkTab, ui,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

fn database(speed: i32) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Ship").field("speed", TestType::Int32))
        .record(
            TestRecord::new(1, "Ship", "Ship.Tuned", "libs/ships/tuned.xml")
                .value("speed", TestValue::Int32(speed)),
        )
}

fn write(dir: &Path, channel: &str, speed: i32, extra: &str) -> Result<PathBuf, Box<dyn Error>> {
    let folder = dir.join(channel);
    fs::create_dir_all(&folder)?;
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data\\Game2.dcb",
            database(speed).build(),
        ))
        .entry(TestEntry::stored(extra, vec![1; 8]))
        .write_to(&path)?;
    Ok(path)
}

fn pair() -> Result<(TempDir, PathBuf, PathBuf), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let old = write(dir.path(), "LIVE", 4, "Data\\old.txt")?;
    let new = write(dir.path(), "PTU", 40, "Data\\new.txt")?;
    Ok((dir, old, new))
}

fn wait(tab: &mut ComparisonTab, done: impl Fn(&ComparisonTab) -> bool) -> TestResult {
    wait_polling_described(
        tab,
        "the Comparison to settle",
        |tab| {
            tab.poll();
        },
        done,
        |tab| format!("{:?}", tab.status()),
    )?;
    Ok(())
}

fn compared(old: &Path, new: &Path, levels: ComparisonLevels) -> Result<Model, Box<dyn Error>> {
    let mut model = Model::new(Language::BritishEnglish);
    let tab = model.comparison_mut();
    tab.set_archive(ComparisonSide::Old, old.to_path_buf());
    tab.set_archive(ComparisonSide::New, new.to_path_buf());
    tab.set_levels(levels);
    if !tab.start(|| {}) {
        return Err("the Comparison did not start".into());
    }
    wait(tab, |tab| !tab.is_running())?;
    match tab.status() {
        ComparisonStatus::Finished(_) => Ok(model),
        other => Err(format!("not finished: {other:?}").into()),
    }
}

fn cli_report(old: &Path, new: &Path, levels: Levels, out: &Path) -> TestResult {
    let (old, new) = (
        Archive::open_with_key(old, Some(p4kx_testkit::test_key()))?,
        Archive::open_with_key(new, Some(p4kx_testkit::test_key()))?,
    );
    let (old_entry, new_entry) = (DataForgeEntry::read(&old)?, DataForgeEntry::read(&new)?);
    let (old_forge, new_forge) = (old_entry.open()?, new_entry.open()?);
    let forges = DataForges {
        old: &old_forge,
        new: &new_forge,
    };
    let options = CompareOptions {
        levels,
        ..CompareOptions::default()
    };
    compare_with_dataforge_entries(&old, &new, forges, [&old_entry, &new_entry], &options)?
        .save_report(out)?;
    Ok(())
}

#[test]
fn the_saved_report_holds_the_record_level_like_the_cli() -> TestResult {
    let (dir, old, new) = pair()?;
    let both = ComparisonLevels {
        entries: true,
        records: true,
    };
    let mut model = compared(&old, &new, both)?;
    let saved = dir.path().join("window.json");
    let expected = dir.path().join("cli.json");

    let tab = model.comparison_mut();
    tab.view_mut()
        .ok_or("no view")?
        .save_report(saved.clone(), || {});
    wait(tab, |tab| {
        tab.view()
            .is_some_and(|view| !matches!(view.report_status(), Some(ReportStatus::Saving(_))))
    })?;
    cli_report(&old, &new, Levels::ALL, &expected)?;

    let status = tab.view().and_then(|view| view.report_status());
    assert!(
        matches!(status, Some(ReportStatus::Saved(_))),
        "saved: {status:?}"
    );
    let written = fs::read_to_string(&saved)?;
    assert_eq!(written, fs::read_to_string(&expected)?, "byte for byte");
    let report: serde_json::Value = serde_json::from_str(&written)?;
    let name_marker = report
        .pointer("/records/fieldOrigins")
        .and_then(|origins| origins.get("/records/changes/0/new/name"))
        .and_then(|origin| origin.get("containsEncryptedSource"))
        .and_then(serde_json::Value::as_bool);
    assert_eq!(
        name_marker,
        Some(false),
        "the saved Record name has bound clear input evidence"
    );
    Ok(())
}

#[test]
fn the_record_level_alone_lists_no_entries() -> TestResult {
    let (_dir, old, new) = pair()?;
    let records = ComparisonLevels {
        entries: false,
        records: true,
    };

    let model = compared(&old, &new, records)?;

    let view = model.comparison().view().ok_or("no view")?;
    assert!(view.shown().is_empty(), "no Entry changes");
    assert!(view.comparison().levels().records, "the Record level ran");
    Ok(())
}

#[test]
fn both_level_switches_work_in_the_window() -> TestResult {
    let (_dir, old, new) = pair()?;
    let mut model = Model::new(Language::BritishEnglish);
    model.set_work_tab(WorkTab::Comparison);
    let tab = model.comparison_mut();
    tab.set_archive(ComparisonSide::Old, old);
    tab.set_archive(ComparisonSide::New, new);
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1280.0, 900.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    harness.run_quiet();

    harness.get_by_label("Records").click();
    harness.run_quiet();
    harness.get_by_label("Entries").click();
    harness.run_quiet();
    let records_only = harness.state().comparison().levels();
    let compare_enabled = !harness
        .get_by_label("Compare")
        .accesskit_node()
        .is_disabled();
    harness.get_by_label("Records").click();
    harness.run_quiet();

    assert_eq!(
        records_only,
        ComparisonLevels {
            entries: false,
            records: true
        },
        "Records on, Entries off"
    );
    assert!(compare_enabled, "the Record level alone can run");
    assert!(
        harness
            .get_by_label("Compare")
            .accesskit_node()
            .is_disabled(),
        "no level, no Comparison"
    );

    Ok(())
}
