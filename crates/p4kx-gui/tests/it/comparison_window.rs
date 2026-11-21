// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Comparison tab drawn and driven headless over a pair of synthetic Archives.

use crate::comparing;
use crate::waiting::{RunQuiet, wait_for};

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use comparing::{Pair, TestResult, pair};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use p4kx_channels::SearchPaths;
use p4kx_gui::ui::{self, Dialog, Requests};
use p4kx_gui::{
    ChangeColumn, ChannelList, ComparisonSide, ComparisonStatus, Language, Model, ReportStatus,
    SortDirection, WorkTab,
};
use p4kx_settings::{ManualChannels, RecentArchives};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};
use serde_json::Value;

type State = (Model, Requests);

fn harness(pair: &Pair, language: Language) -> Harness<'static, State> {
    let mut model = Model::new(language);
    model.set_channels(ChannelList::new(
        SearchPaths {
            default_library: Some(pair.dir.path().to_path_buf()),
            ..SearchPaths::default()
        },
        ManualChannels::default(),
    ));
    let mut recent = RecentArchives::default();
    recent.push(pair.new.clone());
    model.set_recent(recent);
    model.set_work_tab(WorkTab::Comparison);
    Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1280.0, 900.0])
        .build_ui_state(
            |ui, (model, requests): &mut State| {
                let frame = ui::show(ui, model);
                requests.dialog = requests.dialog.or(frame.dialog);
            },
            (model, Requests::default()),
        )
}

fn settle(harness: &mut Harness<'_, State>, done: impl Fn(&Model) -> bool) -> TestResult {
    wait_for(harness, "the window to settle", |harness| {
        done(&harness.state().0)
    })?;
    harness.run_quiet();
    Ok(())
}

fn finished(model: &Model) -> bool {
    matches!(model.comparison().status(), ComparisonStatus::Finished(view) if !view.is_filtering())
}

fn compared(pair: &Pair, language: Language) -> Result<Harness<'static, State>, Box<dyn Error>> {
    let mut harness = harness(pair, language);
    let tab = harness.state_mut().0.comparison_mut();
    tab.set_archive(ComparisonSide::Old, pair.old.clone());
    tab.set_archive(ComparisonSide::New, pair.new.clone());
    harness.run_quiet();
    let start = if language == Language::German {
        "Vergleichen"
    } else {
        "Compare"
    };
    harness.get_by_label(start).click();
    settle(&mut harness, finished)?;
    Ok(harness)
}

#[test]
fn the_tab_starts_empty_with_both_pickers_both_levels_and_a_disabled_compare() -> TestResult {
    let pair = pair()?;
    let mut harness = harness(&pair, Language::BritishEnglish);
    harness.run_quiet();

    assert!(
        harness.query_by_label("No Comparison yet").is_some(),
        "empty state"
    );
    assert!(
        harness.query_by_label("Old Archive").is_some(),
        "old picker"
    );
    assert!(
        harness.query_by_label("New Archive").is_some(),
        "new picker"
    );
    assert!(
        harness
            .get_by_label("Compare")
            .accesskit_node()
            .is_disabled(),
        "nothing to compare yet"
    );
    assert!(
        !harness
            .get_by_label("Records")
            .accesskit_node()
            .is_disabled(),
        "the Record level can be switched on"
    );
    Ok(())
}

#[test]
fn a_picker_offers_the_channels_the_recent_archives_and_a_file() -> TestResult {
    let pair = pair()?;
    let mut harness = harness(&pair, Language::BritishEnglish);
    harness.run_quiet();

    harness.get_by_label("Old Archive").click();
    harness.run_quiet();
    let offered = (
        harness.query_by_label("LIVE · Build 4.3.1.10203").is_some(),
        harness.query_by_label("PTU · Build 4.3.2.10400").is_some(),
        harness
            .query_by_label(&pair.new.display().to_string())
            .is_some(),
    );
    harness.get_by_label("LIVE · Build 4.3.1.10203").click();
    harness.run_quiet();
    harness.get_by_label("New Archive").click();
    harness.run_quiet();
    harness.get_by_label("Choose file…").click();
    harness.run_quiet();

    assert_eq!(
        offered,
        (true, true, true),
        "Channels and the Recent Archive"
    );
    assert_eq!(
        harness.state().0.comparison().archive(ComparisonSide::Old),
        Some(pair.old.as_path()),
        "LIVE picked"
    );
    assert_eq!(
        harness.state().1.dialog,
        Some(Dialog::ComparisonArchive(ComparisonSide::New)),
        "a file dialog for the new side"
    );
    Ok(())
}

#[test]
fn a_finished_comparison_shows_both_builds_the_counts_and_the_changes() -> TestResult {
    let pair = pair()?;

    let mut harness = compared(&pair, Language::BritishEnglish)?;
    harness.get_by_label("By directory (2)").click();
    harness.run_quiet();

    assert!(
        harness
            .query_by_label("Old LIVE · Build 4.3.1.10203 New PTU · Build 4.3.2.10400")
            .is_some(),
        "both sides with Channel and Build, old first"
    );
    for kpi in ["Added 1", "Removed 1", "Changed 2"] {
        assert!(harness.query_by_label(kpi).is_some(), "{kpi}");
    }

    assert!(
        harness.query_by_label("Engine 1 0 0").is_some(),
        "a directory row"
    );
    for row in [
        "Changed: Data\\Libs\\crc.xml",
        "Changed: Data\\Libs\\size.xml",
        "Removed: Data\\removed.txt",
        "Added: Engine\\added.cfg",
    ] {
        assert!(harness.query_by_label(row).is_some(), "{row}");
    }
    assert!(harness.query_by_label("4 shown").is_some(), "count");
    Ok(())
}

#[test]
fn typing_a_filter_narrows_the_list_and_the_counts() -> TestResult {
    let pair = pair()?;
    let mut harness = compared(&pair, Language::BritishEnglish)?;

    harness.get_by_role(Role::TextInput).click();
    harness.run_quiet();
    harness.get_by_role(Role::TextInput).type_text("engine");
    settle(&mut harness, |model| {
        finished(model)
            && model
                .comparison()
                .view()
                .is_some_and(|view| view.filter_text() == "engine")
    })?;

    assert!(
        harness.query_by_label("Added: Engine\\added.cfg").is_some(),
        "the match"
    );
    assert!(
        harness
            .query_by_label("Removed: Data\\removed.txt")
            .is_none(),
        "filtered out"
    );
    assert!(harness.query_by_label("Changed 0").is_some(), "recounted");
    assert!(harness.query_by_label("1 shown").is_some(), "count");
    Ok(())
}

#[test]
fn a_click_on_a_column_head_sorts_and_a_second_reverses() -> TestResult {
    let pair = pair()?;
    let mut harness = compared(&pair, Language::BritishEnglish)?;

    harness.get_by_label("Old size").click();
    harness.run_quiet();
    harness.get_by_label("Old size").click();
    harness.run_quiet();

    let sort = harness
        .state()
        .0
        .comparison()
        .view()
        .map(p4kx_gui::ComparisonView::sort);
    assert_eq!(
        sort.map(|sort| (sort.column, sort.direction)),
        Some((ChangeColumn::OldSize, SortDirection::Descending)),
        "descending by old size"
    );
    Ok(())
}

#[test]
fn save_report_asks_for_a_file() -> TestResult {
    let pair = pair()?;
    let mut harness = compared(&pair, Language::BritishEnglish)?;

    harness.get_by_label("Save report…").click();
    harness.run_quiet();

    assert_eq!(
        harness.state().1.dialog,
        Some(Dialog::ComparisonReport),
        "the save dialog"
    );
    Ok(())
}

#[test]
fn the_german_tab_speaks_german() -> TestResult {
    let pair = pair()?;

    let harness = compared(&pair, Language::German)?;

    for text in [
        "Vergleich",
        "Hinzugefügt 1",
        "Entfernt 1",
        "Geändert 2",
        "4 angezeigt",
    ] {
        assert!(harness.query_by_label(text).is_some(), "{text}");
    }
    Ok(())
}

const MIXED_CHECKSUM_PATH: &str = "Data/same.txt";
const INCOMPARABLE_NOTICE: &str =
    "1 Entry pairs use different checksum algorithms and cannot be compared.";

#[test]
fn incomparable_entries_follow_the_filter_and_the_saved_report() -> TestResult {
    let pair = mixed_checksum_pair()?;
    let mut harness = compared(&pair, Language::BritishEnglish)?;
    assert_incomparable_summary(&harness);

    apply_filter(&mut harness, "DATA\\SAME.TXT")?;
    assert_incomparable_summary(&harness);
    let included = saved_report(&mut harness, pair.dir.path().join("included.json"))?;
    assert_eq!(
        included
            .pointer("/entries/incomparable/0/path")
            .and_then(Value::as_str),
        Some(MIXED_CHECKSUM_PATH),
        "the visible matching pair is in the saved report"
    );
    assert_eq!(
        included
            .pointer("/entries/counts/changed")
            .and_then(Value::as_u64),
        Some(0),
        "the report has no confirmed change"
    );

    apply_filter(&mut harness, "Engine")?;
    assert!(
        harness.query_by_label(INCOMPARABLE_NOTICE).is_none(),
        "the incomparable notice follows the filter"
    );
    assert!(
        harness
            .query_by_label("No change matches the filter.")
            .is_some(),
        "the empty list describes the filter result"
    );
    let excluded = saved_report(&mut harness, pair.dir.path().join("excluded.json"))?;
    assert!(
        excluded.pointer("/entries/incomparable").is_none(),
        "the saved report omits the excluded pair"
    );
    Ok(())
}

fn mixed_checksum_pair() -> Result<Pair, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let old = dir.path().join("old.zip");
    let entry = || TestEntry::stored(MIXED_CHECKSUM_PATH, b"123456789");
    let plain_zip = TestContainer::new().entry(entry()).build();
    fs::write(&old, plain_zip)?;
    let new = comparing::write(
        dir.path(),
        "PTU",
        "4.3.2.10400",
        &ArchiveBuilder::new().entry(entry()),
    )?;
    Ok(Pair { dir, old, new })
}

fn assert_incomparable_summary(harness: &Harness<'_, State>) {
    for label in [
        "Changed 0",
        INCOMPARABLE_NOTICE,
        "No confirmed Entry changes.",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
    for label in ["Extract old (0)…", "Extract new (0)…"] {
        assert!(
            harness.get_by_label(label).accesskit_node().is_disabled(),
            "{label} is disabled"
        );
    }
}

fn apply_filter(harness: &mut Harness<'_, State>, text: &str) -> TestResult {
    harness
        .state_mut()
        .0
        .comparison_mut()
        .view_mut()
        .ok_or("finished comparison")?
        .set_filter(text, || {});
    settle(harness, finished)
}

fn saved_report(harness: &mut Harness<'_, State>, path: PathBuf) -> Result<Value, Box<dyn Error>> {
    harness
        .state_mut()
        .0
        .comparison_mut()
        .view_mut()
        .ok_or("finished comparison")?
        .save_report(path.clone(), || {});
    settle(harness, |model| {
        model.comparison().view().is_some_and(|view| {
            matches!(
                view.report_status(),
                Some(ReportStatus::Saved(_) | ReportStatus::Failed(_, _))
            )
        })
    })?;
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
